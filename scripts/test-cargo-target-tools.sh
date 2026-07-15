#!/usr/bin/env bash
set -euo pipefail
unset CARGO_TARGET_DIR

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

expected_target_dir() {
  local target_root="$1"
  env KANBAN_CARGO_TARGET_ROOT="$target_root" "$LOCK_SCRIPT" --print-target-dir
}

assert_exact_shared_target_dir() {
  local target_root="$1"
  local actual="$2"

  [[ "$actual" == "$target_root" ]] || fail "expected exact shared CARGO_TARGET_DIR $target_root, got $actual"
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
  [[ "$help_output" == *'scripts/cargo-build-lock.sh --print-target-dir'* ]] || {
    fail "package help does not document the wrapper target-dir output path"
  }
  [[ "$help_output" != *"$REMOVED_HELPER.sh"* ]] || {
    fail "package help still documents the removed target helper"
  }
  [[ "$help_output" != *'$ROOT'* ]] || {
    fail "package help should not print a literal \$ROOT placeholder"
  }
  [[ "$help_output" != *'$('* ]] || {
    fail "package help should describe the target-dir probe without shell substitution syntax"
  }
  [[ "$help_output" == *'release/bundle/cli/deb/*.deb'* ]] || {
    fail "package help does not document the CLI deb relative output path"
  }
}

assert_debian_control_directory_mode() {
  rg -F 'install -d -m 0755 "$control_dir"' "$ROOT/scripts/package-cli-linux.sh" >/dev/null ||
    fail "CLI package must create DEBIAN control directory with a dpkg-deb compatible mode"
}

assert_nextest_junit_stays_under_shared_target() {
  local configured_path

  configured_path="$(sed -n 's/^[[:space:]]*path[[:space:]]*=[[:space:]]*"\([^"]*\)"[[:space:]]*$/\1/p' "$ROOT/.config/nextest.toml")"
  [[ "$configured_path" == "junit.xml" ]] ||
    fail "nextest junit path must be relative to its shared target profile directory, got: $configured_path"
  rg -F 'COMMAND=(cargo nextest run --config-file "$config_path" --target-dir "$target_dir"' "$LOCK_SCRIPT" >/dev/null ||
    fail "shared-target wrapper must pass its generated config and exact target dir to cargo nextest run"
  rg -F "printf '[store]\\ndir = \"%s\"\\n\\n'" "$LOCK_SCRIPT" >/dev/null ||
    fail "shared-target wrapper must override nextest store.dir"
}

assert_target_dir_probe_call_sites_quote_paths() {
  local file line_number line
  local files=(
    "$ROOT/scripts/package-cli-linux.sh"
    "$ROOT/scripts/prepare-desktop-helper-binaries.sh"
    "$ROOT/scripts/test-cli-package-layout.sh"
    "$ROOT/scripts/test-desktop-package-layout.sh"
    "$ROOT/scripts/ontology-bootstrap-verify-e2e.sh"
    "$ROOT/scripts/ontology-closure-e2e.sh"
    "$ROOT/scripts/ontology-negative-atom-e2e.sh"
  )

  for file in "${files[@]}"; do
    line_number=0
    while IFS= read -r line || [[ -n "$line" ]]; do
      line_number=$((line_number + 1))
      case "$line" in
        *'$($LOCK --print-target-dir)'*)
          fail "unquoted target-dir probe in ${file#$ROOT/}:$line_number: $line"
          ;;
        *'$($ROOT/scripts/cargo-build-lock.sh --print-target-dir)'*)
          fail "unquoted ROOT target-dir probe in ${file#$ROOT/}:$line_number: $line"
          ;;
      esac
    done < "$file"
  done
}

