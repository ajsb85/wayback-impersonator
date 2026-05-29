//! # wayback
//!
//! A concurrent, browser-impersonating scraper written in Rust that downloads
//! archived assets from the Internet Archive's Wayback Machine CDX API without
//! being blocked.
//!
//! ## Architecture
//!
//! The library is structured around four main concerns:
//!
//! - **CDX querying** ([`query_cdx`]) — fetches the list of captured URLs from
//!   the Wayback Machine CDX API, using a browser-impersonating HTTP client to
//!   avoid bot-detection.
//! - **Task building** ([`build_tasks`]) — deduplicates CDX records by URL key,
//!   keeping the most-recently captured snapshot of each asset.
//! - **Journal I/O** ([`save_journal`]) — atomically persists download state so
//!   interrupted sessions can be resumed.
//! - **Decompression** ([`decompress_gzip`], [`decompress_brotli`]) — transparent
//!   content-encoding handling for responses that arrive compressed.
//!
//! ## Example
//!
//! ```no_run
//! use wayback_impersonator::{build_tasks, decompress_gzip, CdxRecord};
//!
//! let records = vec![
//!     CdxRecord {
//!         original:  "https://example.com/app.wasm".to_string(),
//!         timestamp: "20230601120000".to_string(),
//!         mimetype:  "application/wasm".to_string(),
//!     },
//! ];
//! let tasks = build_tasks(records, "application/wasm");
//! assert_eq!(tasks.len(), 1);
//! ```

use anyhow::Context;
use impersonate_rs::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;

// ─── Public types ────────────────────────────────────────────────────────────

/// The lifecycle state of a single download task.
///
/// Persisted in the JSON journal so interrupted sessions can resume
/// from where they left off.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum DownloadStatus {
    /// The asset has not been attempted yet.
    Pending,
    /// The asset was downloaded and written to disk successfully.
    Success,
    /// The asset failed after all retry attempts. Stores the last error message
    /// so `--retry-errors` can filter on it.
    Failed {
        /// Human-readable description of the last failure.
        reason: String,
    },
}

/// A single asset to be downloaded from the Wayback Machine.
///
/// Created by [`build_tasks`] from a [`CdxRecord`] and stored in the [`Journal`].
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DownloadTask {
    /// The original URL of the asset as recorded by the CDX API.
    pub original_url: String,
    /// The Wayback Machine capture timestamp (14-digit: `YYYYMMDDHHmmss`).
    pub timestamp: String,
    /// Current download state.
    pub status: DownloadStatus,
    /// Sanitised local filename derived from `original_url`.
    pub local_filename: String,
}

/// The session journal that tracks every discovered asset and its download state.
///
/// Written to `<output_dir>/download_journal.json` atomically after every
/// status change. Reload it with [`serde_json::from_reader`] to resume.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Journal {
    /// The domain or CDX URL that was scraped.
    pub target_url: String,
    /// The MIME type filter used for the CDX query.
    pub mime_type: String,
    /// All discovered assets and their statuses.
    pub tasks: Vec<DownloadTask>,
}

/// A raw record returned by the Wayback Machine CDX API.
///
/// Passed in bulk to [`build_tasks`], which deduplicates them into
/// [`DownloadTask`] entries.
#[derive(Debug, Clone)]
pub struct CdxRecord {
    /// Original URL of the asset.
    pub original: String,
    /// Capture timestamp (`YYYYMMDDHHmmss`).
    pub timestamp: String,
    /// MIME type as reported by the CDX API.
    pub mimetype: String,
}

// ─── CDX query ───────────────────────────────────────────────────────────────

