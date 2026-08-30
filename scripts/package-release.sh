#!/usr/bin/env sh
set -eu

target="${1:?target is required}"
version="${2:?version is required}"
output_dir="${3:?output directory is required}"

case "$target" in
    aarch64-apple-darwin) arch=aarch64 ;;
    x86_64-apple-darwin) arch=x86_64 ;;
    *) echo "ERROR: unsupported release target: $target" >&2; exit 1 ;;
esac

binary="${CARGO_TARGET_DIR:?CARGO_TARGET_DIR is required}/release/shelve"
[ -x "$binary" ] || { echo "ERROR: missing release binary: $binary" >&2; exit 1; }
actual_version="$($binary -V)"
[ "$actual_version" = "shelve $version" ] || {
    echo "ERROR: expected shelve $version, got $actual_version" >&2
    exit 1
}

mkdir -p "$output_dir"
stage_dir="$(mktemp -d "${TMPDIR:-/tmp}/shelve-package.XXXXXX")"
trap 'rm -rf "$stage_dir"' EXIT HUP INT TERM
cp "$binary" "$stage_dir/shelve"
cp config.example.toml "$stage_dir/config.example.toml"

archive="$output_dir/shelve-macos-$arch.tar.gz"
tar -czf "$archive" -C "$stage_dir" shelve config.example.toml
shasum -a 256 "$archive" > "$archive.sha256"
echo "$archive"
