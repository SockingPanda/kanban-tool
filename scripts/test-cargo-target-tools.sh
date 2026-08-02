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
    "$ROOT/scripts/release-cohort.sh"
    "$ROOT/scripts/release-artifact-manifest.sh"
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
  cp "$ROOT/scripts/package-source-provenance.sh" "$repo/scripts/package-source-provenance.sh"
  cp "$ROOT/scripts/release-safe-path.py" "$repo/scripts/release-safe-path.py"
  cp "$ROOT/README.md" "$repo/README.md"
  cat > "$repo/scripts/cargo-build-lock.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--print-target-dir" ]]; then
  printf '%s\n' "$PACKAGE_TEST_TARGET_ROOT"
  exit 0
fi
if [[ "${1:-}" == "--verify-inherited-lock" ]]; then
  [[ "${KANBAN_CARGO_BUILD_LOCK_HELD:-}" == "1" ]] || exit 2
  [[ "${CARGO_TARGET_DIR:-}" == "$PACKAGE_TEST_TARGET_ROOT" ]] || exit 2
  [[ "${KANBAN_CARGO_BUILD_LOCK_PATH:-}" == "$PACKAGE_TEST_TARGET_ROOT/.build.lock" ]] || exit 2
  lock_fd="${KANBAN_CARGO_BUILD_LOCK_FD:-}"
  [[ "$lock_fd" =~ ^[3-9][0-9]*$ && -e "/proc/self/fd/$lock_fd" ]] || exit 2
  expected="$(stat -Lc '%d:%i:%h:%F' "$PACKAGE_TEST_TARGET_ROOT/.build.lock")" || exit 2
  [[ "$expected" == *":1:regular "* ]] || exit 2
  inherited="$(stat -Lc '%d:%i:%h:%F' "/proc/self/fd/$lock_fd")" || exit 2
  [[ "$inherited" == "$expected" ]] || exit 2
  /usr/bin/flock -n "$lock_fd"
  exit $?
fi
[[ "${1:-}" == "--" ]]
shift
if [[ "${KANBAN_CARGO_BUILD_LOCK_HELD:-}" == "1" ]]; then
  export CARGO_TARGET_DIR="$PACKAGE_TEST_TARGET_ROOT"
  exec "$@"
fi
[[ -z "${KANBAN_PACKAGE_BUILD_LOCK_HELD:-}" ]] || {
  echo "error: package forged its own build-lock marker" >&2
  exit 97
}
touch "$PACKAGE_WRAPPER_MARKER"
lock_path="$PACKAGE_TEST_TARGET_ROOT/.build.lock"
mkdir -p "$PACKAGE_TEST_TARGET_ROOT"
exec 3>"$lock_path"
/usr/bin/flock -n 3
export CARGO_TARGET_DIR="$PACKAGE_TEST_TARGET_ROOT"
export KANBAN_CARGO_BUILD_LOCK_HELD=1
export KANBAN_CARGO_BUILD_LOCK_PATH="$lock_path"
export KANBAN_CARGO_BUILD_LOCK_FD=3
export CARGO_BUILD_JOBS=2
export NEXTEST_TEST_THREADS=2
export RUST_TEST_THREADS=2
exec "$@"
EOF
  cat > "$fake_bin/cargo" <<'EOF'
#!/usr/bin/env bash
exit 86
EOF
  mkdir -p "$TMPDIR/package-marker-target"
  chmod +x "$repo/scripts/package-cli-linux.sh" \
    "$repo/scripts/cargo-build-lock.sh" "$fake_bin/cargo"

  set +e
  env \
    -u KANBAN_CARGO_BUILD_LOCK_HELD \
    -u KANBAN_PACKAGE_BUILD_LOCK_HELD \
    PATH="$fake_bin:$PATH" \
    PACKAGE_TEST_TARGET_ROOT="$TMPDIR/package-marker-target" \
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

