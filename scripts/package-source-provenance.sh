#!/usr/bin/env bash
set -euo pipefail

verify_dep_info() {
  local root="$1" dep_info="$2" canonical
  canonical="$(cd "$root" && pwd -P)"
  [[ -f "$dep_info" ]] || { echo "error: dep-info not found: $dep_info" >&2; exit 1; }
  python3 - "$canonical/crates/" "$dep_info" <<'PY' || {
from pathlib import Path
import os
import sys

prefix = os.fsencode(sys.argv[1])
data = Path(sys.argv[2]).read_bytes()
prerequisites = []
word = bytearray()
in_prerequisites = False
i = 0

def finish_word():
    if word:
        if in_prerequisites:
            prerequisites.append(bytes(word))
        word.clear()

while i < len(data):
    byte = data[i]
    if byte == ord("\\"):
        if i + 1 >= len(data):
            word.append(byte)
            i += 1
            continue
        next_byte = data[i + 1]
        if next_byte == ord("\n"):
            i += 2
            continue
        if next_byte == ord("\r") and i + 2 < len(data) and data[i + 2] == ord("\n"):
            i += 3
            continue
        word.append(next_byte)
        i += 2
        continue
    if byte == ord("$") and i + 1 < len(data) and data[i + 1] == ord("$"):
        word.append(byte)
        i += 2
        continue
    if byte == ord(":") and not in_prerequisites:
        word.clear()
        in_prerequisites = True
        i += 1
        continue
    if byte in b" \t\r\n":
        finish_word()
        if byte in b"\r\n":
            in_prerequisites = False
        i += 1
        continue
    if byte == ord("#"):
        finish_word()
        newline = data.find(b"\n", i + 1)
        in_prerequisites = False
        i = len(data) if newline == -1 else newline + 1
        continue
    word.append(byte)
    i += 1

finish_word()
sys.exit(0 if any(candidate.startswith(prefix) for candidate in prerequisites) else 1)
PY
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
