//! Binary entry point for the `wayback` CLI.
//!
//! Parses command-line arguments with [`clap`], then orchestrates
//! the download session by calling into the [`wayback_impersonator`] library.

use anyhow::Context;
use clap::Parser;
use impersonate_rs::{Browser, Client};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use wayback_impersonator::{
    build_tasks, query_cdx, save_journal, decompress_brotli, decompress_gzip,
    DownloadStatus, Journal,
};

// ─── CLI definition ───────────────────────────────────────────────────────────

/// Command-line arguments parsed by [`clap`].
#[derive(Parser, Debug)]
#[command(
    name = "wayback",
    version,
    author,
    about = "Robust browser-impersonated Wayback Machine scraper",
    long_about = "A concurrent, browser-impersonating scraper in Rust to download \
                  archived assets from the Internet Archive's Wayback Machine CDX API \
                  without being blocked."
)]
struct Cli {
    /// The target domain (e.g. wokwi.com) or a direct CDX search URL.
    #[arg(short, long)]
    url: String,

    /// The target MIME type to filter on the CDX API
    /// (e.g., application/wasm, image/png, text/css).
    #[arg(short, long)]
    mime: String,

    /// Output folder where downloaded assets and the journal will be stored.
    #[arg(short, long, default_value = "downloads")]
    output_dir: PathBuf,

    /// Browser profile to impersonate (chrome, firefox, edge, safari, tor,
    /// or versioned variants such as chrome124, firefox135).
    #[arg(short, long, default_value = "chrome")]
    browser: String,

    /// Resume the download session from `download_journal.json` in the output directory.
    #[arg(short, long)]
    resume: bool,

    /// Retry only downloads that failed with a specific substring in their error message.
    /// Use `"all"` to retry every failed download.
    #[arg(long)]
    retry_errors: Option<String>,

    /// Number of concurrent downloader threads.
    #[arg(short, long, default_value_t = 4)]
    threads: usize,

    /// Maximum retry attempts per individual download (exponential back-off).
    #[arg(long, default_value_t = 5)]
    max_retries: usize,

    /// Enable verbose logging to stdout.
    #[arg(short, long)]
    verbose: bool,
}

