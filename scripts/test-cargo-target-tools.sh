#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

TARGET_ROOT="$TMPDIR/cargo-targets/kanban-tool"
LOCK_SCRIPT="$ROOT/scripts/cargo-build-lock.sh"
REMOVED_FLAG="--la""ne"
REMOVED_PATH_FLAG="--target-""dir"
REMOVED_ENV="KANBAN_CARGO_TARGET_""LANE"
REMOVED_HELPER="cargo-target-la""ne"

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

assert_process_exits() {
  local pid="$1"
  local label="$2"
  local i

  for i in {1..200}; do
    if ! kill -0 "$pid" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.02
  done

  fail "$label still running after interruption: pid $pid"
}

assert_no_bare_target_writing_cargo() {
  local file line_number line
  local files=(
    "$ROOT/justfile"
    "$ROOT/scripts/smoke-v1-local.sh"
    "$ROOT/scripts/package-cli-linux.sh"
  )

  for file in "${files[@]}"; do
    line_number=0
    while IFS= read -r line || [[ -n "$line" ]]; do
      line_number=$((line_number + 1))
      [[ "$line" =~ ^[[:space:]]*# ]] && continue
      if [[ "$line" =~ (^|[^[:alnum:]_/.-])cargo[[:space:]]+(build|check|clippy|test|run|nextest[[:space:]]+run) ]]; then
        if [[ "$line" != *cargo-build-lock.sh* && "$line" != *'"$LOCK"'* ]]; then
          fail "bare target-writing cargo command in ${file#$ROOT/}:$line_number: $line"
        fi
      fi
      if [[ "$line" == *"$REMOVED_FLAG"* || "$line" == *"$REMOVED_ENV"* || "$line" == *"$REMOVED_HELPER"* ]]; then
        fail "removed target split contract in ${file#$ROOT/}:$line_number: $line"
      fi
    done < "$file"
  done
}

assert_signal_status() {
  local signal="$1"
  local expected_status="$2"
  local marker="$TMPDIR/signal-${signal}-ready"
  local pid status

  rm -f "$marker"
  env --default-signal="$signal" KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- bash -c '
    set -euo pipefail
    touch "$1"
    while true; do
      sleep 1
    done
  ' _ "$marker" 2>"$TMPDIR/signal-${signal}.stderr" &
  pid=$!
  wait_for_file "$marker" "signal $signal lock holder"
  kill "-$signal" "$pid"
  set +e
  wait "$pid"
  status=$?
  set -e
  [[ "$status" -eq "$expected_status" ]] || fail "expected $signal wrapper status $expected_status, got $status"
}

assert_package_help_output_path() {
  local help_output

  help_output="$("$ROOT/scripts/package-cli-linux.sh" --help)"
  [[ "$help_output" == *'${KANBAN_CARGO_TARGET_ROOT:-$HOME/.cache/kanban-tool/cargo-target}/release/bundle/cli/deb/*.deb'* ]] || {
    fail "package help does not document the shared target root output path"
  }
  [[ "$help_output" != *"$REMOVED_HELPER.sh"* ]] || {
    fail "package help still documents the removed target helper"
  }
}

assert_resource_limit_defaults() {
  local nested_marker="$TMPDIR/resource-nested-marker"

  KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- bash -c '
    [[ "${CARGO_BUILD_JOBS:-}" == "2" ]]
    [[ "${NEXTEST_TEST_THREADS:-}" == "2" ]]
    [[ "${RUST_TEST_THREADS:-}" == "2" ]]
  '

  KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
    KANBAN_CARGO_BUILD_JOBS=1 \
    KANBAN_TEST_THREADS=3 \
    "$LOCK_SCRIPT" -- bash -c '
      [[ "${CARGO_BUILD_JOBS:-}" == "1" ]]
      [[ "${NEXTEST_TEST_THREADS:-}" == "3" ]]
      [[ "${RUST_TEST_THREADS:-}" == "3" ]]
    '

  KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
    KANBAN_CARGO_BUILD_JOBS=auto \
    KANBAN_TEST_THREADS=auto \
    "$LOCK_SCRIPT" -- bash -c '
      [[ -z "${CARGO_BUILD_JOBS:-}" ]]
      [[ -z "${NEXTEST_TEST_THREADS:-}" ]]
      [[ -z "${RUST_TEST_THREADS:-}" ]]
    '

  KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
    CARGO_BUILD_JOBS=4 \
    NEXTEST_TEST_THREADS=5 \
    RUST_TEST_THREADS=6 \
    KANBAN_CARGO_BUILD_JOBS=1 \
    KANBAN_TEST_THREADS=3 \
    "$LOCK_SCRIPT" -- bash -c '
      [[ "${CARGO_BUILD_JOBS:-}" == "4" ]]
      [[ "${NEXTEST_TEST_THREADS:-}" == "5" ]]
      [[ "${RUST_TEST_THREADS:-}" == "6" ]]
    '

  KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- "$LOCK_SCRIPT" -- bash -c '
    [[ "${CARGO_BUILD_JOBS:-}" == "2" ]]
    [[ "${NEXTEST_TEST_THREADS:-}" == "2" ]]
    [[ "${RUST_TEST_THREADS:-}" == "2" ]]
    touch "$1"
  ' _ "$nested_marker"
  [[ -e "$nested_marker" ]] || fail "nested resource limit command did not run"
}

[[ ! -e "$ROOT/scripts/$REMOVED_HELPER.sh" ]] || fail "removed target helper still exists"

KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT/" "$LOCK_SCRIPT" -- bash -c '[[ "$CARGO_TARGET_DIR" == "$1" ]]' _ "$TARGET_ROOT"
KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" CARGO_TARGET_DIR="$TARGET_ROOT/" "$LOCK_SCRIPT" -- true
assert_failure env KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" CARGO_TARGET_DIR="$TARGET_ROOT/analysis-123" "$LOCK_SCRIPT" -- true
assert_failure env KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" "$REMOVED_FLAG" cli -- true
assert_failure env KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" "$REMOVED_PATH_FLAG" "$TARGET_ROOT" -- true

locked="$TMPDIR/locked"
release="$TMPDIR/release"
first_done="$TMPDIR/first-done"
second_done="$TMPDIR/second-done"
wait_stderr="$TMPDIR/wait.stderr"

KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- bash -c '
  set -euo pipefail
  touch "$1"
  while [[ ! -e "$2" ]]; do
    sleep 0.02
  done
  touch "$3"
' _ "$locked" "$release" "$first_done" &
first_pid=$!

wait_for_file "$locked" "first lock holder"

KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- bash -c '
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
KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- bash -c 'exit 42'
failure_status=$?
set -e
[[ "$failure_status" -eq 42 ]] || fail "expected wrapped command status 42, got $failure_status"
KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- true

assert_signal_status INT 130
assert_signal_status HUP 129

descendant_pid_file="$TMPDIR/descendant.pid"
descendant_started="$TMPDIR/descendant-started"
KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- bash -c '
  set -euo pipefail
  (
    trap "" TERM
    touch "$2"
    while true; do
      sleep 1
    done
  ) &
  echo "$!" > "$1"
  wait
' _ "$descendant_pid_file" "$descendant_started" 2>"$TMPDIR/descendant.stderr" &
interrupted_pid=$!
wait_for_file "$descendant_started" "long-lived descendant"
wait_for_file "$descendant_pid_file" "long-lived descendant pid"
descendant_pid="$(cat "$descendant_pid_file")"
kill -TERM "$interrupted_pid"
set +e
wait "$interrupted_pid"
interrupted_status=$?
set -e
[[ "$interrupted_status" -eq 143 ]] || fail "expected interrupted wrapper status 143, got $interrupted_status"
assert_process_exits "$descendant_pid" "long-lived descendant"
KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- true

outer_lock_marker="$TMPDIR/outer-lock-marker"
KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- "$LOCK_SCRIPT" -- bash -c 'touch "$1"' _ "$outer_lock_marker"
[[ -e "$outer_lock_marker" ]] || fail "nested lock-held command did not run"

assert_resource_limit_defaults
assert_no_bare_target_writing_cargo
assert_package_help_output_path

echo "cargo target root and build lock tests passed"
