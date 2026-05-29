# fish completion for wayback(1)
# Installed to: /usr/share/fish/vendor_completions.d/wayback.fish
# SPDX-License-Identifier: MIT

# Disable file completion by default
complete -c wayback -f

# ── Required arguments ───────────────────────────────────────────────────────
complete -c wayback -s u -l url \
    -d "Target domain (e.g. wokwi.com) or full CDX search URL" -r

complete -c wayback -s m -l mime \
    -d "MIME type to filter on CDX API" -r \
    -a "application/wasm\t'WebAssembly'
        application/javascript\t'JavaScript'
        application/json\t'JSON'
        text/css\t'CSS'
        text/html\t'HTML'
        image/png\t'PNG images'
        image/jpeg\t'JPEG images'
        image/svg+xml\t'SVG images'
        image/webp\t'WebP images'
        font/woff2\t'WOFF2 fonts'
        font/woff\t'WOFF fonts'
        application/octet-stream\t'Binary'"

# ── Optional arguments ────────────────────────────────────────────────────────
complete -c wayback -s o -l output-dir \
    -d "Output directory for downloaded assets" -r -F

complete -c wayback -s b -l browser \
    -d "Browser profile to impersonate" -r \
    -a "chrome\t'Latest Chrome'
        firefox\t'Latest Firefox'
        edge\t'Latest Edge'
        safari\t'Latest Safari'
        tor\t'Tor Browser'
        chrome124\t'Chrome 124'
        chrome131\t'Chrome 131'
        firefox135\t'Firefox 135'
        safari17_2_1\t'Safari 17.2.1'
        safari18\t'Safari 18'"

complete -c wayback -s r -l resume \
    -d "Resume an interrupted download session"

complete -c wayback -l retry-errors \
    -d "Retry failed downloads matching pattern" -r \
    -a "all\t'Retry every failure'
        HTTP\t'Retry HTTP errors'
        timeout\t'Retry timeouts'
        ssl\t'Retry SSL errors'"

complete -c wayback -s t -l threads \
    -d "Number of concurrent downloader threads" -r \
    -a "1 2 4 8 16 32"

complete -c wayback -l max-retries \
    -d "Max retry attempts per download" -r \
    -a "1 3 5 10"

complete -c wayback -s v -l verbose \
    -d "Enable verbose output logging"

complete -c wayback -s V -l version \
    -d "Print version information and exit"

complete -c wayback -s h -l help \
    -d "Print help information"