// ─── Entry point ─────────────────────────────────────────────────────────────

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Validate and resolve the browser profile.
    let browser = match Browser::from_str(&cli.browser) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error parsing browser profile '{}': {}", cli.browser, e);
            eprintln!("Available profiles: chrome, firefox, edge, safari, tor, chrome124, firefox135, …");
            std::process::exit(1);
        }
    };

    if cli.verbose {
        println!("Selected browser profile: {:?}", browser);
    }

    // Ensure the output directory exists.
    std::fs::create_dir_all(&cli.output_dir)
        .context("Failed to create output directory")?;

    let journal_path = cli.output_dir.join("download_journal.json");

    // Build the impersonating HTTP client.
    let client = Client::builder().impersonate(browser).build();

    // Either resume from an existing journal or start a fresh session.
    let journal = if cli.resume {
        if !journal_path.exists() {
            return Err(anyhow::anyhow!(
                "Resume requested but no journal found at {:?}",
                journal_path
            ));
        }
        if cli.verbose {
            println!("Loading journal from {:?}", journal_path);
        }
        let file = std::fs::File::open(&journal_path)?;
        serde_json::from_reader(file)?
    } else {
        println!("Starting fresh download session…");
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

    // Summarise current state.
    let (mut pending, mut success, mut failed) = (0usize, 0usize, 0usize);
    for t in &journal.tasks {
        match t.status {
            DownloadStatus::Pending => pending += 1,
            DownloadStatus::Success => success += 1,
            DownloadStatus::Failed { .. } => failed += 1,
        }
    }
    println!(
        "State: total={}, pending={}, success={}, failed={}",
        journal.tasks.len(),
        pending,
        success,
        failed
    );

    if pending == 0 && (failed == 0 || cli.retry_errors.is_none()) {
        println!("Nothing to download.");
        return Ok(());
    }

    // Wrap shared state for multi-threaded access.
    let journal = Arc::new(Mutex::new(journal));
    let client = Arc::new(client);
    let output_dir = Arc::new(cli.output_dir);
    let journal_path = Arc::new(journal_path);
    let retry_errors = Arc::new(cli.retry_errors);
    let task_cursor = Arc::new(AtomicUsize::new(0));

    println!("Starting downloads with {} thread(s)…", cli.threads);
    let start = std::time::Instant::now();

    let mut handles = Vec::with_capacity(cli.threads);

    for thread_id in 0..cli.threads {
        let journal = Arc::clone(&journal);
        let client = Arc::clone(&client);
        let output_dir = Arc::clone(&output_dir);
        let journal_path = Arc::clone(&journal_path);
        let retry_errors = Arc::clone(&retry_errors);
        let task_cursor = Arc::clone(&task_cursor);
        let max_retries = cli.max_retries;
        let verbose = cli.verbose;

        handles.push(std::thread::spawn(move || {
            loop {
                // Claim the next task index atomically.
                let idx = task_cursor.fetch_add(1, Ordering::Relaxed);

                let task_opt = {
                    let locked = journal.lock().unwrap();
                    (idx < locked.tasks.len()).then(|| locked.tasks[idx].clone())
                };

                let task = match task_opt {
                    Some(t) => t,
                    None => break, // no more tasks
                };

                // Decide whether this task needs processing.
                let should_process = match &task.status {
                    DownloadStatus::Pending => true,
                    DownloadStatus::Failed { reason } => {
                        retry_errors.as_deref().is_some_and(|f| {
                            f == "all" || reason.to_lowercase().contains(&f.to_lowercase())
                        })
                    }
                    DownloadStatus::Success => false,
                };

                if !should_process {
                    continue;
                }

                if verbose {
                    println!("[thread {thread_id}] → {}", task.local_filename);
                }

                let dest = output_dir.join(&task.local_filename);
                let wayback_url = format!(
                    "http://web.archive.org/web/{}id_/{}",
                    task.timestamp, task.original_url
                );

                let mut result: anyhow::Result<()> = Err(anyhow::anyhow!("not started"));

                for attempt in 0..max_retries {
                    // Staggered back-off: 500 ms + 1.5 s per previous attempt.
                    std::thread::sleep(std::time::Duration::from_millis(
                        500 + attempt as u64 * 1500,
                    ));

                    if verbose && attempt > 0 {
                        println!(
                            "[thread {thread_id}] retry {}/{} for {}",
                            attempt + 1,
                            max_retries,
                            task.local_filename
                        );
                    }

                    match client
                        .get(&wayback_url)
                        .timeout(std::time::Duration::from_secs(30))
                        .send()
                    {
                        Ok(resp) if resp.status() == 200 => {
                            let mut body = resp.bytes().to_vec();

                            // Transparent decompression.
                            if let Some(enc) = resp.headers().get("content-encoding") {
                                let enc = enc.to_lowercase();
                                if enc.contains("gzip") {
                                    if let Ok(d) = decompress_gzip(&body) {
                                        body = d;
                                    }
                                } else if enc.contains("br") {
                                    if let Ok(d) = decompress_brotli(&body) {
                                        body = d;
                                    }
                                }
                            } else if body.starts_with(&[0x1f, 0x8b]) {
                                // Gzip magic bytes present without explicit header.
                                if let Ok(d) = decompress_gzip(&body) {
                                    body = d;
                                }
                            }

                            result = std::fs::write(&dest, body)
                                .map_err(|e| anyhow::anyhow!("Disk write error: {}", e));
                            if result.is_ok() {
                                break;
                            }
                        }
                        Ok(resp) => {
                            result =
                                Err(anyhow::anyhow!("HTTP Status {}", resp.status()));
                        }
                        Err(e) => {
                            result = Err(anyhow::anyhow!("Scraper error: {}", e));
                        }
                    }
                }

                // Persist the updated status atomically.
                {
                    let mut locked = journal.lock().unwrap();
                    match result {
                        Ok(_) => {
                            locked.tasks[idx].status = DownloadStatus::Success;
                            println!("✓ {}", task.local_filename);
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            locked.tasks[idx].status =
                                DownloadStatus::Failed { reason: msg.clone() };
                            eprintln!("✗ {} — {}", task.local_filename, msg);
                        }
                    }
                    if let Err(e) = save_journal(&journal_path, &locked) {
                        eprintln!("Warning: failed to save journal: {}", e);
                    }
                }
            }
        }));
    }

    for h in handles {
        let _ = h.join();
    }

    // Final report.
    let final_journal = journal.lock().unwrap();
    let (fp, fs, ff) = final_journal.tasks.iter().fold((0, 0, 0), |(p, s, f), t| {
        match t.status {
            DownloadStatus::Pending => (p + 1, s, f),
            DownloadStatus::Success => (p, s + 1, f),
            DownloadStatus::Failed { .. } => (p, s, f + 1),
        }
    });

    println!("\n── Final Summary ──────────────────────────");
    println!("  Total:    {}", final_journal.tasks.len());
    println!("  Success:  {}", fs);
    println!("  Pending:  {}", fp);
    println!("  Failed:   {}", ff);
    println!("  Elapsed:  {:.1}s", start.elapsed().as_secs_f64());

    Ok(())
}
