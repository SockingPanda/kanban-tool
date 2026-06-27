#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_ROOT="${KANBAN_CARGO_TARGET_ROOT:-$HOME/.cache/kanban-tool/cargo-target}"
DEB_DIR="$TARGET_ROOT/release/bundle/cli/deb"

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

echo "ok: $deb_path contains CLI and helper layout"
