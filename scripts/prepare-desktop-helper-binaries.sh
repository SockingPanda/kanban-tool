#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HELPER_BINARIES=("kanban-vector-lancedb" "kanban-graph-oxigraph")
LOCK="$ROOT/scripts/cargo-build-lock.sh"
TARGET_ROOT="${KANBAN_CARGO_TARGET_ROOT:-$HOME/.cache/kanban-tool/cargo-target}"
SIDECAR_DIR="$ROOT/apps/desktop/src-tauri/binaries"
TARGET_TRIPLE="$(rustc -vV | awk '/^host:/ { print $2 }')"

while [[ "$TARGET_ROOT" != "/" && "$TARGET_ROOT" == */ ]]; do
  TARGET_ROOT="${TARGET_ROOT%/}"
done
TARGET_DIR="$TARGET_ROOT/release"

command -v cargo >/dev/null 2>&1 || { echo "error: cargo is required" >&2; exit 1; }
command -v rustc >/dev/null 2>&1 || { echo "error: rustc is required" >&2; exit 1; }
[[ -n "$TARGET_TRIPLE" ]] || { echo "error: failed to detect rust target triple" >&2; exit 1; }

(
  cd "$ROOT"
  "$LOCK" -- cargo build -p kanban-vector-lancedb -p kanban-graph-oxigraph --release --bins
)

mkdir -p "$SIDECAR_DIR"
for helper in "${HELPER_BINARIES[@]}"; do
  src="$TARGET_DIR/$helper"
  dest="$SIDECAR_DIR/$helper-$TARGET_TRIPLE"
  [[ -x "$src" ]] || { echo "error: expected helper binary not found: $src" >&2; exit 1; }
  install -Dm755 "$src" "$dest"
done

echo "ok: prepared desktop helper sidecars for $TARGET_TRIPLE"
