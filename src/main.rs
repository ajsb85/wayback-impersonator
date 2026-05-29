use anyhow::Context;
use clap::Parser;
use impersonate_rs::{Browser, Client};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Parser, Debug)]
#[command(name = "impersonate_downloader", about = "Robust browser-impersonated scraper using impersonate-rs")]
struct Cli {
    /// The target domain (e.g. wokwi.com) or a direct CDX search URL
    #[arg(short, long)]
    url: String,

    /// The target mime type to filter on CDX API (e.g., application/wasm, image/png, text/css)
    #[arg(short, long)]
    mime: String,

    /// Output folder where downloaded assets and the journal will be stored
    #[arg(short, long, default_value = "downloads")]
    output_dir: PathBuf,

    /// Browser profile to impersonate (chrome, firefox, edge, safari, tor)
    #[arg(short, long, default_value = "chrome")]
    browser: String,

    /// Resume the download from download_journal.json in the output directory
    #[arg(short, long)]
    resume: bool,

    /// Retry only downloads that failed with a specific substring in their error message (use "all" for all errors)
    #[arg(long)]
    retry_errors: Option<String>,

    /// Number of concurrent downloader threads
    #[arg(short, long, default_value_t = 4)]
    threads: usize,

    /// Max retry attempts per download
    #[arg(long, default_value_t = 5)]
    max_retries: usize,

