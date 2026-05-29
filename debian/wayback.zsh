#compdef wayback
# zsh completion for wayback(1)
# Installed to: /usr/share/zsh/vendor-completions/_wayback
# SPDX-License-Identifier: MIT

_wayback() {
    local -a browsers mimes

    browsers=(
        'chrome:Latest stable Chrome'
        'firefox:Latest stable Firefox'
        'edge:Latest Microsoft Edge'
        'safari:Latest Safari'
        'tor:Tor Browser'
        'chrome124:Chrome 124'
        'chrome131:Chrome 131'
        'firefox135:Firefox 135'
        'safari17_2_1:Safari 17.2.1'
        'safari18:Safari 18'
    )

    mimes=(
        'application/wasm:WebAssembly binaries'
        'application/javascript:JavaScript files'
        'application/json:JSON files'
        'text/css:CSS stylesheets'
        'text/html:HTML documents'
        'image/png:PNG images'
        'image/jpeg:JPEG images'
        'image/svg+xml:SVG images'
        'image/webp:WebP images'
        'font/woff2:WOFF2 fonts'
        'font/woff:WOFF fonts'
        'application/octet-stream:Binary blobs'
    )

    _arguments -s -S \
        '(-u --url)'{-u,--url}'[Target domain or full CDX URL]:domain or CDX URL:()' \
        '(-m --mime)'{-m,--mime}'[MIME type filter for CDX API]:MIME type:->mime' \
        '(-o --output-dir)'{-o,--output-dir}'[Output directory for downloads]:directory:_files -/' \
        '(-b --browser)'{-b,--browser}'[Browser profile to impersonate]:browser profile:->browser' \
        '(-r --resume)'{-r,--resume}'[Resume interrupted download session]' \
        '--retry-errors[Retry failed downloads matching pattern]:pattern:(all HTTP timeout ssl)' \
        '(-t --threads)'{-t,--threads}'[Number of concurrent threads]:count:(1 2 4 8 16 32)' \
        '--max-retries[Max retry attempts per download]:count:(1 3 5 10)' \
        '(-v --verbose)'{-v,--verbose}'[Enable verbose logging]' \
        '(-V --version)'{-V,--version}'[Print version and exit]' \
        '(-h --help)'{-h,--help}'[Print help information]'

    case $state in
        mime)
            _describe 'MIME type' mimes
            ;;
        browser)
            _describe 'browser profile' browsers
            ;;
    esac
}

_wayback "$@"
