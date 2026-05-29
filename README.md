# wayback

> A concurrent, browser-impersonating scraper in Rust — download archived assets
> from the Internet Archive's Wayback Machine CDX API without being blocked.

[![License: MIT](https://img.shields.io/badge/license-MIT-ab2e33.svg)](LICENSE)
[![Latest release](https://img.shields.io/github/v/release/ajsb85/wayback-impersonator?color=ab2e33)](https://github.com/ajsb85/wayback-impersonator/releases)
[![Debian package](https://img.shields.io/badge/debian-amd64-ab2e33)](https://ajsb85.github.io/wayback-impersonator/)

`wayback` uses [`impersonate-rs`](https://github.com/rust-impersonate/impersonate-rs) —
a Rust FFI wrapper around `libcurl-impersonate` — to forge the TLS/JA3 handshakes,
HTTP/2 settings, and browser headers of Chrome, Firefox, Safari, Edge, and Tor Browser.
This lets it bypass anti-bot protections that reject standard HTTP clients.

---

## Table of Contents

- [Features](#features)
- [Prerequisites](#prerequisites)
- [Installation](#installation)
  - [Debian / Ubuntu — APT Repository](#debian--ubuntu--apt-repository)
  - [Direct .deb Download](#direct-deb-download)
  - [Build from Source](#build-from-source)
- [Usage](#usage)
  - [Options](#options)
  - [Examples](#examples)
- [Shell Completions](#shell-completions)
- [Man Page](#man-page)
- [Contributing](#contributing)
- [License](#license)

---

## Features

| Feature | Description |
|---|---|
| **Browser Impersonation** | Forges TLS fingerprints, HTTP/2 frames, and headers via `libcurl-impersonate` |
| **Atomic Progress Journal** | Writes download state to `download_journal.json` atomically (via rename) to prevent corruption |
| **Resumable Downloads** | `--resume` re-loads the journal and skips already-successful files |
| **Targeted Retry** | `--retry-errors` retries only failures matching a specific substring (e.g. `429`, `timeout`) |
| **Concurrency** | Configurable multi-threaded worker pool via `--threads` |
| **Transparent Decompression** | Automatically decompresses Gzip (`1f 8b`) and Brotli encoded responses |
| **Filename Sanitization** | Strips query strings and path separators to produce clean local filenames |

---

## Prerequisites

### `libcurl-impersonate` (required at runtime)

`wayback` dynamically links against `libcurl-impersonate`. This library is **not**
in the official Ubuntu/Debian repositories, so it is not declared as an APT
dependency. You must install it manually before `wayback` will run.

```bash
# 1. Install TLS/NSS dependencies
sudo apt-get update && sudo apt-get install -y \
    libnss3 nss-plugin-pem ca-certificates wget

# 2. Download the pre-compiled release (v0.6.1)
wget https://github.com/lwthiker/curl-impersonate/releases/download/v0.6.1/libcurl-impersonate-v0.6.1.x86_64-linux-gnu.tar.gz
tar -xvf libcurl-impersonate-v0.6.1.x86_64-linux-gnu.tar.gz

# 3. Move shared libraries into the linker path and reload
sudo mv libcurl-impersonate-chrome.so* /usr/local/lib/
sudo mv libcurl-impersonate-ff.so*     /usr/local/lib/
sudo ldconfig

# 4. Clean up
rm -f libcurl-impersonate-v0.6.1.x86_64-linux-gnu.tar.gz
```

Verify the installation:

```bash
ldconfig -p | grep impersonate
# → libcurl-impersonate-chrome.so (libc6,x86-64) => /usr/local/lib/libcurl-impersonate-chrome.so
```

---

## Installation

### Debian / Ubuntu — APT Repository

The recommended method. Packages are signed with GPG and updates are delivered
automatically via `apt upgrade`.

**Step 1 — Trust the repository signing key:**

```bash
sudo wget -O /usr/share/keyrings/wayback-keyring.gpg \
    https://ajsb85.github.io/wayback-impersonator/amd64/archive-key.gpg
```

> The ASCII-armoured public key is also available at
> `https://ajsb85.github.io/wayback-impersonator/amd64/archive-key.asc`

**Step 2 — Add the APT source:**

```bash
echo "deb [signed-by=/usr/share/keyrings/wayback-keyring.gpg] \
    https://ajsb85.github.io/wayback-impersonator/amd64/ ./" \
  | sudo tee /etc/apt/sources.list.d/wayback.list
```

**Step 3 — Update and install:**

```bash
sudo apt update
sudo apt install wayback
```

The binary is installed system-wide at `/usr/bin/wayback`. The package also
installs the [man page](#man-page) and [shell completions](#shell-completions)
for bash, zsh, and fish.

**Upgrading:**

```bash
sudo apt update && sudo apt upgrade wayback
```

**Verifying the GPG signature on the `.deb` file:**

```bash
wget https://ajsb85.github.io/wayback-impersonator/amd64/wayback_0.1.9-1_amd64.deb
wget https://ajsb85.github.io/wayback-impersonator/amd64/wayback_0.1.9-1_amd64.deb.asc
gpg --verify wayback_0.1.9-1_amd64.deb.asc wayback_0.1.9-1_amd64.deb
```

---

### Direct .deb Download

If you prefer a one-off install without adding the APT repository:

```bash
# Download the .deb from the latest release
wget https://github.com/ajsb85/wayback-impersonator/releases/latest/download/wayback_0.1.9-1_amd64.deb

# Install
sudo dpkg -i wayback_0.1.9-1_amd64.deb
```

> **Note:** You will not receive automatic updates with this method.

---

### Build from Source

Requires Rust (stable toolchain) and `libcurl-impersonate` installed (see
[Prerequisites](#prerequisites)).

```bash
git clone https://github.com/ajsb85/wayback-impersonator.git
cd wayback-impersonator

# Compile in release mode
cargo build --release

# The binary is at:
./target/release/wayback-impersonator --version

# Optional: install it system-wide
sudo install -m 755 target/release/wayback-impersonator /usr/local/bin/wayback

# Optional: build and install the .deb package
cargo install cargo-deb
gzip -9 --keep debian/wayback.1
gzip -9 --keep debian/changelog
cargo deb
sudo dpkg -i target/debian/wayback_*.deb
```

---

## Usage

### Options

```text
wayback 0.1.9
A concurrent, browser-impersonating scraper in Rust to download archived assets
from the Internet Archive's Wayback Machine CDX API without being blocked.

Usage: wayback [OPTIONS] --url <URL> --mime <MIME>

Options:
  -u, --url <URL>
          Target domain (e.g. wokwi.com) or a full CDX search URL

  -m, --mime <MIME>
          MIME type to filter on the CDX API
          (e.g. application/wasm, image/png, text/css, font/woff2)

  -o, --output-dir <DIR>
          Output directory for downloaded assets and the journal
          [default: downloads]

  -b, --browser <PROFILE>
          Browser profile to impersonate
          Values: chrome, firefox, edge, safari, tor
                  chrome124, chrome131, firefox135, safari18, …
          [default: chrome]

  -r, --resume
          Resume an interrupted session by reloading the journal

      --retry-errors <PATTERN>
          Retry failed downloads whose error message contains PATTERN
          Use "all" to retry every failed download

  -t, --threads <N>
          Number of concurrent downloader threads [default: 4]

      --max-retries <N>
          Max retry attempts per download (exponential back-off) [default: 5]

  -v, --verbose
          Enable verbose logging

  -V, --version
          Print version and exit

  -h, --help
          Print help (use --help for full details)
```

### Examples

**1. Download all archived WebAssembly files from a domain:**

```bash
wayback --url wokwi.com --mime "application/wasm" \
        --output-dir wasm_downloads --browser chrome124 --threads 8
```

**2. Download archived CSS stylesheets:**

```bash
wayback --url example.com --mime "text/css" --output-dir ./css
```

**3. Resume an interrupted session:**

```bash
wayback --url wokwi.com --mime "application/wasm" \
        --output-dir wasm_downloads --resume
```

**4. Retry only rate-limited (HTTP 429) failures:**

```bash
wayback --url wokwi.com --mime "application/wasm" \
        --output-dir wasm_downloads --resume --retry-errors "429"
```

**5. Use a full CDX API URL directly:**

```bash
wayback \
  --url "http://web.archive.org/cdx/search/cdx?url=example.com/*&filter=mimetype:image/png&collapse=urlkey&output=json" \
  --mime "image/png"
```

**6. Verbose mode with Firefox 135 impersonation:**

```bash
wayback --url archive.org --mime "image/webp" \
        --browser firefox135 --threads 2 --verbose
```

---

## Shell Completions

The `.deb` package installs completions for **bash**, **zsh**, and **fish**
automatically. If you installed from source, copy them manually:

```bash
# Bash (requires bash-completion package)
sudo cp debian/wayback.bash-completion \
       /usr/share/bash-completion/completions/wayback
source /usr/share/bash-completion/completions/wayback

# Zsh
sudo cp debian/wayback.zsh /usr/share/zsh/vendor-completions/_wayback
# then restart your shell or run: autoload -U compinit && compinit

# Fish
sudo cp debian/wayback.fish /usr/share/fish/vendor_completions.d/wayback.fish
```

Completions cover flags, MIME types, and all browser profile names with
in-shell descriptions.

---

## Man Page

A full manual page is installed by the `.deb` at `/usr/share/man/man1/wayback.1.gz`:

```bash
man wayback
```

To render it locally without installing the package:

```bash
man ./debian/wayback.1
```

---

## Contributing

Please review the [Contribution Guidelines](CONTRIBUTING.md) for details on our
development workflow, packaging conventions, Conventional Commits format, and
CI/CD pipeline.

---

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.

---

> **Disclaimer:** `wayback` is an independent open-source project and is not
> affiliated with, endorsed by, or sponsored by the [Internet Archive](https://archive.org).
