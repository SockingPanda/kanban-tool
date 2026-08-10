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

for tool in dpkg-deb stat find id; do
  command -v "$tool" >/dev/null 2>&1 || { echo "error: $tool is required" >&2; exit 1; }
done
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
mapfile -t sidecar_paths < <(
  find "$TMP_ROOT/extracted/usr" -type f -name 'kanban' -print
)
if [[ "${#sidecar_paths[@]}" -ne 1 ]]; then
  echo "error: Desktop deb must contain exactly one bundled kanban sidecar executable" >&2
  exit 1
fi
sidecar_path="${sidecar_paths[0]}"
if [[ "$sidecar_path" == */usr/bin/kanban || ! -x "$sidecar_path" ]]; then
  echo "error: Desktop bundled sidecar must be executable and remain outside usr/bin: $sidecar_path" >&2
  exit 1
fi
resource_root="$(dirname "$sidecar_path")"
sidecar_rel="${sidecar_path#"$TMP_ROOT/extracted/"}"
resource_rel="${resource_root#"$TMP_ROOT/extracted/"}"
# `dpkg-deb -c` 在不同版本可能打印 `./usr/...` 或 `usr/...`；先规范前缀，再按
# 完整 archive path 后缀查找，避免 substring 误匹配别的资源。
archive_contents="$(sed 's#\./usr/#usr/#g' <<<"$contents")"
archive_entry_line() {
  local target="$1" archive_line
  while IFS= read -r archive_line; do
    [[ "$archive_line" == *" $target" ]] || continue
    printf '%s\n' "$archive_line"
    return 0
  done <<<"$archive_contents"
  return 1
}
archive_sidecar_line="$(archive_entry_line "$sidecar_rel" || true)"
archive_resource_line="$(archive_entry_line "$resource_rel" || archive_entry_line "$resource_rel/" || true)"
for archive_line in "$archive_sidecar_line" "$archive_resource_line"; do
  [[ -n "$archive_line" ]] || {
    echo "error: Desktop Deb archive is missing a direct sidecar/resource-root entry" >&2
    exit 1
  }
  archive_owner="$(awk '{print $2}' <<<"$archive_line")"
  [[ "$archive_owner" == "0/0" || "$archive_owner" == "root/root" ]] || {
    echo "error: Desktop Deb sidecar/resource-root owner must be root/root (archive reports $archive_owner)" >&2
    exit 1
  }
done
resource_uid="$(stat -c '%u' "$resource_root")"
sidecar_uid="$(stat -c '%u' "$sidecar_path")"
current_uid="$(id -u)"
for uid in "$resource_uid" "$sidecar_uid"; do
  if [[ "$uid" != "0" && "$uid" != "$current_uid" ]]; then
    echo "error: Desktop resource/sidecar owner must be root or current uid: $uid" >&2
    exit 1
  fi
done
if find "$resource_root" -prune -perm /022 -print -quit | grep -q .; then
  echo "error: Desktop sidecar resource root allows group/world writes: $resource_root" >&2
  exit 1
fi
if find "$sidecar_path" -prune -perm /022 -print -quit | grep -q .; then
  echo "error: Desktop sidecar allows group/world writes: $sidecar_path" >&2
  exit 1
fi

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

echo "ok: $deb_path contains the Desktop app, bundled kanban sidecar, and exact Web artifact"
