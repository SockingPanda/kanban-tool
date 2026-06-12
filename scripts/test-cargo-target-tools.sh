#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

TARGET_ROOT="$TMPDIR/cargo-targets/kanban-tool"
LANE_SCRIPT="$ROOT/scripts/cargo-target-lane.sh"
LOCK_SCRIPT="$ROOT/scripts/cargo-build-lock.sh"

fail() {
  echo "error: $*" >&2
  exit 1
}

wait_for_file() {
  local path="$1"
  local label="$2"
  local i

  for i in {1..200}; do
    if [[ -e "$path" ]]; then
      return 0
    fi
    sleep 0.02
  done

  fail "timed out waiting for $label"
}

wait_for_grep() {
  local pattern="$1"
  local path="$2"
  local label="$3"
  local i

  for i in {1..200}; do
    if [[ -e "$path" ]] && grep -q "$pattern" "$path"; then
      return 0
    fi
    sleep 0.02
  done

  fail "timed out waiting for $label"
}

assert_failure() {
  if "$@" >/dev/null 2>&1; then
    fail "expected failure but command succeeded: $*"
  fi
}

expected_cli_target="$TARGET_ROOT/cli"
actual_cli_target="$(KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LANE_SCRIPT" cli)"
[[ "$actual_cli_target" == "$expected_cli_target" ]] || fail "unexpected cli lane path: $actual_cli_target"

KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LANE_SCRIPT" check "$expected_cli_target/" >/dev/null
assert_failure env KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LANE_SCRIPT" review-vector
assert_failure env KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LANE_SCRIPT" check "$TARGET_ROOT/review-vector"

KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- bash -c '[[ "$CARGO_TARGET_DIR" == "$1" ]]' _ "$TARGET_ROOT/main"
assert_failure env KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" CARGO_TARGET_DIR="$TARGET_ROOT/analysis-123" "$LOCK_SCRIPT" -- true

locked="$TMPDIR/locked"
release="$TMPDIR/release"
first_done="$TMPDIR/first-done"
second_done="$TMPDIR/second-done"
wait_stderr="$TMPDIR/wait.stderr"

KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" --lane cli -- bash -c '
  set -euo pipefail
  touch "$1"
  while [[ ! -e "$2" ]]; do
    sleep 0.02
  done
  touch "$3"
' _ "$locked" "$release" "$first_done" &
first_pid=$!

wait_for_file "$locked" "first lock holder"

KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" --lane cli -- bash -c '
  set -euo pipefail
  [[ -e "$1" ]]
  touch "$2"
' _ "$first_done" "$second_done" 2>"$wait_stderr" &
second_pid=$!

wait_for_grep "正在等待其他构建/测试释放" "$wait_stderr" "lock wait message"
[[ ! -e "$second_done" ]] || fail "second command ran before first lock holder finished"

touch "$release"
wait "$first_pid"
wait "$second_pid"
[[ -e "$first_done" ]] || fail "first command did not finish"
[[ -e "$second_done" ]] || fail "second command did not finish"

set +e
KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" --lane cli -- bash -c 'exit 42'
failure_status=$?
set -e
[[ "$failure_status" -eq 42 ]] || fail "expected wrapped command status 42, got $failure_status"
KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" --lane cli -- true

interrupted_locked="$TMPDIR/interrupted-locked"
KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" --lane cli -- bash -c '
  set -euo pipefail
  trap "exit 0" TERM
  touch "$1"
  while true; do
    sleep 1
  done
' _ "$interrupted_locked" &
interrupted_pid=$!
wait_for_file "$interrupted_locked" "interrupted lock holder"
kill -TERM "$interrupted_pid"
set +e
wait "$interrupted_pid"
interrupted_status=$?
set -e
[[ "$interrupted_status" -eq 143 ]] || fail "expected interrupted wrapper status 143, got $interrupted_status"
KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" --lane cli -- true

echo "cargo target lane and build lock tests passed"