assert_target_dir_probe_handles_space_paths() {
  local space_repo="$TMPDIR/repo root with spaces"
  local space_target_root="$TMPDIR/target root with spaces"
  local space_lock="$space_repo/scripts/cargo-build-lock.sh"
  local expected actual

  mkdir -p "$space_repo/scripts" "$space_target_root"
  cp "$LOCK_SCRIPT" "$space_lock"
  chmod +x "$space_lock"

  expected="$(env KANBAN_CARGO_TARGET_ROOT="$space_target_root" "$space_lock" --print-target-dir)"
  assert_exact_shared_target_dir "$space_target_root" "$expected"
  actual="$(env KANBAN_CARGO_TARGET_ROOT="$space_target_root" bash -c '
    set -euo pipefail
    LOCK="$1"
    TARGET_DIR="$("$LOCK" --print-target-dir)/release"
    printf "%s\n" "$TARGET_DIR"
  ' _ "$space_lock")"

  [[ "$actual" == "$expected/release" ]] || {
    fail "quoted target-dir probe failed with spaces: expected $expected/release, got $actual"
  }
}

assert_distinct_worktrees_share_target_and_lock() {
  local repo_a="$TMPDIR/worktree-a"
  local repo_b="$TMPDIR/worktree-b"
  local lock_a="$repo_a/scripts/cargo-build-lock.sh"
  local lock_b="$repo_b/scripts/cargo-build-lock.sh"
  local shared_root="$TMPDIR/exact-shared-target"
  local first_ready="$TMPDIR/cross-worktree-first-ready"
  local release_first="$TMPDIR/cross-worktree-release-first"
  local second_done="$TMPDIR/cross-worktree-second-done"
  local second_stderr="$TMPDIR/cross-worktree-second.stderr"
  local first_pid second_pid

  mkdir -p "$repo_a/scripts" "$repo_b/scripts" "$shared_root"
  cp "$LOCK_SCRIPT" "$lock_a"
  cp "$LOCK_SCRIPT" "$lock_b"
  chmod +x "$lock_a" "$lock_b"

  [[ "$(KANBAN_CARGO_TARGET_ROOT="$shared_root" "$lock_a" --print-target-dir)" == "$shared_root" ]]
  [[ "$(KANBAN_CARGO_TARGET_ROOT="$shared_root" "$lock_b" --print-target-dir)" == "$shared_root" ]]

  KANBAN_CARGO_TARGET_ROOT="$shared_root" "$lock_a" -- bash -c '
    touch "$1"
    while [[ ! -e "$2" ]]; do sleep 0.02; done
  ' _ "$first_ready" "$release_first" &
  first_pid=$!
  wait_for_file "$first_ready" "cross-worktree first lock holder"

  KANBAN_CARGO_TARGET_ROOT="$shared_root" "$lock_b" -- bash -c 'touch "$1"' _ "$second_done" 2>"$second_stderr" &
  second_pid=$!
  wait_for_grep "正在等待其他构建/测试释放" "$second_stderr" "cross-worktree lock wait"
  [[ ! -e "$second_done" ]] || fail "second worktree bypassed the shared lock"

  touch "$release_first"
  wait "$first_pid"
  wait "$second_pid"
  [[ -e "$second_done" ]] || fail "second worktree did not run after the shared lock was released"
}

assert_package_lock_marker_is_wrapper_owned() {
  local repo="$TMPDIR/package-marker-repo"
  local fake_bin="$repo/bin"
  local wrapper_marker="$repo/wrapper-entered-clean"
  local status

  mkdir -p "$repo/scripts" "$fake_bin"
  cp "$ROOT/scripts/package-cli-linux.sh" "$repo/scripts/package-cli-linux.sh"
  cat > "$repo/scripts/cargo-build-lock.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--print-target-dir" ]]; then
  printf '%s\n' "$PACKAGE_TEST_TARGET_ROOT"
  exit 0
fi
[[ "${1:-}" == "--" ]]
shift
if [[ "${KANBAN_CARGO_BUILD_LOCK_HELD:-}" == "1" ]]; then
  exec "$@"
fi
[[ -z "${KANBAN_PACKAGE_BUILD_LOCK_HELD:-}" ]] || {
  echo "error: package forged its own build-lock marker" >&2
  exit 97
}
touch "$PACKAGE_WRAPPER_MARKER"
exec env KANBAN_CARGO_BUILD_LOCK_HELD=1 "$@"
EOF
  cat > "$fake_bin/cargo" <<'EOF'
