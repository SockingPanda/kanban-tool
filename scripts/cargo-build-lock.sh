#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RAW_TARGET_ROOT="${KANBAN_CARGO_TARGET_ROOT:-/media/kanban-user/Data/cargo-targets/kanban-tool}"
CHILD_PID=""
CHILD_PGID=""

usage() {
  cat <<'USAGE'
Usage:
  scripts/cargo-build-lock.sh -- <command> [args...]

Run a Cargo build/test/check/clippy/nextest command after acquiring the
kanban-tool local build lock. The wrapper serializes commands that write to the
shared Cargo target root and exports CARGO_TARGET_DIR to that root for the
command.

Options:
  -h, --help  Show this help.

Environment:
  CARGO_TARGET_DIR            If set, it must equal the configured shared target
                              root.
  CARGO_BUILD_JOBS            Cargo build jobs passed through when set.
                              Default: ${KANBAN_CARGO_BUILD_JOBS:-2}
  NEXTEST_TEST_THREADS        cargo-nextest test threads passed through when set.
                              Default: ${KANBAN_TEST_THREADS:-2}
  RUST_TEST_THREADS           libtest threads passed through when set.
                              Default: ${KANBAN_TEST_THREADS:-2}
  KANBAN_CARGO_TARGET_ROOT    Override target root for local tests.
                              Default: /media/kanban-user/Data/cargo-targets/kanban-tool
  KANBAN_CARGO_BUILD_JOBS     Repo-level default for CARGO_BUILD_JOBS.
  KANBAN_TEST_THREADS         Repo-level default for nextest/libtest threads.
                              Set either repo-level value to "auto" to leave the
                              tool-specific variable unset.

Examples:
  scripts/cargo-build-lock.sh -- cargo check --workspace --exclude kanban-desktop --tests
  scripts/cargo-build-lock.sh -- cargo nextest run -p kanban-cli --no-fail-fast
USAGE
}

error() {
  echo "error: $*" >&2
}

cleanup_process_group() {
  local signal="$1"
  local pgid="$2"
  local i

  if [[ -z "$pgid" ]]; then
    return 0
  fi

  kill -s "$signal" -- "-$pgid" >/dev/null 2>&1 || true

  for i in {1..100}; do
    if ! kill -0 -- "-$pgid" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.02
  done

  kill -KILL -- "-$pgid" >/dev/null 2>&1 || true
  for i in {1..100}; do
    if ! kill -0 -- "-$pgid" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.02
  done
}

forward_signal() {
  local signal="$1"
  local exit_code="$2"

  trap - INT TERM HUP
  if [[ -n "$CHILD_PGID" ]]; then
    cleanup_process_group "$signal" "$CHILD_PGID"
    if [[ -n "$CHILD_PID" ]]; then
      wait "$CHILD_PID" >/dev/null 2>&1 || true
    fi
  elif [[ -n "$CHILD_PID" ]] && kill -0 "$CHILD_PID" >/dev/null 2>&1; then
    kill -s "$signal" "$CHILD_PID" >/dev/null 2>&1 || true
    wait "$CHILD_PID" >/dev/null 2>&1 || true
  fi

  exit "$exit_code"
}

normalize_path() {
  local path="$1"
  while [[ "$path" != "/" && "$path" == */ ]]; do
    path="${path%/}"
  done
  printf '%s\n' "$path"
}

expand_home_path() {
  local path="$1"

  case "$path" in
    '$HOME')
      printf '%s\n' "$HOME"
      ;;
    '$HOME/'*)
      printf '%s/%s\n' "$HOME" "${path#\$HOME/}"
      ;;
    '${HOME}')
      printf '%s\n' "$HOME"
      ;;
    '${HOME}/'*)
      printf '%s/%s\n' "$HOME" "${path#\$\{HOME\}/}"
      ;;
    '~')
      printf '%s\n' "$HOME"
      ;;
    '~/'*)
      printf '%s/%s\n' "$HOME" "${path#\~/}"
      ;;
    *)
      printf '%s\n' "$path"
      ;;
  esac
}

target_root() {
  normalize_path "$(expand_home_path "$RAW_TARGET_ROOT")"
}

validate_inherited_target_dir() {
  local expected="$1"
  local actual

  if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
    return 0
  fi

  actual="$(normalize_path "$(expand_home_path "$CARGO_TARGET_DIR")")"
  if [[ "$actual" != "$expected" ]]; then
    error "CARGO_TARGET_DIR must be the kanban-tool shared target root: $expected"
    error "got: $CARGO_TARGET_DIR"
    return 2
  fi
}

configure_resource_limits() {
  configure_resource_limit CARGO_BUILD_JOBS "${KANBAN_CARGO_BUILD_JOBS:-2}"
  configure_resource_limit NEXTEST_TEST_THREADS "${KANBAN_TEST_THREADS:-2}"
  configure_resource_limit RUST_TEST_THREADS "${KANBAN_TEST_THREADS:-2}"
}

configure_resource_limit() {
  local name="$1"
  local default_value="$2"

  if [[ -n "${!name:-}" ]]; then
    return 0
  fi
  case "$default_value" in
    auto|AUTO)
      return 0
      ;;
  esac
  export "$name=$default_value"
}

main() {
  local target_dir=""
  local lock_file=""
  local lock_dir
  local status

  while [[ $# -gt 0 ]]; do
    case "$1" in
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

  if ! command -v flock >/dev/null 2>&1; then
    error "flock is required for kanban-tool Cargo build locking"
    exit 1
  fi
  if ! command -v setsid >/dev/null 2>&1; then
    error "setsid is required for kanban-tool Cargo build locking"
    exit 1
  fi

  target_dir="$(target_root)"
  validate_inherited_target_dir "$target_dir"
  lock_file="$target_dir/.build.lock"

  lock_dir="$(dirname "$lock_file")"
  mkdir -p "$lock_dir" "$target_dir"
  configure_resource_limits

  if [[ "${KANBAN_CARGO_BUILD_LOCK_HELD:-}" == "1" ]]; then
    export CARGO_TARGET_DIR="$target_dir"
    "$@"
    exit $?
  fi

  exec 9>"$lock_file"
  if ! flock -n 9; then
    echo "正在等待其他构建/测试释放 Cargo target 锁：$lock_file" >&2
    flock 9
  fi

  export CARGO_TARGET_DIR="$target_dir"
  export KANBAN_CARGO_BUILD_LOCK_HELD=1
  setsid "$@" &
  CHILD_PID=$!
  CHILD_PGID=$CHILD_PID

  trap 'forward_signal INT 130' INT
  trap 'forward_signal TERM 143' TERM
  trap 'forward_signal HUP 129' HUP

  set +e
  wait "$CHILD_PID"
  status=$?
  set -e

  CHILD_PID=""
  CHILD_PGID=""
  trap - INT TERM HUP
  exit "$status"
}

main "$@"
