#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAURI_CONF="$ROOT/apps/desktop/src-tauri/tauri.conf.json"
DESKTOP_MANIFEST="$ROOT/apps/desktop/src-tauri/Cargo.toml"
DESKTOP_CONFIG="$ROOT/apps/desktop/src-tauri/src/desktop_config.rs"
DESKTOP_PACKAGE="$ROOT/apps/desktop/package.json"
ROOT_PACKAGE="$ROOT/package.json"
JUSTFILE="$ROOT/justfile"
GITIGNORE="$ROOT/.gitignore"
PACKAGE_LAYOUT_SCRIPT="$ROOT/scripts/test-desktop-package-layout.sh"
PACKAGED_SMOKE_SCRIPT="$ROOT/scripts/test-desktop-packaged-smoke.sh"
SIDECAR_PREP_SCRIPT="$ROOT/scripts/prepare-desktop-sidecar.sh"

for path in "$TAURI_CONF" "$DESKTOP_MANIFEST" "$DESKTOP_CONFIG" "$DESKTOP_PACKAGE" "$ROOT_PACKAGE" "$JUSTFILE" "$GITIGNORE" "$PACKAGE_LAYOUT_SCRIPT" "$PACKAGED_SMOKE_SCRIPT" "$SIDECAR_PREP_SCRIPT"; do
  [[ -f "$path" ]] || { echo "error: missing expected file: $path" >&2; exit 1; }
done
[[ -x "$PACKAGED_SMOKE_SCRIPT" ]] || { echo "error: packaged Desktop smoke script must be executable" >&2; exit 1; }

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

jq -e '
  (.bundle.linux.deb.depends // []) | index("xdg-utils") != null
' "$TAURI_CONF" >/dev/null || {
  echo "error: Desktop deb must depend on xdg-utils for the fixed /usr/bin/xdg-open launcher" >&2
  exit 1
}

csp="$(jq -r '.app.security.csp // ""' "$TAURI_CONF")"
for forbidden in "unsafe-inline" "unsafe-eval"; do
  if [[ "$csp" == *"$forbidden"* ]]; then
    echo "error: Desktop CSP must not permit $forbidden" >&2
    exit 1
  fi
done
for required_csp in "base-uri 'none'" "object-src 'none'" "frame-ancestors 'none'" "form-action 'none'" "script-src 'self'" "connect-src 'self'"; do
  if [[ "$csp" != *"$required_csp"* ]]; then
    echo "error: Desktop CSP is missing required directive: $required_csp" >&2
    exit 1
  fi
done

[[ ! -d "$ROOT/apps/desktop/src" ]] || {
  echo "error: retired Desktop React source tree remains" >&2
  exit 1
}

