#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source="$root/target/release/maintenance"
dest_dir="$root/skill/doc-maintenance/bin"
dest="$dest_dir/maintenance"

if [[ ! -f "$source" ]]; then
  echo "Release binary not found. Run 'cargo build --release' first." >&2
  exit 1
fi

mkdir -p "$dest_dir"
cp "$source" "$dest"
chmod +x "$dest"
echo "Copied $source to $dest"
