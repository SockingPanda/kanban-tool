#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
TARGET_DIR="$("$LOCK" --print-target-dir)"
SOURCE="$TARGET_DIR/release/kanban"
DEST="$ROOT/apps/desktop/src-tauri/bin/kanban"

[[ -x "$SOURCE" ]] || {
  echo "error: missing release CLI sidecar $SOURCE; run cargo build -p kanban-cli --release first" >&2
  exit 1
}

mkdir -p "$(dirname "$DEST")"
cp -- "$SOURCE" "$DEST"
chmod 0755 "$DEST"
printf '%s\n' "$DEST"
