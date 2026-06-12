#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LANE_SCRIPT="$ROOT/scripts/cargo-target-lane.sh"
TARGET_ROOT="${KANBAN_CARGO_TARGET_ROOT:-/media/kanban-user/Data/cargo-targets/kanban-tool}"
LOCK_FILE="$(printf '%s' "${TARGET_ROOT%/}")/.build.lock"
DEFAULT_LANE="${KANBAN_CARGO_TARGET_LANE:-main}"
CHILD_PID=""

usage() {
  cat <<'USAGE'
Usage:
  scripts/cargo-build-lock.sh [--lane <lane> | --target-dir <path>] -- <command> [args...]

Run a Cargo build/test/check/clippy/nextest command after acquiring the
kanban-tool local build lock. The wrapper serializes commands that write to the
shared Cargo target root and exports CARGO_TARGET_DIR for the command.

Options:
  --lane <lane>        Use one allowed lane from cargo-target-lane.sh.
                       Default: main, unless CARGO_TARGET_DIR is already set.
  --target-dir <path>  Use and validate an explicit allowed CARGO_TARGET_DIR.
  -h, --help           Show this help.

Environment:
  CARGO_TARGET_DIR            If set, it must already be one of the allowed lanes.
  KANBAN_CARGO_TARGET_LANE    Default lane when CARGO_TARGET_DIR is unset.
  KANBAN_CARGO_TARGET_ROOT    Override target root for local tests.
                              Default: /media/kanban-user/Data/cargo-targets/kanban-tool

Examples:
  scripts/cargo-build-lock.sh -- cargo check --workspace --exclude kanban-desktop --tests
  scripts/cargo-build-lock.sh --lane cli -- cargo nextest run -p kanban-cli --no-fail-fast
USAGE
}

error() {
  echo "error: $*" >&2
}

forward_signal() {
  local signal="$1"
  local exit_code="$2"

  trap - INT TERM HUP
  if [[ -n "$CHILD_PID" ]] && kill -0 "$CHILD_PID" >/dev/null 2>&1; then
    kill -s "$signal" "$CHILD_PID" >/dev/null 2>&1 || true
    wait "$CHILD_PID" >/dev/null 2>&1 || true
  fi

  exit "$exit_code"
}

main() {
  local lane_arg=""
  local target_dir_arg=""
  local target_dir=""
  local lock_dir
  local status

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --lane)
        if [[ $# -lt 2 ]]; then
          error "--lane requires a value"
          exit 2
        fi
        lane_arg="$2"
        shift 2
        ;;
      --target-dir)
        if [[ $# -lt 2 ]]; then
          error "--target-dir requires a value"
          exit 2
        fi
        target_dir_arg="$2"
        shift 2
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      --)
        shift
        break
        ;;
      *)
        error "unknown option before --: $1"
        usage >&2
        exit 2
        ;;
    esac
  done

  if [[ $# -eq 0 ]]; then
    error "missing command after --"
    usage >&2
    exit 2
  fi

  if [[ -n "$lane_arg" && -n "$target_dir_arg" ]]; then
    error "use either --lane or --target-dir, not both"
    exit 2
  fi

  if ! command -v flock >/dev/null 2>&1; then
    error "flock is required for kanban-tool Cargo build locking"
    exit 1
  fi

  if [[ -n "$target_dir_arg" ]]; then
    target_dir="$($LANE_SCRIPT check "$target_dir_arg")"
  elif [[ -n "$lane_arg" ]]; then
    target_dir="$($LANE_SCRIPT "$lane_arg")"
  elif [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
    target_dir="$($LANE_SCRIPT check "$CARGO_TARGET_DIR")"
  else
    target_dir="$($LANE_SCRIPT "$DEFAULT_LANE")"
  fi

  lock_dir="$(dirname "$LOCK_FILE")"
  mkdir -p "$lock_dir" "$target_dir"

  exec 9>"$LOCK_FILE"
  if ! flock -n 9; then
    echo "正在等待其他构建/测试释放 Cargo target 锁：$LOCK_FILE" >&2
    flock 9
  fi

  export CARGO_TARGET_DIR="$target_dir"
  "$@" &
  CHILD_PID=$!

  trap 'forward_signal INT 130' INT
  trap 'forward_signal TERM 143' TERM
  trap 'forward_signal HUP 129' HUP

  set +e
  wait "$CHILD_PID"
  status=$?
  set -e

  CHILD_PID=""
  trap - INT TERM HUP
  exit "$status"
}

main "$@"
