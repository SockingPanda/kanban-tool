#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAURI_CONF="$ROOT/apps/desktop/src-tauri/tauri.conf.json"
DESKTOP_MANIFEST="$ROOT/apps/desktop/src-tauri/Cargo.toml"
JUSTFILE="$ROOT/justfile"
PREPARE_SCRIPT="$ROOT/scripts/prepare-desktop-helper-binaries.sh"

for path in "$TAURI_CONF" "$DESKTOP_MANIFEST" "$JUSTFILE" "$PREPARE_SCRIPT"; do
  [[ -f "$path" ]] || { echo "error: missing expected file: $path" >&2; exit 1; }
done

python3 - "$TAURI_CONF" <<'PYCONF'
import json, pathlib, sys
conf = json.loads(pathlib.Path(sys.argv[1]).read_text())
external = conf.get("bundle", {}).get("externalBin")
expected = [
    "binaries/kanban-vector-lancedb",
    "binaries/kanban-graph-oxigraph",
]
if external != expected:
    raise SystemExit(f"error: unexpected bundle.externalBin: {external!r}")
PYCONF

if grep -Fq 'kanban-server = { workspace = true, features = ["vector-lancedb"] }' "$DESKTOP_MANIFEST"; then
  echo "error: desktop manifest still enables kanban-server vector-lancedb feature" >&2
  exit 1
fi

grep -Fq 'kanban-server.workspace = true' "$DESKTOP_MANIFEST" || {
  echo "error: desktop manifest should depend on kanban-server without helper feature flags" >&2
  exit 1
}

for helper in kanban-vector-lancedb kanban-graph-oxigraph; do
  grep -Fq "$helper" "$TAURI_CONF" || { echo "error: tauri config missing $helper externalBin" >&2; exit 1; }
  grep -Fq "$helper" "$PREPARE_SCRIPT" || { echo "error: prepare script missing $helper" >&2; exit 1; }
done

grep -A2 '^desktop-check:' "$JUSTFILE" | grep -Fq 'scripts/prepare-desktop-helper-binaries.sh' || {
  echo "error: desktop-check must prepare helper sidecars before cargo check" >&2
  exit 1
}

grep -A2 '^desktop-package:' "$JUSTFILE" | grep -Fq 'scripts/prepare-desktop-helper-binaries.sh' || {
  echo "error: desktop-package must prepare helper sidecars before tauri build" >&2
  exit 1
}

echo "ok: desktop package layout includes bundled helper sidecars"