#!/usr/bin/env bash
exit 86
EOF
  chmod +x "$repo/scripts/package-cli-linux.sh" \
    "$repo/scripts/cargo-build-lock.sh" "$fake_bin/cargo"

  set +e
  env \
    -u KANBAN_CARGO_BUILD_LOCK_HELD \
    -u KANBAN_PACKAGE_BUILD_LOCK_HELD \
    PATH="$fake_bin:$PATH" \
    PACKAGE_TEST_TARGET_ROOT="$repo/target" \
    PACKAGE_WRAPPER_MARKER="$wrapper_marker" \
    "$repo/scripts/package-cli-linux.sh" --format deb >/dev/null 2>&1
  status=$?
  set -e

  [[ "$status" -eq 86 ]] || fail "expected package child to reach fake cargo with status 86, got $status"
  [[ -e "$wrapper_marker" ]] || fail "package did not enter the build-lock wrapper without a forged marker"
}

assert_package_waits_for_shared_build_lock() (
  local holder_ready="$TMPDIR/package-lock-holder-ready"
  local release_holder="$TMPDIR/package-lock-holder-release"
  local cargo_marker="$TMPDIR/package-cargo-entered"
  local package_stderr="$TMPDIR/package-lock.stderr"
  local fake_bin="$TMPDIR/package-lock-bin"
  local holder_pid="" package_pid="" status

  cleanup_package_lock_processes() {
    local pid i

    touch "$release_holder" 2>/dev/null || true
    for pid in "$package_pid" "$holder_pid"; do
      [[ -n "$pid" ]] || continue
      if kill -0 "$pid" >/dev/null 2>&1; then
        kill -TERM "$pid" >/dev/null 2>&1 || true
        for i in {1..100}; do
          kill -0 "$pid" >/dev/null 2>&1 || break
          sleep 0.02
        done
        kill -KILL "$pid" >/dev/null 2>&1 || true
      fi
      wait "$pid" >/dev/null 2>&1 || true
    done
  }
  trap cleanup_package_lock_processes EXIT

  mkdir -p "$fake_bin"
  cat > "$fake_bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
touch "$PACKAGE_CARGO_MARKER"
exit 86
EOF
  chmod +x "$fake_bin/cargo"

  KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- bash -c '
    set -euo pipefail
    touch "$1"
    while [[ ! -e "$2" ]]; do sleep 0.02; done
  ' _ "$holder_ready" "$release_holder" &
  holder_pid=$!
  wait_for_file "$holder_ready" "package build lock holder"

  env \
    -u KANBAN_CARGO_BUILD_LOCK_HELD \
    -u KANBAN_PACKAGE_BUILD_LOCK_HELD \
    KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
    PACKAGE_CARGO_MARKER="$cargo_marker" \
    PATH="$fake_bin:$PATH" \
    "$ROOT/scripts/package-cli-linux.sh" --format deb \
    >/dev/null 2>"$package_stderr" &
  package_pid=$!

  wait_for_grep "正在等待其他构建/测试释放" "$package_stderr" "package lock wait"
  [[ ! -e "$cargo_marker" ]] || fail "package entered Cargo while the shared build lock was occupied"

  touch "$release_holder"
  wait "$holder_pid"
  holder_pid=""
  set +e
  wait "$package_pid"
  status=$?
  set -e
  package_pid=""
  [[ "$status" -eq 86 ]] || fail "expected package to resume into fake cargo with status 86, got $status"
  [[ -e "$cargo_marker" ]] || fail "package did not resume after the shared build lock was released"
  trap - EXIT
)

