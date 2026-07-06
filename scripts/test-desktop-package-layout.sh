#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
DEB_DIR="$($LOCK --print-target-dir)/release/bundle/deb"
HELPERS=("kanban-vector-lancedb" "kanban-graph-oxigraph")

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

for helper in "${HELPERS[@]}"; do
  grep -Eq "(^|[[:space:]])(\\./)?usr/bin/$helper$" <<<"$contents" || {
    echo "error: Desktop deb is missing usr/bin/$helper: $deb_path" >&2
    exit 1
  }
done

if grep -Eq '(^|[[:space:]])(\./)?usr/bin/kanban$' <<<"$contents"; then
  echo "error: Desktop deb unexpectedly contains standalone CLI usr/bin/kanban" >&2
  exit 1
fi

echo "ok: $deb_path contains Desktop app and bundled helper binaries"
