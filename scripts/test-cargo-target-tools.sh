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

  fail "等待 $label 超时"
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

  fail "等待 $label 超时"
}

expected_target_dir() {
  local target_root="$1"
  env KANBAN_CARGO_TARGET_ROOT="$target_root" "$LOCK_SCRIPT" --print-target-dir
}

assert_exact_shared_target_dir() {
  local target_root="$1"
  local actual="$2"

  [[ "$actual" == "$target_root" ]] || fail "预期共享 CARGO_TARGET_DIR 恰为 $target_root，实际为 $actual"
}

assert_failure() {
  if "$@" >/dev/null 2>&1; then
    fail "预期命令失败，但实际成功：$*"
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

  fail "中断后 $label 仍在运行：pid $pid"
}

assert_no_bare_target_writing_cargo() {
  local file line_number line
  local files=(
    "$ROOT/justfile"
    "$ROOT/scripts/smoke-v1-local.sh"
  )

  for file in "${files[@]}"; do
    line_number=0
    while IFS= read -r line || [[ -n "$line" ]]; do
      line_number=$((line_number + 1))
      [[ "$line" =~ ^[[:space:]]*# ]] && continue
      if [[ "$line" =~ (^|[^[:alnum:]_/.-])cargo[[:space:]]+(build|check|clippy|test|run|nextest[[:space:]]+run) ]]; then
        if [[ "$line" != *cargo-build-lock.sh* && "$line" != *'"$LOCK"'* ]]; then
          fail "${file#$ROOT/}:$line_number 存在未通过 wrapper 写 target 的 cargo 命令：$line"
        fi
      fi
      if [[ "$line" == *"$REMOVED_FLAG"* || "$line" == *"$REMOVED_ENV"* || "$line" == *"$REMOVED_HELPER"* ]]; then
        fail "${file#$ROOT/}:$line_number 仍存在已移除的 target split contract：$line"
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
  [[ "$status" -eq "$expected_status" ]] || fail "预期 $signal wrapper 状态为 $expected_status，实际为 $status"
}

assert_nextest_junit_stays_under_shared_target() {
  local configured_path

  configured_path="$(sed -n 's/^[[:space:]]*path[[:space:]]*=[[:space:]]*"\([^"]*\)"[[:space:]]*$/\1/p' "$ROOT/.config/nextest.toml")"
  [[ "$configured_path" == "junit.xml" ]] ||
    fail "nextest junit path 必须相对 shared target profile directory，实际为：$configured_path"
  rg -F 'COMMAND=(cargo nextest run --config-file "$config_path" --target-dir "$target_dir"' "$LOCK_SCRIPT" >/dev/null ||
    fail "shared-target wrapper 必须向 cargo nextest run 传递生成的 config 和精确 target dir"
  rg -F "printf '[store]\\ndir = \"%s\"\\n\\n'" "$LOCK_SCRIPT" >/dev/null ||
    fail "shared-target wrapper 必须覆盖 nextest store.dir"
}

assert_target_dir_probe_call_sites_quote_paths() {
  local file line_number line
  local files=(
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
          fail "${file#$ROOT/}:$line_number 的 target-dir probe 未加引号：$line"
          ;;
        *'$($ROOT/scripts/cargo-build-lock.sh --print-target-dir)'*)
          fail "${file#$ROOT/}:$line_number 的 ROOT target-dir probe 未加引号：$line"
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

  expected="$(env -i PATH="$PATH" HOME="$HOME" KANBAN_CARGO_TARGET_ROOT="$space_target_root" "$space_lock" --print-target-dir)"
  assert_exact_shared_target_dir "$space_target_root" "$expected"
  actual="$(env -i PATH="$PATH" HOME="$HOME" KANBAN_CARGO_TARGET_ROOT="$space_target_root" bash -c '
    set -euo pipefail
    LOCK="$1"
    TARGET_DIR="$("$LOCK" --print-target-dir)/release"
    printf "%s\n" "$TARGET_DIR"
  ' _ "$space_lock")"

  [[ "$actual" == "$expected/release" ]] || {
    fail "带空格的加引号 target-dir probe 失败：预期 $expected/release，实际为 $actual"
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
  [[ ! -e "$second_done" ]] || fail "第二个 worktree 绕过了共享 lock"

  touch "$release_first"
  wait "$first_pid"
  wait "$second_pid"
  [[ -e "$second_done" ]] || fail "共享 lock 释放后第二个 worktree 未运行"
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
    "feature-p kanban-protocol schema"; do
    before="$(sha256sum "$repo/Cargo.lock")"
    set +e
    # shellcheck disable=SC2086：recipe 有意携带 positional args
    PATH="$repo/bin:$PATH" just --justfile "$repo/justfile" \
      --working-directory "$repo" $recipe >/dev/null 2>&1
    status=$?
    set -e
    [[ "$status" -ne 0 ]] || fail "stale lock $recipe 意外成功"
    after="$(sha256sum "$repo/Cargo.lock")"
    [[ "$before" == "$after" ]] || fail "stale lock $recipe 修改了 Cargo.lock"
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
  [[ -e "$nested_marker" ]] || fail "nested resource limit command 未运行"
}

assert_dev_profile_disables_incremental_without_test_override() {
  if ! awk '
    /^[[:space:]]*\[profile\.dev\][[:space:]]*(#.*)?$/ { in_dev=1; next }
    /^[[:space:]]*\[/ { in_dev=0 }
    in_dev && /^[[:space:]]*incremental[[:space:]]*=[[:space:]]*false([[:space:]]*(#.*)?)?$/ {
      found=1
    }
    END { exit(found ? 0 : 1) }
  ' "$ROOT/Cargo.toml"; then
    fail "root Cargo.toml 必须设置 [profile.dev].incremental = false"
  fi

  if awk '
    /^[[:space:]]*\[profile\.test\][[:space:]]*(#.*)?$/ { found=1 }
    END { exit(found ? 0 : 1) }
  ' "$ROOT/Cargo.toml"; then
    fail "root Cargo.toml 不得覆盖 [profile.test]；Cargo test 继承 [profile.dev]"
  fi
}

[[ ! -e "$ROOT/scripts/$REMOVED_HELPER.sh" ]] || fail "已移除的 target helper 仍存在"

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
default_expected_target="$(env -u KANBAN_CARGO_TARGET_ROOT -u CARGO_TARGET_DIR HOME="$home_dir" "$LOCK_SCRIPT" --print-target-dir)"
[[ "$default_expected_target" == "$home_target" ]] || fail "默认 target root 必须可在 HOME 下移植"
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
[[ ! -e "$second_done" ]] || fail "第一个 lock holder 完成前第二个命令已运行"

touch "$release"
wait "$first_pid"
wait "$second_pid"
[[ -e "$first_done" ]] || fail "第一个命令未完成"
[[ -e "$second_done" ]] || fail "第二个命令未完成"

set +e
KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- bash -c 'exit 42'
failure_status=$?
set -e
[[ "$failure_status" -eq 42 ]] || fail "预期 wrapped command 状态为 42，实际为 $failure_status"
KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- true

assert_signal_status INT 130
assert_signal_status TERM 143
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
[[ "$interrupted_status" -eq 143 ]] || fail "预期 interrupted wrapper 状态为 143，实际为 $interrupted_status"
assert_process_exits "$descendant_pid" "long-lived descendant"
KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- true

outer_lock_marker="$TMPDIR/outer-lock-marker"
KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- "$LOCK_SCRIPT" -- bash -c '
  set -euo pipefail
  "$1" --verify-inherited-lock
  setsid "$1" --verify-inherited-lock
  touch "$2"
' _ "$LOCK_SCRIPT" "$outer_lock_marker"
[[ -e "$outer_lock_marker" ]] || fail "nested lock-held command 未运行"

assert_inherited_lock_proof_edges() {
  local closed_marker="$TMPDIR/closed-proof.marker"
  local reopened_marker="$TMPDIR/reopened-proof.marker"
  local mismatch_marker="$TMPDIR/mismatch-proof.marker"
  local spoof_bin="$TMPDIR/lock-proof-path-spoof-bin"

  # 没有 inherited descriptor 的 marker 和精确 target 必须在 verifier 创建
  # target 或 lock file 之前失败。
  local spoof_target="$TMPDIR/proof-spoof-target"
  assert_failure env \
    KANBAN_CARGO_TARGET_ROOT="$spoof_target" \
    CARGO_TARGET_DIR="$spoof_target" \
    KANBAN_CARGO_BUILD_LOCK_HELD=1 \
    "$LOCK_SCRIPT" --verify-inherited-lock
  [[ ! -e "$spoof_target" ]] || fail "spoofed lock proof 创建了 target state"

  # verifier contract 同时约束 lexical 和 physical 形态：CARGO_TARGET_DIR 的
  # trailing-slash alias 不是 inherited canonical target proof。
  assert_failure env KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
    "$LOCK_SCRIPT" -- bash -c '
      CARGO_TARGET_DIR="$CARGO_TARGET_DIR/" "$1" --verify-inherited-lock
    ' _ "$LOCK_SCRIPT"

  # PATH 不能替换受信任的 lock verifier primitive。即使 fake flock 和 stat
  # 总是报告成功，也不得为未加锁的 descriptor 背书。
  mkdir -p "$spoof_bin"
  printf '#!/usr/bin/env bash\nexit 0\n' >"$spoof_bin/flock"
  printf '#!/usr/bin/env bash\nif [[ "${1:-}" == "-Lc" ]]; then printf "0:0:1:regular regular file\\n"; else exit 0; fi\n' >"$spoof_bin/stat"
  chmod +x "$spoof_bin/flock" "$spoof_bin/stat"
  assert_failure env KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
    "$LOCK_SCRIPT" -- bash -c '
      set -euo pipefail
      exec 9>&-
      exec 9<>"$CARGO_TARGET_DIR/.build.lock"
      PATH="$1:/usr/bin:/bin" "$2" --verify-inherited-lock
    ' _ "$spoof_bin" "$LOCK_SCRIPT"

  KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- bash -c '
    set -euo pipefail
    lock_script="$1"
    closed_marker="$2"
    reopened_marker="$3"
    mismatch_marker="$4"
    exec 9>&-
    if "$lock_script" --verify-inherited-lock; then
      exit 71
    fi
    touch "$closed_marker"
    exec 9>"$CARGO_TARGET_DIR/.build.lock"
    if "$lock_script" --verify-inherited-lock; then
      exit 72
    fi
    touch "$reopened_marker"
    exec 9>"$CARGO_TARGET_DIR/not-the-build-lock"
    if "$lock_script" --verify-inherited-lock; then
      exit 73
    fi
    touch "$mismatch_marker"
  ' _ "$LOCK_SCRIPT" "$closed_marker" "$reopened_marker" "$mismatch_marker"
  [[ -e "$closed_marker" ]] || fail "closed inherited lock descriptor 未被拒绝"
  [[ -e "$reopened_marker" ]] || fail "reopened inherited lock descriptor 未被拒绝"
  [[ -e "$mismatch_marker" ]] || fail "mismatched inherited lock descriptor 未被拒绝"
}

assert_inherited_lock_post_flock_identity_race() {
  local race_target="$TMPDIR/proof-post-flock-race"
  local marker="$TMPDIR/proof-post-flock-race.paused"
  local continuation="$TMPDIR/proof-post-flock-race.continue"
  local status_file="$TMPDIR/proof-post-flock-race.status"
  local verifier_pid status

  mkdir -p "$race_target"
  : > "$race_target/.build.lock"
  (
    exec 9<> "$race_target/.build.lock"
    set +e
    env \
      KANBAN_CARGO_TARGET_ROOT="$race_target" \
      CARGO_TARGET_DIR="$race_target" \
      KANBAN_CARGO_BUILD_LOCK_HELD=1 \
      KANBAN_CARGO_BUILD_LOCK_PATH="$race_target/.build.lock" \
      KANBAN_CARGO_BUILD_LOCK_FD=9 \
      KANBAN_CARGO_BUILD_LOCK_TEST_PAUSE_BEFORE_FLOCK=1 \
      KANBAN_CARGO_BUILD_LOCK_TEST_PAUSE_MARKER="$marker" \
      KANBAN_CARGO_BUILD_LOCK_TEST_CONTINUE="$continuation" \
      "$LOCK_SCRIPT" --verify-inherited-lock
    printf '%s\n' "$?" > "$status_file"
  ) &
  verifier_pid=$!
  wait_for_file "$marker" "inherited lock post-flock race verifier"
  mv "$race_target/.build.lock" "$race_target/.build.lock.held"
  printf 'replacement\n' > "$race_target/.build.lock"
  : > "$continuation"
  wait "$verifier_pid"
  status="$(cat "$status_file")"
  [[ "$status" -ne 0 ]] || fail "flock 后接受了 inherited lock path replacement"
  [[ "$(cat "$race_target/.build.lock")" == "replacement" ]] ||
    fail "inherited lock race 未保留 replacement path"
}

assert_inherited_lock_post_flock_symlink_race() {
  local race_target="$TMPDIR/proof-post-flock-symlink-race"
  local marker="$TMPDIR/proof-post-flock-symlink-race.paused"
  local continuation="$TMPDIR/proof-post-flock-symlink-race.continue"
  local status_file="$TMPDIR/proof-post-flock-symlink-race.status"
  local verifier_pid status

  mkdir -p "$race_target"
  : > "$race_target/.build.lock"
  (
    exec 9<> "$race_target/.build.lock"
    set +e
    env \
      KANBAN_CARGO_TARGET_ROOT="$race_target" \
      CARGO_TARGET_DIR="$race_target" \
      KANBAN_CARGO_BUILD_LOCK_HELD=1 \
      KANBAN_CARGO_BUILD_LOCK_PATH="$race_target/.build.lock" \
      KANBAN_CARGO_BUILD_LOCK_FD=9 \
      KANBAN_CARGO_BUILD_LOCK_TEST_PAUSE_BEFORE_FLOCK=1 \
      KANBAN_CARGO_BUILD_LOCK_TEST_PAUSE_MARKER="$marker" \
      KANBAN_CARGO_BUILD_LOCK_TEST_CONTINUE="$continuation" \
      "$LOCK_SCRIPT" --verify-inherited-lock
    printf '%s\n' "$?" > "$status_file"
  ) &
  verifier_pid=$!
  wait_for_file "$marker" "inherited lock post-flock symlink race verifier"
  mv "$race_target/.build.lock" "$race_target/.build.lock.held"
  ln -s "$race_target/.build.lock.held" "$race_target/.build.lock"
  : > "$continuation"
  wait "$verifier_pid"
  status="$(cat "$status_file")"
  [[ "$status" -ne 0 ]] ||
    fail "flock 后接受了 inherited lock symlink replacement"
  [[ -L "$race_target/.build.lock" ]] ||
    fail "inherited lock symlink race 未保留 replacement"
}

assert_fresh_lock_path_safety() {
  local symlink_target="$TMPDIR/fresh-symlink-target"
  local hardlink_target="$TMPDIR/fresh-hardlink-target"
  local special_target="$TMPDIR/fresh-special-target"
  local race_target="$TMPDIR/fresh-race-target"
  local sentinel child race_ready race_release race_stderr race_child
  local holder_pid second_pid status

  sentinel="$TMPDIR/fresh-sentinel"
  printf 'sentinel-preserved\n' >"$sentinel"
  mkdir -p "$symlink_target" "$hardlink_target" "$special_target"

  child="$TMPDIR/fresh-symlink-child"
  ln -s "$sentinel" "$symlink_target/.build.lock"
  assert_failure env KANBAN_CARGO_TARGET_ROOT="$symlink_target" \
    "$LOCK_SCRIPT" -- bash -c 'touch "$1"' _ "$child"
  [[ "$(cat "$sentinel")" == "sentinel-preserved" ]] ||
    fail "fresh symlink lock acquisition 修改了 sentinel"
  [[ ! -e "$child" ]] || fail "fresh symlink lock acquisition 运行了 child"

  child="$TMPDIR/fresh-hardlink-child"
  ln "$sentinel" "$hardlink_target/.build.lock"
  assert_failure env KANBAN_CARGO_TARGET_ROOT="$hardlink_target" \
    "$LOCK_SCRIPT" -- bash -c 'touch "$1"' _ "$child"
  [[ "$(cat "$sentinel")" == "sentinel-preserved" ]] ||
    fail "fresh hardlink lock acquisition 修改了 sentinel"
  [[ ! -e "$child" ]] || fail "fresh hardlink lock acquisition 运行了 child"

  child="$TMPDIR/fresh-special-child"
  mkfifo "$special_target/.build.lock"
  assert_failure timeout 3 env KANBAN_CARGO_TARGET_ROOT="$special_target" \
    "$LOCK_SCRIPT" -- bash -c 'touch "$1"' _ "$child"
  [[ ! -e "$child" ]] || fail "fresh special-file lock acquisition 运行了 child"

  mkdir -p "$race_target"
  race_ready="$TMPDIR/fresh-race-ready"
  race_release="$TMPDIR/fresh-race-release"
  race_stderr="$TMPDIR/fresh-race.stderr"
  race_child="$TMPDIR/fresh-race-child"
  env KANBAN_CARGO_TARGET_ROOT="$race_target" "$LOCK_SCRIPT" -- bash -c '
    touch "$1"
    while [[ ! -e "$2" ]]; do sleep 0.02; done
  ' _ "$race_ready" "$race_release" &
  holder_pid=$!
  wait_for_file "$race_ready" "fresh lock race holder"
  env KANBAN_CARGO_TARGET_ROOT="$race_target" "$LOCK_SCRIPT" -- \
    bash -c 'touch "$1"' _ "$race_child" 2>"$race_stderr" &
  second_pid=$!
  wait_for_grep "正在等待其他构建/测试释放" "$race_stderr" "fresh lock race waiter"
  mv "$race_target/.build.lock" "$race_target/.build.lock.held"
  ln -s "$sentinel" "$race_target/.build.lock"
  touch "$race_release"
  set +e
  wait "$second_pid"
  status=$?
  set -e
  wait "$holder_pid"
  [[ "$status" -ne 0 ]] || fail "fresh lock path race 运行了 child"
  [[ ! -e "$race_child" ]] || fail "fresh lock path race 运行了 child"
  [[ "$(cat "$sentinel")" == "sentinel-preserved" ]] ||
    fail "fresh lock path race 修改了 sentinel"
}

assert_inherited_lock_proof_edges
assert_inherited_lock_post_flock_identity_race
assert_inherited_lock_post_flock_symlink_race
assert_fresh_lock_path_safety


assert_dev_profile_disables_incremental_without_test_override

assert_distinct_worktrees_share_target_and_lock
assert_schema_cargo_lanes_stale_lock_fail_without_mutation
assert_resource_limit_defaults
assert_no_bare_target_writing_cargo
assert_target_dir_probe_call_sites_quote_paths
assert_target_dir_probe_handles_space_paths
assert_nextest_junit_stays_under_shared_target

echo "cargo target root 和 build lock tests 已通过"
