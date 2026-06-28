#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAURI_CONF="$ROOT/apps/desktop/src-tauri/tauri.conf.json"
DESKTOP_MANIFEST="$ROOT/apps/desktop/src-tauri/Cargo.toml"
JUSTFILE="$ROOT/justfile"
PREPARE_SCRIPT="$ROOT/scripts/prepare-desktop-helper-binaries.sh"
SIDECAR_SCRIPT="$ROOT/scripts/test-desktop-helper-sidecars.sh"
PACKAGE_LAYOUT_SCRIPT="$ROOT/scripts/test-desktop-package-layout.sh"

for path in "$TAURI_CONF" "$DESKTOP_MANIFEST" "$JUSTFILE" "$PREPARE_SCRIPT" "$SIDECAR_SCRIPT" "$PACKAGE_LAYOUT_SCRIPT"; do
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

grep -A3 '^desktop-check:' "$JUSTFILE" | grep -Fq 'scripts/prepare-desktop-helper-binaries.sh' || {
  echo "error: desktop-check must prepare helper sidecars before cargo check" >&2
  exit 1
}

grep -A4 '^desktop-check:' "$JUSTFILE" | grep -Fq 'scripts/test-desktop-helper-sidecars.sh' || {
  echo "error: desktop-check must verify prepared helper sidecars" >&2
  exit 1
}

grep -A3 '^desktop-package:' "$JUSTFILE" | grep -Fq 'scripts/prepare-desktop-helper-binaries.sh' || {
  echo "error: desktop-package must prepare helper sidecars before tauri build" >&2
  exit 1
}

grep -A4 '^desktop-package:' "$JUSTFILE" | grep -Fq 'scripts/test-desktop-helper-sidecars.sh' || {
  echo "error: desktop-package must verify prepared helper sidecars before tauri build" >&2
  exit 1
}

python3 - "$JUSTFILE" <<'PYJUST'
import pathlib, sys
lines = pathlib.Path(sys.argv[1]).read_text().splitlines()
try:
    release = lines.index("release:")
except ValueError as exc:
    raise SystemExit("error: missing release recipe") from exc
steps = [line.strip() for line in lines[release + 1:] if line.startswith("    just ")]
for expected in ["just desktop-package-config", "just desktop-package", "just desktop-package-layout"]:
    if expected not in steps:
        raise SystemExit(f"error: release recipe missing {expected}")
if steps.index("just desktop-package-layout") < steps.index("just desktop-package"):
    raise SystemExit("error: desktop-package-layout must run after desktop-package in release")
PYJUST

echo "ok: desktop package config points at helper sidecars"
