# Wayback Impersonator

`wayback-impersonator` is a robust, concurrent command-line utility built in Rust that retrieves archived assets from the Internet Archive's Wayback Machine CDX API. It utilizes `impersonate-rs` to mimic the TLS/JA3 handshakes and HTTP/2 settings of modern browsers (Chrome, Firefox, Safari, Edge, Tor) to bypass anti-bot protections.

## Features

- **Browser Impersonation**: Uses FFI wrapper `impersonate-rs` around `libcurl-impersonate` to forge browser fingerprints (handshakes, headers, and HTTP/2 configurations).
- **Atomic Progress Journaling**: Saves download progress in a structured JSON journal file (`download_journal.json`). Progress is written atomically to prevent file corruption.
- **Resumable Downloads**: Can be interrupted and resumed using the `--resume` flag, only retrying files that are pending or failed.
- **Error-Specific Retries**: Allows targeted retries based on recorded failure error messages (e.g. retrying only `429` rate-limit errors or timeouts).
- **Concurrency**: Fast downloads utilizing a configurable multithreaded worker pool.
- **Decompression Handling**: Automatically inspects headers and magic bytes to decompress Gzip (`1f 8b`) and Brotli encoded files transparently.
- **Filename Sanitization**: Cleans up path names and query strings to keep clean extensions (e.g., `.woff2`, `.svg`, `.wasm`).

## Installation

### Prerequisites

You must have `libcurl-impersonate` installed on your system.

```bash
# On Linux (ensure the shared library is in your linker path, e.g. /usr/local/lib)
ldconfig -p | grep impersonate
```

### Build

```bash
cargo build --release
```

The compiled binary will be available at `./target/release/wayback-impersonator`.

## Usage

### Options

```text
Robust browser-impersonated scraper using impersonate-rs

Usage: wayback-impersonator [OPTIONS] --url <URL> --mime <MIME>

Options:
  -u, --url <URL>                    The target domain (e.g. wokwi.com) or a direct CDX search URL
  -m, --mime <MIME>                  The target mime type to filter on CDX API (e.g., application/wasm, image/png, text/css)
  -o, --output-dir <OUTPUT_DIR>      Output folder where downloaded assets and the journal will be stored [default: downloads]
  -b, --browser <BROWSER>            Browser profile to impersonate (chrome, firefox, edge, safari, tor) [default: chrome]
  -r, --resume                       Resume the download from download_journal.json in the output directory
      --retry-errors <RETRY_ERRORS>  Retry only downloads that failed with a specific substring in their error message (use "all" for all errors)
  -t, --threads <THREADS>            Number of concurrent downloader threads [default: 4]
      --max-retries <MAX_RETRIES>    Max retry attempts per download [default: 5]
  -v, --verbose                      Verbose output logging
  -h, --help                         Print help
```

### Examples

#### 1. Download unique WebAssembly files
```bash
./target/release/wayback-impersonator --url wokwi.com --mime "application/wasm" --output-dir wasm_downloads --browser chrome124 --threads 8
```

#### 2. Resume a download session
```bash
./target/release/wayback-impersonator --url wokwi.com --mime "application/wasm" --output-dir wasm_downloads --resume
```

#### 3. Retry only rate-limited (HTTP 429) errors
```bash
./target/release/wayback-impersonator --url wokwi.com --mime "application/wasm" --output-dir wasm_downloads --resume --retry-errors "429"
```

## Contributing

Please review the [Contribution Guidelines](CONTRIBUTING.md) for details on our development workflow, Trunk-Based Development, and Conventional Commits format.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