/// Queries the Wayback Machine CDX API and returns a flat list of captured records.
///
/// If `target_url` already contains `web.archive.org/cdx/` it is used as-is;
/// otherwise a canonical CDX URL is constructed from the domain and MIME type:
///
/// ```text
/// http://web.archive.org/cdx/search/cdx?url={domain}/*
///   &filter=mimetype:{mime}
///   &collapse=urlkey
///   &output=json
/// ```
///
/// The `collapse=urlkey` parameter tells the CDX API to return only one
/// result per unique URL key, significantly reducing response size.
///
/// # Errors
///
/// Returns an error if the HTTP request fails, the API responds with a
/// non-200 status, or the JSON body cannot be parsed.
pub fn query_cdx(
    client: &Client,
    target_url: &str,
    mime_type: &str,
    verbose: bool,
) -> anyhow::Result<Vec<CdxRecord>> {
    let api_url = if target_url.contains("web.archive.org/cdx/") {
        target_url.to_string()
    } else {
        let encoded_mime =
            url::form_urlencoded::byte_serialize(mime_type.as_bytes()).collect::<String>();
        format!(
            "http://web.archive.org/cdx/search/cdx?url={}/*&filter=mimetype:{}&collapse=urlkey&output=json",
            target_url, encoded_mime
        )
    };

    if verbose {
        println!("Querying CDX API: {}", api_url);
    }

    let response = client
        .get(&api_url)
        .send()
        .map_err(|e| anyhow::anyhow!("CDX request failed: {}", e))?;

    if response.status() != 200 {
        return Err(anyhow::anyhow!(
            "CDX query returned HTTP {}",
            response.status()
        ));
    }

    // The CDX JSON format is a 2-D array: first row = field names, rest = values.
    let data: Vec<Vec<String>> = response
        .json()
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

// ─── Task building ────────────────────────────────────────────────────────────

/// Deduplicates CDX records into download tasks, keeping the latest snapshot
/// of each unique asset.
///
/// Two records are considered duplicates when their sanitised local filenames
/// (computed by [`sanitize_filename`]) are identical. When duplicates exist
/// the record with the lexicographically greater timestamp is kept, ensuring
/// the most recent capture is used.
///
/// All returned tasks start with [`DownloadStatus::Pending`].
///
/// # Example
///
/// ```
/// use wayback_impersonator::{build_tasks, CdxRecord, DownloadStatus};
///
/// let records = vec![
///     CdxRecord { original: "https://example.com/a.wasm".into(),
///                 timestamp: "20200101000000".into(), mimetype: "application/wasm".into() },
///     CdxRecord { original: "https://example.com/a.wasm".into(),
///                 timestamp: "20230101000000".into(), mimetype: "application/wasm".into() },
/// ];
/// let tasks = build_tasks(records, "application/wasm");
/// assert_eq!(tasks.len(), 1);
/// assert_eq!(tasks[0].timestamp, "20230101000000");
/// assert_eq!(tasks[0].status, DownloadStatus::Pending);
/// ```
pub fn build_tasks(records: Vec<CdxRecord>, mime_type: &str) -> Vec<DownloadTask> {
    // Map known MIME types to their expected file extensions so
    // sanitize_filename can truncate cleanly.
    let clean_extensions: Vec<&str> = match mime_type {
        m if m.contains("wasm") => vec![".wasm"],
        m if m.contains("css") => vec![".css"],
        m if m.contains("png") => vec![".png"],
        m if m.contains("jpeg") || m.contains("jpg") => vec![".jpeg", ".jpg"],
        m if m.contains("octet-stream") => vec![".bin"],
        m if m.contains("font")
            || m.contains("woff")
            || m.contains("ttf")
            || m.contains("eot") =>
        {
            vec![".woff2", ".woff", ".ttf", ".eot"]
        }
        _ => vec![".html", ".js", ".json", ".txt"],
    };

    let mut task_map: std::collections::HashMap<String, CdxRecord> =
        std::collections::HashMap::new();

    for record in records {
        let local_filename = sanitize_filename(&record.original, &clean_extensions);

        // Retain only the most-recently captured snapshot.
        let keep = match task_map.get(&local_filename) {
            Some(existing) => record.timestamp > existing.timestamp,
            None => true,
        };
        if keep {
            task_map.insert(local_filename, record);
        }
    }

    task_map
        .into_iter()
        .map(|(local_filename, record)| DownloadTask {
            original_url: record.original,
            timestamp: record.timestamp,
            status: DownloadStatus::Pending,
            local_filename,
        })
        .collect()
}

// ─── Filename sanitisation ───────────────────────────────────────────────────

/// Converts an original asset URL into a safe, flat local filename.
///
/// The transformation pipeline is:
///
/// 1. Parse the URL and extract the path component (falls back to the raw
///    string if parsing fails).
/// 2. Strip the leading `/`.
/// 3. Replace `/` with `_` so the result is a single, flat filename with no
///    directory components.
/// 4. Append the URL query string (if any) separated by `_`.
/// 5. Truncate at the first known extension (from `clean_extensions`) so query
///    parameters after the extension are discarded.
///
/// # Arguments
///
/// * `original_url` — The full URL as returned by the CDX API.
/// * `clean_extensions` — Ordered list of known extensions to match (e.g.
///   `&[".wasm", ".js"]`). Matching stops at the first hit.
///
/// # Example
///
/// ```
/// use wayback_impersonator::sanitize_filename;
///
/// // Path separators become underscores:
/// assert_eq!(sanitize_filename("https://example.com/a/b/c.wasm", &[".wasm"]), "a_b_c.wasm");
///
/// // Query string is stripped after extension:
/// assert_eq!(sanitize_filename("https://example.com/main.css?v=42", &[".css"]), "main.css");
///
/// // No extension match → full sanitised path is returned:
/// assert!(!sanitize_filename("https://example.com/data.bin", &[".wasm"]).is_empty());
/// ```
pub fn sanitize_filename(original_url: &str, clean_extensions: &[&str]) -> String {
    let parsed = url::Url::parse(original_url);
    let (path, query) = match parsed {
        Ok(u) => (
            u.path().to_string(),
            u.query().map(|q| q.to_string()),
        ),
        Err(_) => (original_url.to_string(), None),
    };

    let clean_path = path.trim_start_matches('/');
    let mut filename = clean_path.replace('/', "_");

    if let Some(q) = query {
        filename = format!("{}_{}", filename, q);
    }

    // Truncate at the first recognised extension so trailing query cruft is dropped.
    for ext in clean_extensions {
        if let Some(idx) = filename.find(ext) {
            filename = filename[..idx + ext.len()].to_string();
            break;
        }
    }

    filename
}

// ─── Journal persistence ─────────────────────────────────────────────────────

/// Atomically writes a [`Journal`] to `journal_path`.
///
/// The journal is first serialised to a `.tmp` sibling file and then
/// renamed over the target path. This ensures that a crash or power loss
/// during the write cannot produce a corrupted or truncated journal.
///
/// # Errors
///
/// Returns an error if the temporary file cannot be created, serialisation
/// fails, or the rename operation fails.
///
/// # Example
///
/// ```no_run
/// use std::path::PathBuf;
/// use wayback_impersonator::{Journal, DownloadStatus, DownloadTask, save_journal};
///
/// let journal = Journal {
///     target_url: "example.com".to_string(),
///     mime_type: "application/wasm".to_string(),
///     tasks: vec![],
/// };
/// save_journal(&PathBuf::from("/tmp/journal.json"), &journal).unwrap();
/// ```
pub fn save_journal(journal_path: &Path, journal: &Journal) -> anyhow::Result<()> {
    let tmp_path = journal_path.with_extension("tmp");
    let file =
        std::fs::File::create(&tmp_path).context("Failed to create temporary journal file")?;
    serde_json::to_writer_pretty(file, journal).context("Failed to serialise journal")?;
    std::fs::rename(&tmp_path, journal_path).context("Failed to atomically rename journal")?;
    Ok(())
}

// ─── Decompression ───────────────────────────────────────────────────────────

/// Decompresses a Gzip-encoded byte slice.
///
/// Used to transparently decompress Wayback Machine responses that arrive with
/// `Content-Encoding: gzip`, and also as a fallback when the magic bytes
/// `1f 8b` are detected in the response body.
///
/// # Errors
///
/// Returns an [`std::io::Error`] if the data is not valid Gzip.
///
/// # Example
///
/// ```
/// use wayback_impersonator::decompress_gzip;
/// use flate2::write::GzEncoder;
/// use flate2::Compression;
/// use std::io::Write;
///
/// let mut enc = GzEncoder::new(Vec::new(), Compression::default());
/// enc.write_all(b"hello").unwrap();
/// let compressed = enc.finish().unwrap();
///
/// let decompressed = decompress_gzip(&compressed).unwrap();
/// assert_eq!(decompressed, b"hello");
/// ```
pub fn decompress_gzip(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let mut decoder = GzDecoder::new(bytes);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

/// Decompresses a Brotli-encoded byte slice.
///
/// Used to transparently decompress Wayback Machine responses that arrive with
/// `Content-Encoding: br`. If decompression fails (e.g. the data is not valid
/// Brotli) the **original bytes** are returned unchanged, acting as a
/// pass-through fallback.
///
/// # Errors
///
/// This function is infallible at the `Result` level — it always returns
/// `Ok`. Brotli decode errors silently fall back to returning the input.
///
/// # Example
///
/// ```
/// use wayback_impersonator::decompress_brotli;
///
/// // Invalid Brotli → original bytes returned unchanged (fallback).
/// let data = b"not brotli";
/// let result = decompress_brotli(data).unwrap();
/// assert_eq!(result, data);
/// ```
pub fn decompress_brotli(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    use brotli::Decompressor;
    use std::io::Read;

    let mut decompressed = Vec::new();
    let mut decompressor = Decompressor::new(bytes, 4096);
    match decompressor.read_to_end(&mut decompressed) {
        Ok(_) => Ok(decompressed),
        Err(_) => Ok(bytes.to_vec()), // graceful pass-through on invalid input
    }
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Compress `data` with Gzip using the same library used by the decoder.
    fn gzip_compress(data: &[u8]) -> Vec<u8> {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(data).unwrap();
        enc.finish().unwrap()
    }

    /// Compress `data` with Brotli using the same library used by the decoder.
    fn brotli_compress(data: &[u8]) -> Vec<u8> {
        use brotli::CompressorWriter;
        use std::io::Write;
        let mut out = Vec::new();
        {
            let mut w = CompressorWriter::new(&mut out, 4096, 11, 22);
            w.write_all(data).unwrap();
        }
        out
    }

    fn make_record(original: &str, timestamp: &str) -> CdxRecord {
        CdxRecord {
            original: original.to_string(),
            timestamp: timestamp.to_string(),
            mimetype: "application/wasm".to_string(),
        }
    }

    fn make_journal() -> Journal {
        Journal {
            target_url: "example.com".to_string(),
            mime_type: "application/wasm".to_string(),
            tasks: vec![DownloadTask {
                original_url: "https://example.com/app.wasm".to_string(),
                timestamp: "20230601120000".to_string(),
                status: DownloadStatus::Success,
                local_filename: "app.wasm".to_string(),
            }],
        }
    }

    // ── sanitize_filename ────────────────────────────────────────────────────

    #[test]
    fn sanitize_simple_path() {
        assert_eq!(
            sanitize_filename("https://example.com/file.wasm", &[".wasm"]),
            "file.wasm"
        );
    }

    #[test]
    fn sanitize_nested_path_replaces_slashes() {
        assert_eq!(
            sanitize_filename("https://example.com/a/b/c.wasm", &[".wasm"]),
            "a_b_c.wasm"
        );
    }

    #[test]
    fn sanitize_strips_query_after_extension() {
        assert_eq!(
            sanitize_filename("https://example.com/main.css?v=42&cb=1", &[".css"]),
            "main.css"
        );
    }

    #[test]
    fn sanitize_query_string_appended_when_no_ext_match() {
        // Extension not in the list → query is appended, full path returned
        let result = sanitize_filename("https://example.com/data.bin?x=1", &[".wasm"]);
        assert!(result.contains("data.bin"));
        assert!(result.contains("x=1"));
    }

    #[test]
    fn sanitize_invalid_url_returns_input() {
        // Non-URL strings are returned as-is (best-effort).
        let result = sanitize_filename("not_a_url", &[".wasm"]);
        assert_eq!(result, "not_a_url");
    }

    #[test]
    fn sanitize_root_path_trims_leading_slash() {
        let result = sanitize_filename("https://example.com/", &[".html"]);
        // path is "/" → clean_path is "" → filename is ""
        assert_eq!(result, "");
    }

    #[test]
    fn sanitize_chooses_first_matching_extension() {
        // Both .woff2 and .woff are in the list; .woff2 appears first.
        let result = sanitize_filename(
            "https://example.com/fonts/icon.woff2?v=3",
            &[".woff2", ".woff"],
        );
        assert_eq!(result, "fonts_icon.woff2");
    }

    // ── build_tasks ──────────────────────────────────────────────────────────

    #[test]
    fn build_tasks_empty_input() {
        assert!(build_tasks(vec![], "application/wasm").is_empty());
    }

    #[test]
    fn build_tasks_single_record() {
        let tasks = build_tasks(
            vec![make_record("https://example.com/app.wasm", "20230101000000")],
            "application/wasm",
        );
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].local_filename, "app.wasm");
        assert_eq!(tasks[0].timestamp, "20230101000000");
    }

    #[test]
    fn build_tasks_dedup_keeps_latest_timestamp() {
        let records = vec![
            make_record("https://example.com/app.wasm", "20200101000000"),
            make_record("https://example.com/app.wasm", "20230601000000"), // newer
            make_record("https://example.com/app.wasm", "20210101000000"),
        ];
        let tasks = build_tasks(records, "application/wasm");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].timestamp, "20230601000000");
    }

    #[test]
    fn build_tasks_multiple_unique_urls() {
        let records = vec![
            make_record("https://example.com/a.wasm", "20230101000000"),
            make_record("https://example.com/b.wasm", "20230101000000"),
            make_record("https://example.com/c.wasm", "20230101000000"),
        ];
        let tasks = build_tasks(records, "application/wasm");
        assert_eq!(tasks.len(), 3);
    }

    #[test]
    fn build_tasks_all_statuses_start_as_pending() {
        let records = vec![
            make_record("https://example.com/a.wasm", "20230101000000"),
            make_record("https://example.com/b.wasm", "20230101000000"),
        ];
        let tasks = build_tasks(records, "application/wasm");
        assert!(tasks
            .iter()
            .all(|t| t.status == DownloadStatus::Pending));
    }

    #[test]
    fn build_tasks_mime_css_uses_css_extension() {
        let tasks = build_tasks(
            vec![make_record("https://example.com/style.css", "20230101")],
            "text/css",
        );
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].local_filename.ends_with(".css"));
    }

    // ── DownloadStatus ───────────────────────────────────────────────────────

    #[test]
    fn download_status_failed_stores_reason() {
        let s = DownloadStatus::Failed {
            reason: "HTTP 429 Too Many Requests".to_string(),
        };
        match s {
            DownloadStatus::Failed { reason } => {
                assert!(reason.contains("429"))
            }
            _ => panic!("Expected Failed variant"),
        }
    }

    #[test]
    fn download_status_equality() {
        assert_eq!(DownloadStatus::Pending, DownloadStatus::Pending);
        assert_eq!(DownloadStatus::Success, DownloadStatus::Success);
        assert_ne!(DownloadStatus::Pending, DownloadStatus::Success);
        assert_ne!(
            DownloadStatus::Pending,
            DownloadStatus::Failed {
                reason: "err".to_string()
            }
        );
    }

    // ── decompress_gzip ──────────────────────────────────────────────────────

    #[test]
    fn gzip_roundtrip() {
        let original = b"wayback gzip roundtrip test payload";
        let compressed = gzip_compress(original);
        let decompressed = decompress_gzip(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn gzip_empty_payload() {
        let compressed = gzip_compress(b"");
        let decompressed = decompress_gzip(&compressed).unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    fn gzip_invalid_bytes_returns_error() {
        let result = decompress_gzip(b"this is not gzip data at all");
        assert!(result.is_err());
    }

    #[test]
    fn gzip_detects_magic_bytes() {
        // Ensure compressed data starts with Gzip magic bytes 0x1f 0x8b.
        let compressed = gzip_compress(b"hello");
        assert_eq!(&compressed[..2], &[0x1f, 0x8b]);
    }

    // ── decompress_brotli ────────────────────────────────────────────────────

    #[test]
    fn brotli_roundtrip() {
        let original = b"wayback brotli roundtrip test payload";
        let compressed = brotli_compress(original);
        let decompressed = decompress_brotli(&compressed).unwrap();
        assert_eq!(decompressed, original.as_ref());
    }

    #[test]
    fn brotli_empty_payload() {
        let compressed = brotli_compress(b"");
        let decompressed = decompress_brotli(&compressed).unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    fn brotli_invalid_input_falls_back_to_original() {
        // The decoder falls back to returning the original bytes — never panics.
        let data = b"this is not valid brotli";
        let result = decompress_brotli(data).unwrap();
        assert_eq!(result, data.as_ref());
    }

    // ── save_journal (uses tempfile) ─────────────────────────────────────────

    #[test]
    fn journal_save_and_reload_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.json");

        let original = make_journal();
        save_journal(&path, &original).unwrap();

        let file = std::fs::File::open(&path).unwrap();
        let loaded: Journal = serde_json::from_reader(file).unwrap();

        assert_eq!(loaded.target_url, original.target_url);
        assert_eq!(loaded.mime_type, original.mime_type);
        assert_eq!(loaded.tasks.len(), 1);
        assert_eq!(loaded.tasks[0].local_filename, "app.wasm");
        assert_eq!(loaded.tasks[0].status, DownloadStatus::Success);
    }

    #[test]
    fn journal_write_is_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.json");

        save_journal(&path, &make_journal()).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed.is_object());
        assert!(parsed["tasks"].is_array());
    }

    #[test]
    fn journal_atomic_write_removes_tmp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.json");

        save_journal(&path, &make_journal()).unwrap();

        // The temporary `.tmp` file must not be left on disk.
        let tmp = path.with_extension("tmp");
        assert!(!tmp.exists(), ".tmp file was not cleaned up after rename");
    }

    #[test]
    fn journal_serialises_failed_status_reason() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.json");

        let mut j = make_journal();
        j.tasks[0].status = DownloadStatus::Failed {
            reason: "HTTP 503".to_string(),
        };
        save_journal(&path, &j).unwrap();

        let file = std::fs::File::open(&path).unwrap();
        let loaded: Journal = serde_json::from_reader(file).unwrap();
        match &loaded.tasks[0].status {
            DownloadStatus::Failed { reason } => assert_eq!(reason, "HTTP 503"),
            _ => panic!("Expected Failed status after reload"),
        }
    }

    #[test]
    fn journal_empty_tasks_list() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.json");

        let j = Journal {
            target_url: "empty.test".to_string(),
            mime_type: "text/html".to_string(),
            tasks: vec![],
        };
        save_journal(&path, &j).unwrap();

        let file = std::fs::File::open(&path).unwrap();
        let loaded: Journal = serde_json::from_reader(file).unwrap();
        assert!(loaded.tasks.is_empty());
    }
}
