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
    "just check-windows-p kanban-local",
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

echo "ok: desktop package config points at helper sidecars"
