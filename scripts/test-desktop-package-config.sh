#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAURI_CONF="$ROOT/apps/desktop/src-tauri/tauri.conf.json"
DESKTOP_MANIFEST="$ROOT/apps/desktop/src-tauri/Cargo.toml"
DESKTOP_CONFIG="$ROOT/apps/desktop/src-tauri/src/desktop_config.rs"
JUSTFILE="$ROOT/justfile"
GITIGNORE="$ROOT/.gitignore"
PACKAGE_LAYOUT_SCRIPT="$ROOT/scripts/test-desktop-package-layout.sh"
SIDECAR_PREP_SCRIPT="$ROOT/scripts/prepare-desktop-sidecar.sh"

for path in "$TAURI_CONF" "$DESKTOP_MANIFEST" "$DESKTOP_CONFIG" "$JUSTFILE" "$GITIGNORE" "$PACKAGE_LAYOUT_SCRIPT" "$SIDECAR_PREP_SCRIPT"; do
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
  .build.frontendDist == "../bootstrap"
  and (.build.devUrl? // null) == null
  and .build.beforeDevCommand == "just desktop-dev-prep"
  and (.bundle.resources | type == "object")
  and .bundle.resources["../../web/dist/"] == "web/"
  and .bundle.resources["bin/kanban"] == "kanban"
' "$TAURI_CONF" >/dev/null || {
  echo "error: Tauri config must use static bootstrap in dev/package and map Web/kanban resources" >&2
  exit 1
}

rg -n 'resolve_sidecar_path|resource_dir\.join\("kanban"\)' "$DESKTOP_CONFIG" >/dev/null || {
  echo "error: Desktop host must resolve the packaged kanban sidecar at resource_dir/kanban" >&2
  exit 1
}

rg -n 'DEST=.*src-tauri/bin/kanban|release/kanban' "$SIDECAR_PREP_SCRIPT" >/dev/null || {
  echo "error: sidecar preparation must copy release/kanban to src-tauri/bin/kanban" >&2
  exit 1
}

rg -n 'debug/kanban|prepare-desktop-sidecar\.sh dev' "$SIDECAR_PREP_SCRIPT" "$JUSTFILE" >/dev/null || {
  echo "error: Desktop dev prep must build and resolve the debug kanban sidecar" >&2
  exit 1
}

grep -Fxq 'apps/desktop/src-tauri/bin/kanban' "$GITIGNORE" || {
  echo "error: generated Desktop sidecar must be ignored at apps/desktop/src-tauri/bin/kanban" >&2
  exit 1
}

desktop_check_block="$(sed -n '/^desktop-check:/,/^desktop-build:/p' "$JUSTFILE")"
for required in 'just web-build' 'just web-artifact-check' 'cargo build --locked -p kanban-cli --release' 'scripts/prepare-desktop-sidecar.sh'; do
  if ! grep -Fq -- "$required" <<<"$desktop_check_block"; then
    echo "error: desktop-check is missing prerequisite: $required" >&2
    exit 1
  fi
done

desktop_dev_block="$(sed -n '/^desktop-dev-prep:/,/^desktop-check:/p' "$JUSTFILE")"
for required in 'just web-build' 'just web-artifact-check' 'cargo build --locked -p kanban-cli' 'scripts/prepare-desktop-sidecar.sh dev'; do
  if ! grep -Fq -- "$required" <<<"$desktop_dev_block"; then
    echo "error: desktop-dev-prep is missing startup step: $required" >&2
    exit 1
  fi
done

desktop_build_block="$(sed -n '/^desktop-build:/,/^desktop-package:/p' "$JUSTFILE")"
for required in 'just web-build' 'just web-artifact-check' 'cargo build --locked -p kanban-cli --release' 'scripts/prepare-desktop-sidecar.sh' 'tauri build'; do
  if ! grep -Fq -- "$required" <<<"$desktop_build_block"; then
    echo "error: desktop-build is missing sidecar/package step: $required" >&2
    exit 1
  fi
done

if rg -n 'kanban-(vector-lancedb|graph-oxigraph)|prepare-desktop-helper|test-desktop-helper' \
  "$TAURI_CONF" "$DESKTOP_MANIFEST" "$JUSTFILE" "$PACKAGE_LAYOUT_SCRIPT"; then
  echo "error: retired helper packaging references remain" >&2
  exit 1
fi

echo "ok: desktop package config has no retired helper sidecars"
