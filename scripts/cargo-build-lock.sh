#!/usr/bin/env bash
set -euo pipefail

RAW_TARGET_ROOT="${KANBAN_CARGO_TARGET_ROOT:-$HOME/.cache/kanban-tool/cargo-target}"
CHILD_PID=""
CHILD_PGID=""
COMMAND=()
NEXTEST_RUN=0
LOCK_FD=9

# Bash 后台 job 可能继承父 shell 的 ignored SIGINT；先恢复默认 disposition，
# 之后的 wrapper trap 才能把 INT/TERM/HUP 转发到 setsid 建立的 child group。
trap - INT TERM HUP

usage() {
  cat <<'USAGE'
Usage:
  scripts/cargo-build-lock.sh -- <command> [args...]
  scripts/cargo-build-lock.sh --print-target-dir

Run a Cargo build/test/check/clippy/nextest command after acquiring the
kanban-tool local build lock. The wrapper serializes commands that write under
the shared Cargo target root and exports one exact CARGO_TARGET_DIR for every
worktree so the shared build lock protects the shared artifacts.

Options:
  --print-target-dir  Print the exact shared Cargo target directory and exit.
  --verify-inherited-lock  Verify an already-held inherited lock and exit.
  -h, --help          Show this help.

Environment:
  CARGO_TARGET_DIR            If set, it must equal the configured shared
                              target root.
  CARGO_BUILD_JOBS            Cargo build jobs passed through when set.
                              Default: ${KANBAN_CARGO_BUILD_JOBS:-2}
  NEXTEST_TEST_THREADS        cargo-nextest test threads passed through when set.
                              Default: ${KANBAN_TEST_THREADS:-2}
  RUST_TEST_THREADS           libtest threads passed through when set.
                              Default: ${KANBAN_TEST_THREADS:-2}
  KANBAN_CARGO_TARGET_ROOT    Override target root for local tests. The wrapper
                              uses this exact directory for every worktree while
                              keeping one shared build lock.
                              Default: $HOME/.cache/kanban-tool/cargo-target
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

prepare_nextest_command() {
  local config_path="$target_dir/.nextest.toml"
  local temporary_config
  local toml_store_dir="$target_dir/nextest"

  toml_store_dir="${toml_store_dir//\\/\\\\}"
  toml_store_dir="${toml_store_dir//\"/\\\"}"
  temporary_config="$(mktemp "$target_dir/.nextest.toml.XXXXXX")"
  {
    printf '[store]\ndir = "%s"\n\n' "$toml_store_dir"
    cat .config/nextest.toml
  } >"$temporary_config"
  mv "$temporary_config" "$config_path"
  COMMAND=(cargo nextest run --config-file "$config_path" --target-dir "$target_dir" "${COMMAND[@]:3}")
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

lock_proof_is_valid() {
  local expected_lock="$1" expected_target="$2"
  local lock_fd status

  [[ "${KANBAN_CARGO_BUILD_LOCK_HELD:-}" == "1" ]] || return 1
  [[ "${CARGO_TARGET_DIR:-}" == "$expected_target" ]] || return 1
  [[ "${KANBAN_CARGO_BUILD_LOCK_PATH:-}" == "$expected_lock" ]] || return 1
  lock_fd="${KANBAN_CARGO_BUILD_LOCK_FD:-}"
  [[ "$lock_fd" =~ ^[3-9][0-9]*$ ]] || return 1
  [[ -e "/proc/self/fd/$lock_fd" ]] || return 1

  lock_identity_is_valid "$expected_lock" "$lock_fd" || return 1

  # `flock -n <fd>` 作用于继承的 open file description。锁持有者会成功；
  # 如果调用方提供了尚未加锁的 descriptor，它也会原子地获取同一个 canonical
  # lock；无论哪种情况，该 proof 都会让本进程在整个生命周期持有 exclusive lock。
  if [[ "${KANBAN_CARGO_BUILD_LOCK_TEST_PAUSE_BEFORE_FLOCK:-0}" == "1" ]]; then
    local pause_marker="${KANBAN_CARGO_BUILD_LOCK_TEST_PAUSE_MARKER:-}"
    local pause_continue="${KANBAN_CARGO_BUILD_LOCK_TEST_CONTINUE:-}"
    [[ -n "$pause_marker" && -n "$pause_continue" ]] || return 1
    : > "$pause_marker"
    while [[ ! -e "$pause_continue" ]]; do
      sleep 0.01
    done
  fi
  /usr/bin/flock -n "$lock_fd" >/dev/null 2>&1
  status=$?
  [[ "$status" -eq 0 ]] || return "$status"

  # 不跟随 symlink 重新检查绝对路径：descriptor 完成 flock 后，不得接受
  # 指向原始 inode 的 symlink。
  lock_identity_is_valid "$expected_lock" "$lock_fd" || return 1
}

lock_identity_is_valid() {
  local lock_path="$1" lock_fd="$2"
  local path_metadata descriptor_metadata expected_identity

  [[ -f "$lock_path" && ! -L "$lock_path" ]] || return 1
  path_metadata="$(/usr/bin/stat -Lc '%d:%i:%h:%F' -- "$lock_path" 2>/dev/null)" || return 1
  [[ "$path_metadata" == *":1:regular "* ]] || return 1
  expected_identity="${path_metadata%:*:*}"

  [[ -e "/proc/self/fd/$lock_fd" ]] || return 1
  descriptor_metadata="$(/usr/bin/stat -Lc '%d:%i:%h:%F' -- "/proc/self/fd/$lock_fd" 2>/dev/null)" || return 1
  [[ "$descriptor_metadata" == "$expected_identity:1:regular "* ]] || return 1
}

lock_path_is_cooperative_before_open() {
  local lock_path="$1" metadata

  # Bash 的 pathname redirection 没有 O_NOFOLLOW；这里的 pre/post 检查只在
  # dedicated、cooperative target owner 模型下提供路径身份护栏，不声称抵抗
  # hostile same-UID 在检查与 open 之间的 TOCTOU。更强的 adversarial 文件系统
  # 模型需要保留具备 openat(O_NOFOLLOW) syscall 语义的独立 owner。
  [[ ! -L "$lock_path" ]] || return 1
  if [[ -e "$lock_path" ]]; then
    [[ -f "$lock_path" ]] || return 1
    metadata="$(/usr/bin/stat -Lc '%d:%i:%h:%F' -- "$lock_path" 2>/dev/null)" || return 1
    [[ "$metadata" == *":1:regular "* ]] || return 1
  fi
}


validate_inherited_target_dir() {
  local expected="$1"
  local actual

  if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
    return 0
  fi

  actual="$(normalize_path "$(expand_home_path "$CARGO_TARGET_DIR")")"
  if [[ "$actual" == "$expected" ]]; then
    return 0
  fi

  error "CARGO_TARGET_DIR must equal the kanban-tool shared target root: $expected"
  error "got: $CARGO_TARGET_DIR"
  return 2
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
  local target_root_dir=""
  local target_dir=""
  local lock_file=""
  local lock_dir
  local status
  local verify_inherited_lock=0

  target_root_dir="$(target_root)"
  validate_inherited_target_dir "$target_root_dir"
  target_dir="$target_root_dir"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      -h|--help)
        usage
        exit 0
        ;;
      --print-target-dir)
        printf '%s\n' "$target_dir"
        exit 0
        ;;
      --verify-inherited-lock)
        verify_inherited_lock=1
        shift
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

  lock_file="$target_root_dir/.build.lock"

  if [[ "$verify_inherited_lock" == "1" ]]; then
    [[ $# -eq 0 ]] || {
      error "--verify-inherited-lock does not accept a command"
      exit 2
    }
    lock_proof_is_valid "$lock_file" "$target_root_dir" || {
      error "inherited Cargo build lock proof is invalid"
      exit 2
    }
    exit 0
  fi

  if [[ $# -eq 0 ]]; then
    error "missing command after --"
    usage >&2
    exit 2
  fi

  COMMAND=("$@")
  if [[ "${1:-}" == "cargo" && "${2:-}" == "nextest" && "${3:-}" == "run" ]]; then
    NEXTEST_RUN=1
    for argument in "${@:4}"; do
      if [[ "$argument" == "--target-dir" || "$argument" == --target-dir=* || "$argument" == "--config-file" || "$argument" == --config-file=* ]]; then
        error "cargo nextest target dir and config are managed by the shared-target wrapper"
        exit 2
      fi
    done
  fi

  if [[ ! -x /usr/bin/flock ]]; then
    error "/usr/bin/flock is required for kanban-tool Cargo build locking"
    exit 1
  fi
  if [[ ! -x /usr/bin/setsid ]]; then
    error "/usr/bin/setsid is required for kanban-tool Cargo build locking"
    exit 1
  fi
  if [[ "${KANBAN_CARGO_BUILD_LOCK_HELD:-}" == "1" ]]; then
    if ! lock_proof_is_valid "$lock_file" "$target_root_dir"; then
      error "KANBAN_CARGO_BUILD_LOCK_HELD requires an inherited lock proof"
      exit 2
    fi
    export CARGO_TARGET_DIR="$target_dir"
    if [[ "$NEXTEST_RUN" == "1" ]]; then
      prepare_nextest_command
    fi
    "${COMMAND[@]}"
    exit $?
  fi

  lock_dir="$(dirname "$lock_file")"
  mkdir -p "$lock_dir" "$target_dir"
  configure_resource_limits

  if ! lock_path_is_cooperative_before_open "$lock_file"; then
    error "Cargo build lock 必须是 single-linked regular file 且不得是 symlink：$lock_file"
    exit 1
  fi
  # Shell redirection 不能原子地表达 O_NOFOLLOW；在 dedicated/cooperative
  # target owner 约束内，以 fixed FD9 打开后立即做 inode、regular、nlink 和
  # /proc/self/fd identity 的 post-check。该降级不抵抗 hostile same-UID TOCTOU。
  if ! exec 9<> "$lock_file"; then
    error "无法打开 Cargo build lock：$lock_file"
    exit 1
  fi
  if ! lock_identity_is_valid "$lock_file" "$LOCK_FD"; then
    error "Cargo build lock open 后身份校验失败：$lock_file"
    exit 1
  fi

  set +e
  /usr/bin/flock -n "$LOCK_FD" >/dev/null 2>&1
  status=$?
  set -e
  case "$status" in
    0)
      ;;
    1)
      echo "正在等待其他构建/测试释放 Cargo target 锁：$lock_file" >&2
      /usr/bin/flock "$LOCK_FD"
      ;;
    *)
      error "无法获取 Cargo target 锁：$lock_file (flock status $status)"
      exit 1
      ;;
  esac
  if ! lock_identity_is_valid "$lock_file" "$LOCK_FD"; then
    error "Cargo build lock flock 后身份校验失败：$lock_file"
    exit 1
  fi

  if [[ "$NEXTEST_RUN" == "1" ]]; then
    prepare_nextest_command
  fi
  export CARGO_TARGET_DIR="$target_dir"
  export KANBAN_CARGO_BUILD_LOCK_HELD=1
  export KANBAN_CARGO_BUILD_LOCK_FD="$LOCK_FD"
  export KANBAN_CARGO_BUILD_LOCK_PATH="$lock_file"

  /usr/bin/setsid "${COMMAND[@]}" &
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
