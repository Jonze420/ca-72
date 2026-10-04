#!/usr/bin/env bash
# Packs the sources of the crates Cargo.lock takes from git repositories rather than from
# crates.io, each at the commit it pins, into target/packages/CA-72-<version>-git-sources.tar.gz.
# Each release carries it beside the installers, so that the source of everything built into
# the plug-ins stays available with them (THIRD-PARTY-NOTICES.txt, decisions.md R25). Needs git,
# tar and network access to the repositories.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -n 1)"
name="CA-72-$version-git-sources"
work="$root/target/git-sources"
rm -rf "$work"
mkdir -p "$work/$name" "$root/target/packages"

{
    echo "The sources of the crates the CA-72 $version's Cargo.lock takes from git repositories,"
    echo "each at the commit it pins (the others are on crates.io at the versions Cargo.lock names)."
    echo
} > "$work/$name/README.txt"

sed -n 's/^source = "git+\([^?#]*\)[^#]*#\([0-9a-f]\{40\}\)"$/\1 \2/p' "$root/Cargo.lock" | sort -u |
    while read -r url rev; do
        dir="$(basename "$url" .git)-${rev:0:12}"
        git init -q --bare "$work/repo.git"
        git -C "$work/repo.git" fetch -q --depth 1 "$url" "$rev"
        mkdir "$work/$name/$dir"
        git -C "$work/repo.git" archive "$rev" | tar -x -C "$work/$name/$dir"
        rm -rf "$work/repo.git"
        echo "$dir  $url  $rev" >> "$work/$name/README.txt"
    done

tar -czf "$root/target/packages/$name.tar.gz" -C "$work" "$name"
echo "$root/target/packages/$name.tar.gz"
