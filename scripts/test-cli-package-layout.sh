#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
DEB_DIR="$("$LOCK" --print-target-dir)/release/bundle/cli/deb"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/kanban-cli-layout.XXXXXX")"
trap 'rm -rf -- "$TMP_ROOT"' EXIT

deb_path="${1:-}"
if [[ -z "$deb_path" ]]; then
  deb_path="$(find "$DEB_DIR" -maxdepth 1 -name 'kanban-tool-cli_*.deb' -type f -printf '%T@ %p\n' 2>/dev/null | sort -nr | awk 'NR == 1 { print $2 }')"
fi

[[ -n "$deb_path" && -f "$deb_path" ]] || {
  echo "error: no CLI deb found; run just cli-package first" >&2
  exit 1
}

has_exact_package_path() {
  local expected="$1"
  local listing="$2"
  awk -v expected="$expected" '$NF == expected { found = 1 } END { exit(found ? 0 : 1) }' <<<"$listing"
}

# 路径断言必须是 exact：manifest.json.bak 这类近似路径不能满足 package 布局契约。
if has_exact_package_path './usr/share/kanban-tool/web/manifest.json' \
  'root/root 0 0 ./usr/share/kanban-tool/web/manifest.json.bak'; then
  echo "error: package path matcher accepted a prefix near-miss" >&2
  exit 1
fi

contents="$(dpkg-deb -c "$deb_path")"
for path in \
  './usr/bin/kanban' \
  './usr/share/kanban-tool/web/manifest.json'
do
  has_exact_package_path "$path" "$contents" || {
    echo "error: package $deb_path is missing $path" >&2
    exit 1
  }
done

depends="$(dpkg-deb -f "$deb_path" Depends)"
[[ -n "$depends" ]] || {
  echo "error: package $deb_path has an empty Depends field" >&2
  exit 1
}

"$LOCK" -- cargo run --locked -p xtask --bin xtask -- \
  web-assets check --root "$ROOT" --dir apps/web/dist >/dev/null

dpkg-deb --extract "$deb_path" "$TMP_ROOT/extracted"
"$LOCK" -- cargo run --locked -p xtask --bin xtask -- \
  web-assets check --root "$TMP_ROOT/extracted/usr/share/kanban-tool" --dir web >/dev/null

diff -r --no-dereference \
  "$ROOT/apps/web/dist" \
  "$TMP_ROOT/extracted/usr/share/kanban-tool/web" >/dev/null || {
  echo "error: packaged Web artifact differs from apps/web/dist" >&2
  exit 1
}

echo "ok: $deb_path contains the CLI layout and exact Web artifact"