assert_cli_package_stale_lock_fails_closed_without_mutation() {
  local repo="$TMPDIR/cli-package-stale-lock-repo"
  local fake_bin="$repo/bin"
  local trace="$repo/cargo.trace"
  local metadata_marker="$repo/metadata-entered"
  local invalidation_marker="$repo/invalidation-entered"
  local build_marker="$repo/build-entered"
  local provenance_marker="$repo/provenance-entered"
  local package_marker="$repo/package-entered"
  local before after first_call status

  mkdir -p "$repo/scripts" "$fake_bin"
  cp "$ROOT/scripts/package-cli-linux.sh" "$repo/scripts/package-cli-linux.sh"
  cp "$ROOT/scripts/cargo-build-lock.sh" "$repo/scripts/cargo-build-lock.sh"
  cp "$ROOT/scripts/release-safe-path.py" "$repo/scripts/release-safe-path.py"
  printf 'stale-lock\n' > "$repo/Cargo.lock"
  printf '# package fixture\n' > "$repo/README.md"

  cat > "$repo/scripts/package-source-provenance.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  --invalidate-packages)
    touch "$PACKAGE_INVALIDATION_MARKER"
    ;;
  --verify-dep-info)
    touch "$PACKAGE_PROVENANCE_MARKER"
    ;;
  *)
    exit 2
    ;;
esac
EOF
  cat > "$fake_bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "$PACKAGE_CARGO_TRACE"

has_locked=0
for argument in "$@"; do
  if [[ "$argument" == "--locked" ]]; then
    has_locked=1
    break
  fi
done

case "${1:-}" in
  pkgid)
    if [[ "$has_locked" == "1" && "$(cat "$PACKAGE_TEST_REPO/Cargo.lock")" == "stale-lock" ]]; then
      echo 'error: the lock file needs to be updated but --locked was passed' >&2
      exit 101
    fi
    if [[ "$has_locked" != "1" ]]; then
      printf 'resolved-lock\n' > "$PACKAGE_TEST_REPO/Cargo.lock"
    fi
    printf 'path+file:///fixture#2.1.3\n'
    ;;
  metadata)
    touch "$PACKAGE_METADATA_MARKER"
    [[ "$has_locked" == "1" ]]
    [[ "$(cat "$PACKAGE_TEST_REPO/Cargo.lock")" == "resolved-lock" ]]
    printf '{"packages":[{"name":"kanban-cli"},{"name":"kanban-vector-lancedb"},{"name":"kanban-graph-oxigraph"}]}\n'
    ;;
  build)
    touch "$PACKAGE_BUILD_MARKER"
    [[ "$has_locked" == "1" ]]
    [[ "$(cat "$PACKAGE_TEST_REPO/Cargo.lock")" == "resolved-lock" ]]
    target="$PACKAGE_TEST_TARGET_ROOT/release"
    mkdir -p "$target"
    if [[ " $* " == *' -p kanban-cli '* ]]; then
      touch "$target/kanban" "$target/kanban.d"
      chmod +x "$target/kanban"
    else
      for helper in kanban-vector-lancedb kanban-graph-oxigraph; do
        touch "$target/$helper" "$target/$helper.d"
        chmod +x "$target/$helper"
      done
    fi
    ;;
  *)
    exit 89
    ;;
esac
EOF
  cat > "$fake_bin/rustc" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf 'rustc 1.0.0\nhost: x86_64-unknown-linux-gnu\n'
EOF
  cat > "$fake_bin/dpkg-shlibdeps" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf 'shlibs:Depends=libc6\n'
EOF
  cat > "$fake_bin/dpkg-deb" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
touch "$PACKAGE_DPKG_MARKER"
output="${@: -1}"
mkdir -p "$(dirname "$output")"
touch "$output"
EOF
  chmod +x "$repo/scripts/package-cli-linux.sh" \
    "$repo/scripts/cargo-build-lock.sh" \
    "$repo/scripts/package-source-provenance.sh" \
    "$fake_bin/cargo" "$fake_bin/rustc" \
    "$fake_bin/dpkg-shlibdeps" "$fake_bin/dpkg-deb"

  before="$(sha256sum "$repo/Cargo.lock")"
  set +e
  env \
    -u CARGO_TARGET_DIR \
    -u KANBAN_CARGO_BUILD_LOCK_HELD \
    KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
    PACKAGE_TEST_REPO="$repo" \
    PACKAGE_TEST_TARGET_ROOT="$TARGET_ROOT" \
    PACKAGE_CARGO_TRACE="$trace" \
    PACKAGE_METADATA_MARKER="$metadata_marker" \
    PACKAGE_INVALIDATION_MARKER="$invalidation_marker" \
    PACKAGE_BUILD_MARKER="$build_marker" \
    PACKAGE_PROVENANCE_MARKER="$provenance_marker" \
    PACKAGE_DPKG_MARKER="$package_marker" \
    PATH="$fake_bin:$PATH" \
    "$repo/scripts/package-cli-linux.sh" --format deb >/dev/null 2>&1
  status=$?
  set -e
  after="$(sha256sum "$repo/Cargo.lock")"

  [[ "$before" == "$after" ]] ||
    fail "stale-lock cli-package let its first Cargo query mutate Cargo.lock"
  [[ "$status" -eq 101 ]] ||
    fail "expected stale-lock cli-package to fail closed with status 101, got $status"
  [[ -f "$trace" ]] || fail "stale-lock cli-package did not reach its first Cargo query"
  [[ "$(wc -l < "$trace")" -eq 1 ]] ||
    fail "stale-lock cli-package reached Cargo after its first locked query"
  first_call="$(head -n 1 "$trace")"
  [[ " $first_call " == *' --locked '* ]] ||
    fail "cli-package first Cargo query was not locked: $first_call"
  [[ ! -e "$metadata_marker" ]] || fail "stale-lock cli-package reached cargo metadata"
  [[ ! -e "$invalidation_marker" ]] || fail "stale-lock cli-package invalidated build artifacts"
  [[ ! -e "$build_marker" ]] || fail "stale-lock cli-package reached cargo build"
  [[ ! -e "$provenance_marker" ]] || fail "stale-lock cli-package reached dep-info verification"
  [[ ! -e "$package_marker" ]] || fail "stale-lock cli-package reached Debian assembly"
}

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