assert_package_source_provenance_is_current_and_non_mutating() {
  local source_a="$TMPDIR/source a"
  local source_b="$TMPDIR/source b"
  local stale_dep_info="$TMPDIR/stale.d"
  local escaped_source_a escaped_source_b misleading_target before after

  mkdir -p "$source_a/crates/kanban-cli/src" "$source_b/crates/kanban-cli/src"
  printf 'fn marker() {}\n' > "$source_a/crates/kanban-cli/src/main.rs"
  printf 'fn marker() {}\n' > "$source_b/crates/kanban-cli/src/main.rs"
  escaped_source_a="${source_a// /\\ }"
  escaped_source_b="${source_b// /\\ }"
  printf '%s/release/kanban: %s/crates/kanban-cli/src/main.rs\n' \
    "$TARGET_ROOT" "$escaped_source_a" > "$stale_dep_info"
  before="$(sha256sum "$stale_dep_info")"
  assert_failure "$ROOT/scripts/package-source-provenance.sh" --verify-dep-info "$source_b" "$stale_dep_info"
  after="$(sha256sum "$stale_dep_info")"
  [[ "$before" == "$after" ]] || fail "stale provenance rejection mutated the dep-info artifact"
  "$ROOT/scripts/package-source-provenance.sh" --verify-dep-info "$source_a" "$stale_dep_info"

  misleading_target="$TMPDIR/misleading-target.d"
  printf '%s/crates/target/release/kanban: %s/crates/kanban-cli/src/main.rs\n' \
    "$escaped_source_b" "$escaped_source_a" > "$misleading_target"
  assert_failure "$ROOT/scripts/package-source-provenance.sh" \
    --verify-dep-info "$source_b" "$misleading_target"

  mkdir -p "$TARGET_ROOT/release/.fingerprint/workspace-crate-aaa" \
    "$TARGET_ROOT/release/.fingerprint/registry-crate-bbb" "$TARGET_ROOT/release/deps"
  touch "$TARGET_ROOT/release/deps/libworkspace_crate-aaa.rlib" \
    "$TARGET_ROOT/release/deps/libregistry_crate-bbb.rlib"
  "$ROOT/scripts/package-source-provenance.sh" --invalidate-packages \
    "$TARGET_ROOT/release" workspace-crate
  [[ ! -e "$TARGET_ROOT/release/.fingerprint/workspace-crate-aaa" ]]
  [[ ! -e "$TARGET_ROOT/release/deps/libworkspace_crate-aaa.rlib" ]]
  [[ -e "$TARGET_ROOT/release/.fingerprint/registry-crate-bbb" ]]
  [[ -e "$TARGET_ROOT/release/deps/libregistry_crate-bbb.rlib" ]]
}

assert_schema_cargo_lanes_stale_lock_fail_without_mutation() {
  local repo="$TMPDIR/stale-lock-repo"
  local before after status recipe
  mkdir -p "$repo/scripts" "$repo/bin"
  cp "$ROOT/justfile" "$repo/justfile"
  printf 'stale-lock\n' > "$repo/Cargo.lock"
  cat > "$repo/scripts/cargo-build-lock.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ "$1" == "--" ]]
shift
exec "$@"
EOF
  cat > "$repo/bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "nextest" && "${2:-}" == "--version" ]]; then
  exit 0
fi
if [[ " $* " == *' --locked '* ]]; then
  echo 'error: the lock file needs to be updated but --locked was passed' >&2
  exit 101
fi
printf 'mutated\n' >> Cargo.lock
EOF
  chmod +x "$repo/scripts/cargo-build-lock.sh" "$repo/bin/cargo"
  for recipe in schema-check schema-tool schema-surface-audit \
    "feature-p kanban-contract schema"; do
    before="$(sha256sum "$repo/Cargo.lock")"
    set +e
    # shellcheck disable=SC2086 # recipe intentionally carries positional args
    PATH="$repo/bin:$PATH" just --justfile "$repo/justfile" \
      --working-directory "$repo" $recipe >/dev/null 2>&1
    status=$?
    set -e
    [[ "$status" -ne 0 ]] || fail "stale lock $recipe unexpectedly succeeded"
    after="$(sha256sum "$repo/Cargo.lock")"
    [[ "$before" == "$after" ]] || fail "stale lock $recipe mutated Cargo.lock"
  done
}