jq -e '
  (.scripts | keys) == ["tauri"]
  and (.devDependencies | keys) == ["@tauri-apps/cli"]
  and ((.dependencies // {}) | length) == 0
' "$DESKTOP_PACKAGE" >/dev/null || {
  echo "error: Desktop package must be Tauri-only" >&2
  exit 1
}

jq -e '(.scripts["desktop:typecheck"]? // null) == null and (.scripts["desktop:test"]? // null) == null and .scripts["desktop:build"] == "just desktop-package"' "$ROOT_PACKAGE" >/dev/null || {
  echo "error: root package scripts must expose only the Tauri desktop package build" >&2
  exit 1
}

rg -n 'resolve_sidecar_path|resource_dir\.join\("kanban"\)' "$DESKTOP_CONFIG" >/dev/null || {
  echo "error: Desktop host must resolve the packaged kanban sidecar at resource_dir/kanban" >&2
  exit 1
}

rg -n 'DEST=.*src-tauri/bin/kanban|release/kanban|debug/kanban' "$SIDECAR_PREP_SCRIPT" >/dev/null || {
  echo "error: sidecar preparation must copy release/debug kanban to src-tauri/bin/kanban" >&2
  exit 1
}

rg -n 'debug/kanban|prepare-desktop-sidecar\.sh dev' "$SIDECAR_PREP_SCRIPT" "$JUSTFILE" >/dev/null || {
  echo "error: Desktop dev prep must build and resolve the debug kanban sidecar" >&2
  exit 1
}

cargo_build_hint_pattern='build[[:space:]]+--locked[[:space:]]+-p[[:space:]]+kanban-cli'
rg -n 'missing \$PROFILE_LABEL CLI sidecar|BUILD_HINT=.*'"$cargo_build_hint_pattern" "$SIDECAR_PREP_SCRIPT" >/dev/null || {
  echo "error: sidecar preparation must report a profile-specific build hint" >&2
  exit 1
}

grep -Fxq 'apps/desktop/src-tauri/bin/kanban' "$GITIGNORE" || {
  echo "error: generated Desktop sidecar must be ignored at apps/desktop/src-tauri/bin/kanban" >&2
  exit 1
}

desktop_check_block="$(sed -n '/^desktop-check:/,/^desktop-build:/p' "$JUSTFILE")"
cargo_word=cargo
for required in 'just web-build' 'just web-artifact-check' "${cargo_word} build --locked -p kanban-cli --release" 'scripts/prepare-desktop-sidecar.sh' "${cargo_word} check --locked -p kanban-desktop --tests"; do
  if ! grep -Fq -- "$required" <<<"$desktop_check_block"; then
    echo "error: desktop-check is missing prerequisite: $required" >&2
    exit 1
  fi
done

desktop_dev_block="$(sed -n '/^desktop-dev-prep:/,/^desktop-check:/p' "$JUSTFILE")"
for required in 'just web-build' 'just web-artifact-check' "${cargo_word} build --locked -p kanban-cli" 'scripts/prepare-desktop-sidecar.sh dev'; do
  if ! grep -Fq -- "$required" <<<"$desktop_dev_block"; then
    echo "error: desktop-dev-prep is missing startup step: $required" >&2
    exit 1
  fi
done

generic_tauri_override="TAURI_CONFIG='{\"bundle\":{\"resources\":[]}}'"
if [[ "$(rg -F "$generic_tauri_override" "$JUSTFILE" | wc -l)" -lt 5 ]]; then
  echo "error: generic workspace check/test/clippy/doc gates must disable package-only Tauri resources" >&2
  exit 1
fi
if grep -Fq 'TAURI_CONFIG=' <<<"$desktop_check_block"; then
  echo "error: desktop-check must exercise the real bundled resource paths" >&2
  exit 1
fi

desktop_build_block="$(sed -n '/^desktop-build:/,/^desktop-package:/p' "$JUSTFILE")"
for required in 'just web-build' 'just web-artifact-check' "${cargo_word} build --locked -p kanban-cli --release" 'scripts/prepare-desktop-sidecar.sh' 'tauri build'; do
  if ! grep -Fq -- "$required" <<<"$desktop_build_block"; then
    echo "error: desktop-build is missing sidecar/package step: $required" >&2
    exit 1
  fi
done

desktop_smoke_block="$(sed -n '/^desktop-packaged-smoke:/,/^[^[:space:]]/p' "$JUSTFILE")"
grep -Fq 'just desktop-build' <<<"$desktop_smoke_block" || {
  echo "error: desktop-packaged-smoke must build the packaged Deb before smoke" >&2
  exit 1
}
grep -Fq 'scripts/test-desktop-packaged-smoke.sh' <<<"$desktop_smoke_block" || {
  echo "error: desktop-packaged-smoke must invoke the packaged WebKitGTK smoke script" >&2
  exit 1
}

if rg -n 'kanban-(vector-lancedb|graph-oxigraph)|prepare-desktop-helper|test-desktop-helper' \
  "$TAURI_CONF" "$DESKTOP_MANIFEST" "$JUSTFILE" "$PACKAGE_LAYOUT_SCRIPT"; then
  echo "error: retired helper packaging references remain" >&2
  exit 1
fi

echo "ok: desktop package config has no retired helper sidecars"
