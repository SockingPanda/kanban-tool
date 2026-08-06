#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAURI_CONF="$ROOT/apps/desktop/src-tauri/tauri.conf.json"
DESKTOP_MANIFEST="$ROOT/apps/desktop/src-tauri/Cargo.toml"
JUSTFILE="$ROOT/justfile"
PACKAGE_LAYOUT_SCRIPT="$ROOT/scripts/test-desktop-package-layout.sh"

for path in "$TAURI_CONF" "$DESKTOP_MANIFEST" "$JUSTFILE" "$PACKAGE_LAYOUT_SCRIPT"; do
  [[ -f "$path" ]] || { echo "error: missing expected file: $path" >&2; exit 1; }
done

python3 - "$TAURI_CONF" <<'PYCONF'
import json, pathlib, sys
conf = json.loads(pathlib.Path(sys.argv[1]).read_text())
external = conf.get("bundle", {}).get("externalBin")
if external is not None:
    raise SystemExit(f"error: retired helper externalBin remains configured: {external!r}")
PYCONF

if rg -n 'kanban-(vector-lancedb|graph-oxigraph)|prepare-desktop-helper|test-desktop-helper' \
  "$TAURI_CONF" "$DESKTOP_MANIFEST" "$JUSTFILE" "$PACKAGE_LAYOUT_SCRIPT"; then
  echo "error: retired helper packaging references remain" >&2
  exit 1
fi

echo "ok: desktop package config has no retired helper sidecars"
