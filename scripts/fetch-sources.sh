#!/usr/bin/env bash
# Fetches the research sources listed in docs/circuit/sources/manifest.tsv into a local
# cache (default ~/.cache/ca72-sources), checking each file's sha256, and exits non-zero if
# any could not be fetched or does not match. The sources are not committed: several sites
# ask that their scans not be re-hosted, and vendor SPICE models may not be redistributed
# (docs/circuit/sources.md).
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
cache="${CA72_SOURCES:-$HOME/.cache/ca72-sources}"
mkdir -p "$cache"

sha() {
    if command -v sha256sum > /dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1
}

failed=0
while IFS=$'\t' read -r path url sum _; do
    case "$url" in http*) ;; *) continue ;; esac
    dest="$cache/$path"
    mkdir -p "$(dirname "$dest")"
    if [ ! -f "$dest" ] || [ "$(sha "$dest")" != "$sum" ]; then
        if ! curl -fsSL -A "Mozilla/5.0" -o "$dest" "$url"; then
            echo "FAILED $url"
            rm -f "$dest"
            failed=$((failed + 1))
            continue
        fi
    fi
    got="$(sha "$dest")"
    if [ "$got" != "$sum" ]; then
        echo "MISMATCH $path ($got)"
        failed=$((failed + 1))
    else
        echo "ok $path"
    fi
done < <(tail -n +2 "$here/docs/circuit/sources/manifest.tsv")

if [ "$failed" -gt 0 ]; then
    echo "fetch-sources.sh: $failed of the sources failed" >&2
    exit 1
fi
