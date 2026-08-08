#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAURI_CONF="$ROOT/apps/desktop/src-tauri/tauri.conf.json"
DESKTOP_MANIFEST="$ROOT/apps/desktop/src-tauri/Cargo.toml"
JUSTFILE="$ROOT/justfile"
PACKAGE_LAYOUT_SCRIPT="$ROOT/scripts/test-desktop-package-layout.sh"
SIDECAR_PREP_SCRIPT="$ROOT/scripts/prepare-desktop-sidecar.sh"
DESKTOP_LIB="$ROOT/apps/desktop/src-tauri/src/lib.rs"

for path in "$TAURI_CONF" "$DESKTOP_MANIFEST" "$JUSTFILE" "$PACKAGE_LAYOUT_SCRIPT" \
  "$SIDECAR_PREP_SCRIPT" "$DESKTOP_LIB"; do
  [[ -f "$path" ]] || { echo "error: missing expected file: $path" >&2; exit 1; }
done

command -v jq >/dev/null 2>&1 || {
  echo "error: jq is required" >&2
  exit 1
}

if ! jq -e '(.bundle.externalBin? // null) == null' "$TAURI_CONF" >/dev/null; then
  external="$(jq -c '.bundle.externalBin' "$TAURI_CONF")"
  echo "error: Desktop must package the kanban sidecar as a resource, not externalBin: $external" >&2
  exit 1
fi

jq -e '
  .build.frontendDist == "../../web/dist"
  and (.bundle.resources | type == "object")
  and .bundle.resources["../../web/dist/"] == "web/"
  and .bundle.resources["bin/kanban"] == "kanban"
' "$TAURI_CONF" >/dev/null || {
  echo "error: Tauri package must map the Web artifact to web/ and the sidecar to resource root kanban" >&2
  exit 1
}

jq -e '
  (.build.beforeDevCommand | contains("@kanban-tool/web"))
  and (.build.beforeBuildCommand | contains("@kanban-tool/web"))
  and .build.devUrl == "http://127.0.0.1:8721/app/"
' "$TAURI_CONF" >/dev/null || {
  echo "error: Desktop Tauri build must use the browser-first Web artifact and fixed host URL" >&2
  exit 1
}

rg -n 'resource_dir\.join\("kanban"\)' "$DESKTOP_LIB" >/dev/null || {
  echo "error: Desktop runtime must resolve the bundled sidecar at resource_dir/kanban" >&2
  exit 1
}

rg -n 'DEST=.*src-tauri/bin/kanban|release/kanban' \
  "$SIDECAR_PREP_SCRIPT" >/dev/null || {
  echo "error: sidecar preparation must copy release/kanban to stable bin/kanban" >&2
  exit 1
}

desktop_check_recipe="$(awk '/^desktop-check:/{capture=1; next} capture && /^[[:alnum:]_-]+:/{exit} capture{print}' "$JUSTFILE")"
for required in \
  'pnpm --filter @kanban-tool/desktop typecheck' \
  'pnpm --filter @kanban-tool/desktop test'; do
  grep -Fq "$required" <<<"$desktop_check_recipe" || {
    echo "error: desktop-check recipe is missing $required" >&2
    exit 1
  }
done

desktop_recipe="$(awk '/^desktop-build:/{capture=1; next} capture && /^[[:alnum:]_-]+:/{exit} capture{print}' "$JUSTFILE")"
for required in \
  'cargo build --locked -p kanban-cli --release' \
  'scripts/prepare-desktop-sidecar.sh' \
  'pnpm --filter @kanban-tool/desktop tauri build'; do
  grep -Fq "$required" <<<"$desktop_recipe" || {
    echo "error: desktop-build recipe is missing $required" >&2
    exit 1
  }
done

if [[ "$desktop_recipe" != *'cargo build --locked -p kanban-cli --release'*'scripts/prepare-desktop-sidecar.sh'*'pnpm --filter @kanban-tool/desktop tauri build'* ]]; then
  echo "error: desktop-build must release-build, prepare, then invoke Tauri" >&2
  exit 1
fi

if rg -n 'kanban-(vector-lancedb|graph-oxigraph)|prepare-desktop-helper|test-desktop-helper' \
  "$TAURI_CONF" "$DESKTOP_MANIFEST" "$JUSTFILE" "$PACKAGE_LAYOUT_SCRIPT"; then
  echo "error: retired helper packaging references remain" >&2
  exit 1
fi

echo "ok: desktop package config has no retired helper sidecars"
