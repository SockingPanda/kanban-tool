#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
DEB_DIR="$("$LOCK" --print-target-dir)/release/bundle/deb"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/kanban-desktop-layout.XXXXXX")"
trap 'rm -rf -- "$TMP_ROOT"' EXIT

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

"$LOCK" -- cargo run --locked -p xtask --bin xtask -- \
  web-assets check --root "$ROOT" --dir apps/web/dist >/dev/null

dpkg-deb --extract "$deb_path" "$TMP_ROOT/extracted"
mapfile -t web_manifests < <(
  find "$TMP_ROOT/extracted/usr" -type f -path '*/web/manifest.json' -print
)
if [[ "${#web_manifests[@]}" -ne 1 ]]; then
  echo "error: Desktop deb must contain exactly one bundled Web artifact" >&2
  exit 1
fi
web_root="$(dirname "${web_manifests[0]}")"
web_parent="$(dirname "$web_root")"
"$LOCK" -- cargo run --locked -p xtask --bin xtask -- \
  web-assets check --root "$web_parent" --dir web >/dev/null

diff -r --no-dereference \
  "$ROOT/apps/web/dist" \
  "$web_root" >/dev/null || {
  echo "error: packaged Desktop Web artifact differs from apps/web/dist" >&2
  exit 1
}

echo "ok: $deb_path contains the Desktop app and exact Web artifact"
