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

command -v jq >/dev/null 2>&1 || {
  echo "error: jq is required" >&2
  exit 1
}

if ! jq -e '(.bundle.externalBin? // null) == null' "$TAURI_CONF" >/dev/null; then
  external="$(jq -c '.bundle.externalBin' "$TAURI_CONF")"
  echo "error: retired helper externalBin remains configured: $external" >&2
  exit 1
fi

jq -e '
  .build.frontendDist == "../dist"
  and (.bundle.resources | type == "object")
  and .bundle.resources["../../web/dist/"] == "web/"
' "$TAURI_CONF" >/dev/null || {
  echo "error: Tauri package must retain frontendDist ../dist and map ../../web/dist/ to web/" >&2
  exit 1
}

if rg -n 'kanban-(vector-lancedb|graph-oxigraph)|prepare-desktop-helper|test-desktop-helper' \
  "$TAURI_CONF" "$DESKTOP_MANIFEST" "$JUSTFILE" "$PACKAGE_LAYOUT_SCRIPT"; then
  echo "error: retired helper packaging references remain" >&2
  exit 1
fi

echo "ok: desktop package config has no retired helper sidecars"