assert_package_layout_provenance_pairing() {
  local fixture_root="$TMPDIR/package-layout-provenance"
  local manifest="$fixture_root/source-provenance.json"
  local source_map="$fixture_root/derived-projection-v2-source-map.json"
  local manifest_bad="$fixture_root/source-provenance.bad.json"
  local source_map_bad="$fixture_root/derived-projection-v2-source-map.bad.json"
  local cli_root="$fixture_root/cli-root"
  local desktop_root="$fixture_root/desktop-root"
  local cli_deb="$fixture_root/kanban-tool-cli.deb"
  local desktop_deb="$fixture_root/kanban-tool-desktop.deb"
  local script package label mode output status expected

  mkdir -p "$cli_root/DEBIAN" "$cli_root/usr/bin" "$cli_root/usr/lib/kanban" \
    "$cli_root/usr/share/doc/kanban-tool-cli"
  chmod 0755 "$cli_root/DEBIAN"
  touch "$cli_root/usr/bin/kanban" \
    "$cli_root/usr/lib/kanban/kanban-vector-lancedb" \
    "$cli_root/usr/lib/kanban/kanban-graph-oxigraph" \
    "$cli_root/usr/share/doc/kanban-tool-cli/source-provenance.json" \
    "$cli_root/usr/share/doc/kanban-tool-cli/derived-projection-v2-source-map.json"
  chmod 0755 "$cli_root/usr/bin/kanban" "$cli_root/usr/lib/kanban"/*
  printf '%s\n' \
    'Package: kanban-tool-cli' \
    'Version: 1' \
    'Architecture: amd64' \
    'Maintainer: fixture <fixture@example.invalid>' \
    'Depends: libc6' \
    'Description: package-layout provenance fixture' > "$cli_root/DEBIAN/control"

  mkdir -p "$desktop_root/DEBIAN" "$desktop_root/usr/bin" \
    "$desktop_root/usr/share/doc/kanban-tool-desktop"
  chmod 0755 "$desktop_root/DEBIAN"
  touch "$desktop_root/usr/bin/kanban-desktop" \
    "$desktop_root/usr/bin/kanban-vector-lancedb" \
    "$desktop_root/usr/bin/kanban-graph-oxigraph" \
    "$desktop_root/usr/share/doc/kanban-tool-desktop/source-provenance.json" \
    "$desktop_root/usr/share/doc/kanban-tool-desktop/derived-projection-v2-source-map.json"
  chmod 0755 "$desktop_root/usr/bin"/*
  printf '%s\n' \
    'Package: kanban-tool-desktop' \
    'Version: 1' \
    'Architecture: amd64' \
    'Maintainer: fixture <fixture@example.invalid>' \
    'Description: package-layout provenance fixture' > "$desktop_root/DEBIAN/control"

  printf 'manifest fixture v1\n' > "$manifest"
  printf 'source-map fixture v1\n' > "$source_map"
  cp "$manifest" "$cli_root/usr/share/doc/kanban-tool-cli/source-provenance.json"
  cp "$source_map" "$cli_root/usr/share/doc/kanban-tool-cli/derived-projection-v2-source-map.json"
  cp "$manifest" "$desktop_root/usr/share/doc/kanban-tool-desktop/source-provenance.json"
  cp "$source_map" "$desktop_root/usr/share/doc/kanban-tool-desktop/derived-projection-v2-source-map.json"
  printf 'manifest fixture mismatch\n' > "$manifest_bad"
  printf 'source-map fixture mismatch\n' > "$source_map_bad"
  dpkg-deb --build --root-owner-group "$cli_root" "$cli_deb" >/dev/null
  dpkg-deb --build --root-owner-group "$desktop_root" "$desktop_deb" >/dev/null

  for script in "$ROOT/scripts/test-cli-package-layout.sh" \
    "$ROOT/scripts/test-desktop-package-layout.sh"; do
    if [[ "$script" == *test-cli-package-layout.sh ]]; then
      package="$cli_deb"
      label="CLI"
    else
      package="$desktop_deb"
      label="Desktop"
    fi

    for mode in neither both manifest-only map-only manifest-mismatch map-mismatch; do
      case "$mode" in
        neither|both)
          expected=0
          ;;
        manifest-only|map-only|manifest-mismatch|map-mismatch)
          expected=1
          ;;
      esac
      set +e
      case "$mode" in
        neither)
          output="$(env -u KANBAN_RELEASE_SOURCE_MANIFEST \
            -u KANBAN_RELEASE_SOURCE_MAP \
            KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
            "$script" "$package" 2>&1)"
          ;;
        both)
          output="$(env -u KANBAN_RELEASE_SOURCE_MANIFEST \
            -u KANBAN_RELEASE_SOURCE_MAP \
            KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
            KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
            KANBAN_RELEASE_SOURCE_MAP="$source_map" \
            "$script" "$package" 2>&1)"
          ;;
        manifest-only)
          output="$(env -u KANBAN_RELEASE_SOURCE_MAP \
            KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
            KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
            "$script" "$package" 2>&1)"
          ;;
        map-only)
          output="$(env -u KANBAN_RELEASE_SOURCE_MANIFEST \
            KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
            KANBAN_RELEASE_SOURCE_MAP="$source_map" \
            "$script" "$package" 2>&1)"
          ;;
        manifest-mismatch)
          output="$(env -u KANBAN_RELEASE_SOURCE_MANIFEST \
            -u KANBAN_RELEASE_SOURCE_MAP \
            KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
            KANBAN_RELEASE_SOURCE_MANIFEST="$manifest_bad" \
            KANBAN_RELEASE_SOURCE_MAP="$source_map" \
            "$script" "$package" 2>&1)"
          ;;
        map-mismatch)
          output="$(env -u KANBAN_RELEASE_SOURCE_MANIFEST \
            -u KANBAN_RELEASE_SOURCE_MAP \
            KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
            KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
            KANBAN_RELEASE_SOURCE_MAP="$source_map_bad" \
            "$script" "$package" 2>&1)"
          ;;
      esac
      status=$?
      set -e
      [[ "$status" -eq "$expected" ]] || {
        echo "$output" >&2
        fail "$label package-layout $mode expected status $expected, got $status"
      }
      case "$mode" in
        manifest-only)
          [[ "$output" == *"requires KANBAN_RELEASE_SOURCE_MAP when KANBAN_RELEASE_SOURCE_MANIFEST is set"* ]] ||
            fail "$label package-layout manifest-only mutation lacks a diagnostic pairing error"
          ;;
        map-only)
          [[ "$output" == *"requires KANBAN_RELEASE_SOURCE_MANIFEST when KANBAN_RELEASE_SOURCE_MAP is set"* ]] ||
            fail "$label package-layout map-only mutation lacks a diagnostic pairing error"
          ;;
        manifest-mismatch)
          [[ "$output" == *"$label package source provenance does not match the release cohort"* ]] ||
            fail "$label package-layout did not compare the source manifest"
          ;;
        map-mismatch)
          [[ "$output" == *"$label package source map does not match the release cohort"* ]] ||
            fail "$label package-layout did not compare the source map"
          ;;
      esac
    done
  done
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

assert_release_resource_environment_compatibility() {
  local source_gate="$ROOT/scripts/release-source-gate.sh"

  # Cloud setup's canonical auto policy intentionally leaves all three
  # tool-specific variables unset; the release entrypoint must accept that
  # inherited lock shape just as it accepts the wrapper's explicit 2/2/2
  # defaults.
  env -u CARGO_BUILD_JOBS -u NEXTEST_TEST_THREADS -u RUST_TEST_THREADS \
    KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
    KANBAN_CARGO_BUILD_JOBS=auto KANBAN_TEST_THREADS=auto \
    "$LOCK_SCRIPT" -- "$source_gate" --help >/dev/null

  env -u KANBAN_CARGO_BUILD_JOBS -u KANBAN_TEST_THREADS \
    KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
    "$LOCK_SCRIPT" -- "$source_gate" --help >/dev/null

  assert_failure env KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
    CARGO_BUILD_JOBS=9 "$LOCK_SCRIPT" -- "$source_gate" --help
  assert_failure env -u NEXTEST_TEST_THREADS -u RUST_TEST_THREADS \
    KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
    KANBAN_CARGO_BUILD_JOBS=auto KANBAN_TEST_THREADS=auto \
    CARGO_BUILD_JOBS=2 "$LOCK_SCRIPT" -- "$source_gate" --help
  assert_failure env KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
    NEXTEST_TEST_THREADS=2 RUST_TEST_THREADS=9 \
    "$LOCK_SCRIPT" -- "$source_gate" --help
  assert_failure env KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
    KANBAN_CARGO_BUILD_JOBS=9 KANBAN_TEST_THREADS=auto \
    "$LOCK_SCRIPT" -- "$source_gate" --help

  # A release entrypoint invoked directly, without the wrapper-owned marker,
  # must continue to reject caller-supplied Cargo resource overrides.
  assert_failure env -u KANBAN_CARGO_BUILD_LOCK_HELD \
    CARGO_BUILD_JOBS=2 "$source_gate" --help
}

assert_dev_profile_disables_incremental_without_test_override() {
  python3 -B - "$ROOT/Cargo.toml" <<'PY'
import sys
import tomllib
from pathlib import Path

manifest_path = Path(sys.argv[1])
with manifest_path.open("rb") as manifest_file:
    manifest = tomllib.load(manifest_file)

profiles = manifest.get("profile", {})
dev = profiles.get("dev")
if not isinstance(dev, dict) or dev.get("incremental") is not False:
    raise SystemExit(
        "root Cargo.toml must set [profile.dev].incremental = false"
    )
if "test" in profiles:
    raise SystemExit(
        "root Cargo.toml must not override [profile.test]; Cargo test inherits [profile.dev]"
    )
PY
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
default_expected_target="$(env -u KANBAN_CARGO_TARGET_ROOT -u CARGO_TARGET_DIR HOME="$home_dir" "$LOCK_SCRIPT" --print-target-dir)"
[[ "$default_expected_target" == "$home_target" ]] || fail "default target root must be portable under HOME"
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
KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" "$LOCK_SCRIPT" -- "$LOCK_SCRIPT" -- bash -c '
  set -euo pipefail
  "$1" --verify-inherited-lock
  setsid "$1" --verify-inherited-lock
  touch "$2"
' _ "$LOCK_SCRIPT" "$outer_lock_marker"
[[ -e "$outer_lock_marker" ]] || fail "nested lock-held command did not run"

assert_inherited_lock_proof_edges() {
  local closed_marker="$TMPDIR/closed-proof.marker"
  local reopened_marker="$TMPDIR/reopened-proof.marker"
  local mismatch_marker="$TMPDIR/mismatch-proof.marker"
  local spoof_bin="$TMPDIR/lock-proof-path-spoof-bin"

  # A marker and exact target without the inherited descriptor must fail
  # before the verifier creates the target or lock file.
  local spoof_target="$TMPDIR/proof-spoof-target"
  assert_failure env \
    KANBAN_CARGO_TARGET_ROOT="$spoof_target" \
    CARGO_TARGET_DIR="$spoof_target" \
    KANBAN_CARGO_BUILD_LOCK_HELD=1 \
    "$LOCK_SCRIPT" --verify-inherited-lock
  [[ ! -e "$spoof_target" ]] || fail "spoofed lock proof created target state"

  # The verifier contract is lexical as well as physical: a trailing-slash
  # alias for CARGO_TARGET_DIR is not an inherited canonical target proof.
  assert_failure env KANBAN_CARGO_TARGET_ROOT="$TARGET_ROOT" \
    "$LOCK_SCRIPT" -- bash -c '
      CARGO_TARGET_DIR="$CARGO_TARGET_DIR/" "$1" --verify-inherited-lock
    ' _ "$LOCK_SCRIPT"

  # PATH cannot replace the trusted lock verifier primitives.  A fake flock
  # and stat that always report success must not bless an unlocked descriptor.
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
  [[ -e "$closed_marker" ]] || fail "closed inherited lock descriptor was not rejected"
  [[ -e "$reopened_marker" ]] || fail "reopened inherited lock descriptor was not rejected"
  [[ -e "$mismatch_marker" ]] || fail "mismatched inherited lock descriptor was not rejected"
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
  [[ "$status" -ne 0 ]] || fail "inherited lock path replacement was accepted after flock"
  [[ "$(cat "$race_target/.build.lock")" == "replacement" ]] ||
    fail "inherited lock race did not preserve replacement path"
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
    fail "inherited lock symlink replacement was accepted after flock"
  [[ -L "$race_target/.build.lock" ]] ||
    fail "inherited lock symlink race did not preserve the replacement"
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
    fail "fresh symlink lock acquisition modified the sentinel"
  [[ ! -e "$child" ]] || fail "fresh symlink lock acquisition ran the child"

  child="$TMPDIR/fresh-hardlink-child"
  ln "$sentinel" "$hardlink_target/.build.lock"
  assert_failure env KANBAN_CARGO_TARGET_ROOT="$hardlink_target" \
    "$LOCK_SCRIPT" -- bash -c 'touch "$1"' _ "$child"
  [[ "$(cat "$sentinel")" == "sentinel-preserved" ]] ||
    fail "fresh hardlink lock acquisition modified the sentinel"
  [[ ! -e "$child" ]] || fail "fresh hardlink lock acquisition ran the child"

  child="$TMPDIR/fresh-special-child"
  mkfifo "$special_target/.build.lock"
  assert_failure timeout 3 env KANBAN_CARGO_TARGET_ROOT="$special_target" \
    "$LOCK_SCRIPT" -- bash -c 'touch "$1"' _ "$child"
  [[ ! -e "$child" ]] || fail "fresh special-file lock acquisition ran the child"

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
  [[ "$status" -ne 0 ]] || fail "fresh lock path race ran the child"
  [[ ! -e "$race_child" ]] || fail "fresh lock path race ran the child"
  [[ "$(cat "$sentinel")" == "sentinel-preserved" ]] ||
    fail "fresh lock path race modified the sentinel"
}

assert_inherited_lock_proof_edges
assert_inherited_lock_post_flock_identity_race
assert_inherited_lock_post_flock_symlink_race
assert_fresh_lock_path_safety

assert_target_tools_safe_path_gate_order() {
  local -a recipe=()
  local line safe_path_count=0 provenance_count=0 safe_path_index=-1 provenance_index=-1

  mapfile -t recipe < <(
    awk '
      $0 == "target-tools:" { in_recipe=1; next }
      in_recipe && $0 !~ /^[[:space:]]/ { exit }
      in_recipe { print }
    ' "$ROOT/justfile"
  )

  local index=0
  for line in "${recipe[@]}"; do
    case "$line" in
      "    python3 -B scripts/test_release_safe_path.py")
        safe_path_count=$((safe_path_count + 1))
        safe_path_index=$index
        ;;
      "    scripts/test-release-provenance.sh")
        provenance_count=$((provenance_count + 1))
        provenance_index=$index
        ;;
    esac
    index=$((index + 1))
  done

  [[ "$safe_path_count" -eq 1 ]] ||
    fail "target-tools recipe must invoke standalone release safe-path tests exactly once"
  [[ "$provenance_count" -eq 1 ]] ||
    fail "target-tools recipe must invoke complete release provenance gate exactly once"
  [[ "$safe_path_index" -lt "$provenance_index" ]] ||
    fail "standalone release safe-path tests must run before complete release provenance gate"
}

assert_target_tools_safe_path_gate_order
assert_dev_profile_disables_incremental_without_test_override

assert_distinct_worktrees_share_target_and_lock
assert_package_lock_marker_is_wrapper_owned
assert_package_waits_for_shared_build_lock
assert_cli_package_stale_lock_fails_closed_without_mutation
assert_package_source_provenance_is_current_and_non_mutating
assert_package_layout_provenance_pairing
assert_schema_cargo_lanes_stale_lock_fail_without_mutation
assert_resource_limit_defaults
assert_release_resource_environment_compatibility
assert_no_bare_target_writing_cargo
assert_target_dir_probe_call_sites_quote_paths
assert_target_dir_probe_handles_space_paths
assert_package_help_output_path
assert_debian_control_directory_mode
assert_nextest_junit_stays_under_shared_target

echo "cargo target root and build lock tests passed"
