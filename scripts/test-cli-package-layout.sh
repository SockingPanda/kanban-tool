#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
DEB_DIR="$("$LOCK" --print-target-dir)/release/bundle/cli/deb"

deb_path="${1:-}"
if [[ -z "$deb_path" ]]; then
  deb_path="$(find "$DEB_DIR" -maxdepth 1 -name 'kanban-tool-cli_*.deb' -type f -printf '%T@ %p\n' 2>/dev/null | sort -nr | awk 'NR == 1 { print $2 }')"
fi

[[ -n "$deb_path" && -f "$deb_path" ]] || {
  echo "error: no CLI deb found; run scripts/package-cli-linux.sh --format deb first" >&2
  exit 1
}

contents="$(dpkg-deb -c "$deb_path")"
for path in \
  './usr/bin/kanban' \
  './usr/lib/kanban/kanban-vector-lancedb' \
  './usr/lib/kanban/kanban-graph-oxigraph'
do
  grep -Fq "$path" <<<"$contents" || {
    echo "error: package $deb_path is missing $path" >&2
    exit 1
  }
done

depends="$(dpkg-deb -f "$deb_path" Depends)"
[[ -n "$depends" ]] || {
  echo "error: package $deb_path has an empty Depends field" >&2
  exit 1
}

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
  cmp -s "$TMPDIR/usr/share/doc/kanban-tool-cli/source-provenance.json" \
    "$KANBAN_RELEASE_SOURCE_MANIFEST" || {
    echo "error: CLI package source provenance does not match the release cohort" >&2
    exit 1
  }
  cmp -s "$TMPDIR/usr/share/doc/kanban-tool-cli/derived-projection-v2-source-map.json" \
    "$KANBAN_RELEASE_SOURCE_MAP" || {
    echo "error: CLI package source map does not match the release cohort" >&2
    exit 1
  }
fi

echo "ok: $deb_path contains CLI and helper layout"
