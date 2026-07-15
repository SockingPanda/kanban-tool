#!/usr/bin/env bash
set -euo pipefail

verify_dep_info() {
  local root="$1" dep_info="$2" canonical
  canonical="$(cd "$root" && pwd -P)"
  [[ -f "$dep_info" ]] || { echo "error: dep-info not found: $dep_info" >&2; exit 1; }
  grep -Fq "$canonical/crates/" "$dep_info" || {
    echo "error: artifact provenance is not from current source root: $canonical" >&2
    exit 1
  }
}

invalidate_packages() {
  local target_dir="$1"
  shift
  local package dir leaf hash
  for package in "$@"; do
    for dir in "$target_dir/.fingerprint/$package"-*; do
      [[ -d "$dir" ]] || continue
      leaf="$(basename "$dir")"
      hash="${leaf##*-}"
      rm -rf -- "$dir" "$target_dir/build/$leaf"
      find "$target_dir/deps" -maxdepth 1 -name "*-$hash*" -delete 2>/dev/null || true
    done
  done
}

case "${1:-}" in
  --verify-dep-info) [[ $# -eq 3 ]] || exit 2; verify_dep_info "$2" "$3" ;;
  --invalidate-packages) [[ $# -ge 3 ]] || exit 2; target="$2"; shift 2; invalidate_packages "$target" "$@" ;;
  *) echo "usage: $0 --verify-dep-info ROOT DEP_INFO | --invalidate-packages TARGET PACKAGE..." >&2; exit 2 ;;
esac
