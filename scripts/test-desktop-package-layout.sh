#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
DEB_DIR="$("$LOCK" --print-target-dir)/release/bundle/deb"

deb_path="${1:-}"
if [[ -z "$deb_path" ]]; then
  deb_path="$(find "$DEB_DIR" -maxdepth 1 -name 'Kanban Tool_*.deb' -type f -printf '%T@ %p\n' 2>/dev/null | sort -nr | awk 'NR == 1 { sub(/^[^ ]+ /, ""); print }')"
fi

[[ -n "$deb_path" && -f "$deb_path" ]] || {
  echo "error: no Desktop deb found; run just desktop-package first" >&2
  exit 1
}

command -v dpkg-deb >/dev/null 2>&1 || { echo "error: dpkg-deb is required" >&2; exit 1; }
contents="$(dpkg-deb -c "$deb_path")"

grep -Eq '(^|[[:space:]])(\./)?usr/bin/kanban-desktop$' <<<"$contents" || {
  echo "error: Desktop deb is missing usr/bin/kanban-desktop: $deb_path" >&2
  exit 1
}

if grep -Eq '(^|[[:space:]])(\./)?usr/bin/kanban$' <<<"$contents"; then
  echo "error: Desktop deb unexpectedly contains standalone CLI usr/bin/kanban" >&2
  exit 1
fi

manifest_set=0
map_set=0
[[ "${KANBAN_RELEASE_SOURCE_MANIFEST+x}" == "x" ]] && manifest_set=1
[[ "${KANBAN_RELEASE_SOURCE_MAP+x}" == "x" ]] && map_set=1
if [[ "$manifest_set" -ne "$map_set" ]]; then
  if [[ "$manifest_set" -eq 1 ]]; then
    echo "error: release provenance requires KANBAN_RELEASE_SOURCE_MAP when KANBAN_RELEASE_SOURCE_MANIFEST is set" >&2
  else
    echo "error: release provenance requires KANBAN_RELEASE_SOURCE_MANIFEST when KANBAN_RELEASE_SOURCE_MAP is set" >&2
  fi
  exit 1
fi
if [[ "$manifest_set" -eq 1 ]]; then
  [[ -n "$KANBAN_RELEASE_SOURCE_MANIFEST" && -n "$KANBAN_RELEASE_SOURCE_MAP" ]] || {
    echo "error: release provenance requires non-empty KANBAN_RELEASE_SOURCE_MANIFEST and KANBAN_RELEASE_SOURCE_MAP" >&2
    exit 1
  }
  TMPDIR="$(mktemp -d)"
  trap 'rm -rf "$TMPDIR"' EXIT
  dpkg-deb -x "$deb_path" "$TMPDIR"
  cmp -s "$TMPDIR/usr/share/doc/kanban-tool-desktop/source-provenance.json" \
    "$KANBAN_RELEASE_SOURCE_MANIFEST" || {
    echo "error: Desktop package source provenance does not match the release cohort" >&2
    exit 1
  }
  cmp -s "$TMPDIR/usr/share/doc/kanban-tool-desktop/derived-projection-v2-source-map.json" \
    "$KANBAN_RELEASE_SOURCE_MAP" || {
    echo "error: Desktop package source map does not match the release cohort" >&2
    exit 1
  }
fi

echo "ok: $deb_path contains the Desktop app"