assert_resource_limit_defaults() {
  local nested_marker="$TMPDIR/resource-nested-marker"

  env \
    -u CARGO_BUILD_JOBS \
    -u NEXTEST_TEST_THREADS \
    -u RUST_TEST_THREADS \
    -u KANBAN_CARGO_BUILD_JOBS \
    -u KANBAN_TEST_THREADS \
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

  env \
    -u CARGO_BUILD_JOBS \
    -u NEXTEST_TEST_THREADS \
    -u RUST_TEST_THREADS \
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

  env \
    -u CARGO_BUILD_JOBS \
    -u NEXTEST_TEST_THREADS \
    -u RUST_TEST_THREADS \
    -u KANBAN_CARGO_BUILD_JOBS \
    -u KANBAN_TEST_THREADS \
    KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- "$LOCK_SCRIPT" -- bash -c '
    [[ "${CARGO_BUILD_JOBS:-}" == "2" ]]
    [[ "${NEXTEST_TEST_THREADS:-}" == "2" ]]
    [[ "${RUST_TEST_THREADS:-}" == "2" ]]
    touch "$1"
  ' _ "$nested_marker"
  [[ -e "$nested_marker" ]] || fail "nested resource limit command did not run"
}

[[ ! -e "$ROOT/scripts/$REMOVED_HELPER.sh" ]] || fail "removed target helper still exists"

expected_target="$(expected_target_dir "$TARGET_ROOT")"
KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT/" "$LOCK_SCRIPT" -- bash -c '
  [[ "$CARGO_TARGET_DIR" == "$1" ]]
  [[ -e "$2/.build.lock" ]]
' _ "$expected_target" "$TARGET_ROOT"
assert_exact_shared_target_dir "$TARGET_ROOT" "$expected_target"
KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" CARGO_TARGET_DIR="$TARGET_ROOT/" "$LOCK_SCRIPT" -- true
home_dir="$TMPDIR/home"
home_target="$home_dir/.cache/kanban-tool/cargo-target"
mkdir -p "$home_dir"
home_expected_target="$(HOME="$home_dir" KANBAN_CARGO_TARGET_ROOT='$HOME/.cache/kanban-tool/cargo-target' "$LOCK_SCRIPT" --print-target-dir)"
env HOME="$home_dir"   KANBAN_CARGO_TARGET_ROOT='$HOME/.cache/kanban-tool/cargo-target'   "$LOCK_SCRIPT" -- bash -c '[[ "$CARGO_TARGET_DIR" == "$1" ]]' _ "$home_expected_target"
assert_exact_shared_target_dir "$home_target" "$home_expected_target"
env HOME="$home_dir"   KANBAN_CARGO_TARGET_ROOT='${HOME}/.cache/kanban-tool/cargo-target'   CARGO_TARGET_DIR='${HOME}/.cache/kanban-tool/cargo-target'   "$LOCK_SCRIPT" -- true
tilde_expected_target="$(HOME="$home_dir" KANBAN_CARGO_TARGET_ROOT='~/.cache/kanban-tool/cargo-target' "$LOCK_SCRIPT" --print-target-dir)"
env HOME="$home_dir"   KANBAN_CARGO_TARGET_ROOT='~/.cache/kanban-tool/cargo-target'   "$LOCK_SCRIPT" -- bash -c '[[ "$CARGO_TARGET_DIR" == "$1" ]]' _ "$tilde_expected_target"
assert_exact_shared_target_dir "$home_target" "$tilde_expected_target"
assert_failure env KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" CARGO_TARGET_DIR="$TMPDIR/outside-target" "$LOCK_SCRIPT" -- true
assert_failure env KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" CARGO_TARGET_DIR="$TARGET_ROOT/subdir" "$LOCK_SCRIPT" -- true
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

assert_distinct_worktrees_share_target_and_lock
assert_package_lock_marker_is_wrapper_owned
assert_package_waits_for_shared_build_lock
assert_package_source_provenance_is_current_and_non_mutating
assert_schema_cargo_lanes_stale_lock_fail_without_mutation
assert_resource_limit_defaults
assert_no_bare_target_writing_cargo
assert_target_dir_probe_call_sites_quote_paths
assert_target_dir_probe_handles_space_paths
assert_package_help_output_path
assert_debian_control_directory_mode
assert_nextest_junit_stays_under_shared_target

echo "cargo target root and build lock tests passed"
