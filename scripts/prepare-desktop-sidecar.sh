#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
TARGET_DIR="$("$LOCK" --print-target-dir)"
profile="${1:-release}"
kanban_command=cargo
case "$profile" in
  release)
    SOURCE="$TARGET_DIR/release/kanban"
    DEST="$ROOT/apps/desktop/src-tauri/bin/kanban"
    BUILD_HINT="${kanban_command} build --locked -p kanban-cli --release"
    PROFILE_LABEL="release"
    ;;
  dev)
    SOURCE="$TARGET_DIR/debug/kanban"
    DEST="$ROOT/apps/desktop/src-tauri/bin/kanban"
    BUILD_HINT="${kanban_command} build --locked -p kanban-cli"
    PROFILE_LABEL="debug"
    ;;
  *)
    echo "error: unsupported sidecar profile: $profile (expected release or dev)" >&2
    exit 2
    ;;
esac

[[ -x "$SOURCE" ]] || {
  echo "error: missing $PROFILE_LABEL CLI sidecar $SOURCE; run $BUILD_HINT first" >&2
  exit 1
}

mkdir -p "$(dirname "$DEST")"
if [[ "$SOURCE" != "$DEST" ]]; then
  cp -- "$SOURCE" "$DEST"
fi
chmod 0755 "$DEST"
printf '%s\n' "$DEST"
