#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SIDECAR_DIR="$ROOT/apps/desktop/src-tauri/binaries"
TARGET_TRIPLE="$(rustc -vV | awk '/^host:/ { print $2 }')"
HELPERS=("kanban-vector-lancedb" "kanban-graph-oxigraph")

[[ -n "$TARGET_TRIPLE" ]] || { echo "error: failed to detect rust target triple" >&2; exit 1; }
[[ -d "$SIDECAR_DIR" ]] || { echo "error: missing desktop sidecar dir: $SIDECAR_DIR" >&2; exit 1; }

for helper in "${HELPERS[@]}"; do
  path="$SIDECAR_DIR/$helper-$TARGET_TRIPLE"
  [[ -f "$path" ]] || { echo "error: missing prepared sidecar: $path" >&2; exit 1; }
  [[ -x "$path" ]] || { echo "error: prepared sidecar is not executable: $path" >&2; exit 1; }
  case "$(basename "$path")" in
    "$helper-$TARGET_TRIPLE") ;;
    *) echo "error: sidecar has unexpected filename: $path" >&2; exit 1 ;;
  esac
done

echo "ok: prepared desktop helper sidecars exist for $TARGET_TRIPLE"