    /// Verbose output logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
enum DownloadStatus {
    Pending,
    Success,
    Failed { reason: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DownloadTask {
    original_url: String,
    timestamp: String,
    status: DownloadStatus,
    local_filename: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Journal {
    target_url: String,
    mime_type: String,
    tasks: Vec<DownloadTask>,
}

struct CdxRecord {
    original: String,
    timestamp: String,
    mimetype: String,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    
    // Parse browser
    let browser = match Browser::from_str(&cli.browser) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error parsing browser: {}", e);
            eprintln!("Available browsers: chrome, firefox, edge, safari, tor, chrome124, firefox135, etc.");
            std::process::exit(1);
        }
    };
    
    if cli.verbose {
        println!("Selected browser profile: {:?}", browser);
    }
    
    // Create the output directory
    std::fs::create_dir_all(&cli.output_dir)
        .context("Failed to create output directory")?;
        
    let journal_path = cli.output_dir.join("download_journal.json");
    
    // Build the Client
    let client = Client::builder()
        .impersonate(browser)
        .build();
        
    let mut journal = if cli.resume {
        if !journal_path.exists() {
            return Err(anyhow::anyhow!("Journal file does not exist at {:?}", journal_path));
        }
        if cli.verbose {
            println!("Loading journal from {:?}", journal_path);
        }
        let file = std::fs::File::open(&journal_path)?;
        serde_json::from_reader(file)?
    } else {
        println!("Starting fresh download session...");
        let records = query_cdx(&client, &cli.url, &cli.mime, cli.verbose)?;
        println!("Found {} total records on Wayback CDX.", records.len());
        
        let tasks = build_tasks(records, &cli.mime);
        println!("Deduplicated to {} unique files to download.", tasks.len());
        
        let journal = Journal {
            target_url: cli.url.clone(),
            mime_type: cli.mime.clone(),
            tasks,
        };
        save_journal(&journal_path, &journal)?;
        journal
    };
    
    // Count current state
    let mut pending = 0;
    let mut success = 0;
    let mut failed = 0;
    for t in &journal.tasks {
        match t.status {
            DownloadStatus::Pending => pending += 1,
            DownloadStatus::Success => success += 1,
            DownloadStatus::Failed { .. } => failed += 1,
        }
    }
    println!("Initial state: Total: {}, Pending: {}, Success: {}, Failed: {}", 
             journal.tasks.len(), pending, success, failed);
             
    if pending == 0 && (failed == 0 || cli.retry_errors.is_none()) {
        println!("Nothing to download.");
        return Ok(());
    }
    
    // Wrap shared objects for thread safety
    let journal = Arc::new(Mutex::new(journal));
    let client = Arc::new(client);
    let output_dir = Arc::new(cli.output_dir);
    let journal_path = Arc::new(journal_path);
    let retry_errors = Arc::new(cli.retry_errors);
    let current_task_idx = Arc::new(AtomicUsize::new(0));
    
    let mut thread_handles = Vec::new();
    println!("Starting downloads with {} threads...", cli.threads);
    let start_time = std::time::Instant::now();
    
    for thread_id in 0..cli.threads {
        let journal = Arc::clone(&journal);
        let client = Arc::clone(&client);
        let output_dir = Arc::clone(&output_dir);
        let journal_path = Arc::clone(&journal_path);
        let retry_errors = Arc::clone(&retry_errors);
        let current_task_idx = Arc::clone(&current_task_idx);
        let max_retries = cli.max_retries;
        let verbose = cli.verbose;
        
        let handle = std::thread::spawn(move || {
            loop {
                let idx = current_task_idx.fetch_add(1, Ordering::Relaxed);
                
                let task_opt = {
                    let locked = journal.lock().unwrap();
                    if idx < locked.tasks.len() {
                        Some(locked.tasks[idx].clone())
                    } else {
                        None
                    }
                };
                
                let task = match task_opt {
                    Some(t) => t,
                    None => break,
                };
                
                let should_process = match &task.status {
                    DownloadStatus::Pending => true,
                    DownloadStatus::Failed { reason } => {
                        if let Some(filter) = retry_errors.as_deref() {
                            filter == "all" || reason.to_lowercase().contains(&filter.to_lowercase())
                        } else {
                            false
                        }
                    }
                    DownloadStatus::Success => false,
                };
                
                if !should_process {
                    continue;
                }
                
                if verbose {
                    println!("[Thread {}] Downloading: {}", thread_id, task.local_filename);
                }
                
                let dest_path = output_dir.join(&task.local_filename);
                let wayback_url = format!("http://web.archive.org/web/{}id_/{}", task.timestamp, task.original_url);
                
                let mut result = Err(anyhow::anyhow!("Not started"));
                for attempt in 0..max_retries {
                    // Stagger and backoff
                    std::thread::sleep(std::time::Duration::from_millis(500 + attempt as u64 * 1500));
                    
                    if verbose && attempt > 0 {
                        println!("[Thread {}] Retrying {} (Attempt {}/{})", thread_id, task.local_filename, attempt+1, max_retries);
                    }
                    
                    let req = client.get(&wayback_url)
                        .timeout(std::time::Duration::from_secs(30));
                        
                    match req.send() {
                        Ok(resp) => {
                            if resp.status() == 200 {
                                let mut raw_data = resp.bytes().to_vec();
                                
                                // Decompress content
                                if let Some(encoding) = resp.headers().get("content-encoding") {
                                    let encoding = encoding.to_lowercase();
                                    if encoding.contains("gzip") {
                                        if let Ok(decomp) = decompress_gzip(&raw_data) {
                                            raw_data = decomp;
                                        }
                                    } else if encoding.contains("br") {
                                        if let Ok(decomp) = decompress_brotli(&raw_data) {
                                            raw_data = decomp;
                                        }
                                    }
                                } else {
                                    // Fallback: check gzip magic bytes
                                    if raw_data.starts_with(&[0x1f, 0x8b]) {
                                        if let Ok(decomp) = decompress_gzip(&raw_data) {
                                            raw_data = decomp;
                                        }
                                    }
                                }
                                
                                match std::fs::write(&dest_path, raw_data) {
                                    Ok(_) => {
                                        result = Ok(());
                                        break;
                                    }
                                    Err(e) => {
                                        result = Err(anyhow::anyhow!("Disk write error: {}", e));
                                    }
                                }
                            } else {
                                result = Err(anyhow::anyhow!("HTTP Status {}", resp.status()));
                            }
                        }
                        Err(e) => {
                            result = Err(anyhow::anyhow!("Scraper error: {}", e));
                        }
                    }
                }
                
                // Update status and save journal atomically
                {
                    let mut locked = journal.lock().unwrap();
                    match result {
                        Ok(_) => {
                            locked.tasks[idx].status = DownloadStatus::Success;
                            println!("Successfully downloaded: {}", task.local_filename);
                        }
                        Err(e) => {
                            let err_msg = e.to_string();
                            locked.tasks[idx].status = DownloadStatus::Failed { reason: err_msg.clone() };
                            eprintln!("Failed to download {}: {}", task.local_filename, err_msg);
                        }
                    }
                    if let Err(e) = save_journal(&journal_path, &locked) {
                        eprintln!("Failed to save journal: {}", e);
                    }
                }
            }
        });
        thread_handles.push(handle);
    }
    
    for handle in thread_handles {
        let _ = handle.join();
    }
    
    // Print final report
    let final_journal = journal.lock().unwrap();
    let mut final_pending = 0;
    let mut final_success = 0;
    let mut final_failed = 0;
    for t in &final_journal.tasks {
        match t.status {
            DownloadStatus::Pending => final_pending += 1,
            DownloadStatus::Success => final_success += 1,
            DownloadStatus::Failed { .. } => final_failed += 1,
        }
    }
    
    println!("\nFinal Summary:");
    println!("Total processed: {}", final_journal.tasks.len());
    println!("Downloaded successfully: {}", final_success);
    println!("Pending files: {}", final_pending);
    println!("Failed downloads: {}", final_failed);
    println!("Total elapsed time: {:.1} seconds.", start_time.elapsed().as_secs_f64());
    
    Ok(())
}

fn query_cdx(client: &Client, target_url: &str, mime_type: &str, verbose: bool) -> anyhow::Result<Vec<CdxRecord>> {
    let api_url = if target_url.contains("web.archive.org/cdx/") {
        target_url.to_string()
    } else {
        let encoded_mime = url::form_urlencoded::byte_serialize(mime_type.as_bytes()).collect::<String>();
        format!(
            "http://web.archive.org/cdx/search/cdx?url={}/*&filter=mimetype:{}&collapse=urlkey&output=json",
            target_url, encoded_mime
        )
    };

    if verbose {
        println!("Querying CDX API: {}", api_url);
    }

    let response = client.get(&api_url).send()
        .map_err(|e| anyhow::anyhow!("Request failed: {}", e))?;
        
    if response.status() != 200 {
        return Err(anyhow::anyhow!("CDX query failed with HTTP status {}", response.status()));
    }

    let data: Vec<Vec<String>> = response.json()
        .map_err(|e| anyhow::anyhow!("Failed to parse CDX JSON: {}", e))?;
        
    if data.len() <= 1 {
        return Ok(Vec::new());
    }

    let header = &data[0];
    let mut records = Vec::new();

    for row in &data[1..] {
        let mut original = String::new();
        let mut timestamp = String::new();
        let mut mimetype = String::new();

        for (idx, field) in header.iter().enumerate() {
            if idx < row.len() {
                match field.as_str() {
                    "original" => original = row[idx].clone(),
                    "timestamp" => timestamp = row[idx].clone(),
                    "mimetype" => mimetype = row[idx].clone(),
                    _ => {}
                }
            }
        }

        if !original.is_empty() && !timestamp.is_empty() {
            records.push(CdxRecord {
                original,
                timestamp,
                mimetype,
            });
        }
    }

    Ok(records)
}

fn build_tasks(records: Vec<CdxRecord>, mime_type: &str) -> Vec<DownloadTask> {
    let clean_extensions = match mime_type {
        m if m.contains("wasm") => vec![".wasm"],
        m if m.contains("css") => vec![".css"],
        m if m.contains("png") => vec![".png"],
        m if m.contains("jpeg") || m.contains("jpg") => vec![".jpeg", ".jpg"],
        m if m.contains("octet-stream") => vec![".bin"],
        m if m.contains("font") || m.contains("woff") || m.contains("ttf") || m.contains("eot") => {
            vec![".woff2", ".woff", ".ttf", ".eot"]
        }
        _ => vec![".html", ".js", ".json", ".txt"],
    };

    let mut task_map = std::collections::HashMap::new();

    for record in records {
        let local_filename = sanitize_filename(&record.original, &clean_extensions);
        
        // Keep the latest timestamp capture of the file
        if let Some(existing) = task_map.get(&local_filename) as Option<&CdxRecord> {
            if record.timestamp > existing.timestamp {
                task_map.insert(local_filename, record);
            }
        } else {
            task_map.insert(local_filename, record);
        }
    }

    let mut tasks = Vec::new();
    for (local_filename, record) in task_map {
        tasks.push(DownloadTask {
            original_url: record.original,
            timestamp: record.timestamp,
            status: DownloadStatus::Pending,
            local_filename,
        });
    }

    tasks
}

fn sanitize_filename(original_url: &str, clean_extensions: &[&str]) -> String {
    let parsed = url::Url::parse(original_url);
    let (path, query) = match parsed {
        Ok(u) => (u.path().to_string(), u.query().map(|q| q.to_string())),
        Err(_) => (original_url.to_string(), None),
    };

    let clean_path = path.trim_start_matches('/');
    let mut filename = clean_path.replace('/', "_");
    if let Some(q) = query {
        filename = format!("{}_{}", filename, q);
    }

    for ext in clean_extensions {
        if filename.contains(ext) {
            if let Some(idx) = filename.find(ext) {
                filename = filename[..idx + ext.len()].to_string();
                break;
            }
        }
    }

    filename
}

fn save_journal(journal_path: &Path, journal: &Journal) -> anyhow::Result<()> {
    let tmp_path = journal_path.with_extension("tmp");
    let file = std::fs::File::create(&tmp_path)?;
    serde_json::to_writer_pretty(file, journal)?;
    std::fs::rename(tmp_path, journal_path)?;
    Ok(())
}

fn decompress_gzip(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    use flate2::read::GzDecoder;
    use std::io::Read;
    
    let mut decoder = GzDecoder::new(bytes);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

fn decompress_brotli(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    use brotli::Decompressor;
    use std::io::Read;
    
    let mut decompressed = Vec::new();
    let mut decompressor = Decompressor::new(bytes, 4096);
    // Ignore error if it's not valid brotli and return original
    match decompressor.read_to_end(&mut decompressed) {
        Ok(_) => Ok(decompressed),
        Err(_) => Ok(bytes.to_vec()),
    }
}
