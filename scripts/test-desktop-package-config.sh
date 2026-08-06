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

python3 - "$JUSTFILE" "$ROOT/scripts/release-cohort.sh" <<'PYJUST'
import pathlib, sys
lines = pathlib.Path(sys.argv[1]).read_text().splitlines()
try:
    release = lines.index("release:")
except ValueError as exc:
    raise SystemExit("error: missing release recipe") from exc
if lines[release + 1 : release + 2] != ["    scripts/release-cohort.sh"]:
    raise SystemExit("error: release recipe must enter scripts/release-cohort.sh")
steps = [
    line.strip()
    for line in pathlib.Path(sys.argv[2]).read_text().splitlines()
    if line.startswith("just ")
]
expected_steps = [
    "just affected-self-test",
    "just schema-contract",
    "just audit",
    "just rust-full",
    "just check-windows-p kanban-server",
    "just projection-release-cohort",
    "just bench-check",
    "just target-tools",
    "just cli-package",
    "just cli-package-layout",
    "just desktop-package-config",
    "just desktop-package",
    "just desktop-package-layout",
    "just smoke",
    "just diff-check",
]
if steps != expected_steps:
    raise SystemExit(
        "error: release cohort wrapper must execute the exact canonical recipe sequence; "
        f"got {steps!r}"
    )
PYJUST

echo "ok: desktop package config has no retired helper sidecars"
