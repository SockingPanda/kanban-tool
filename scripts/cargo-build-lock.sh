#!/usr/bin/env bash
set -euo pipefail

RAW_TARGET_ROOT="${KANBAN_CARGO_TARGET_ROOT:-$HOME/.cache/kanban-tool/cargo-target}"
CHILD_PID=""
CHILD_PGID=""
COMMAND=()
NEXTEST_RUN=0
LOCK_FD=9

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
  local lock_fd expected_identity inherited_identity metadata status

  [[ "${KANBAN_CARGO_BUILD_LOCK_HELD:-}" == "1" ]] || return 1
  [[ "${CARGO_TARGET_DIR:-}" == "$expected_target" ]] || return 1
  [[ "${KANBAN_CARGO_BUILD_LOCK_PATH:-}" == "$expected_lock" ]] || return 1
  lock_fd="${KANBAN_CARGO_BUILD_LOCK_FD:-}"
  [[ "$lock_fd" =~ ^[3-9][0-9]*$ ]] || return 1
  [[ -e "/proc/self/fd/$lock_fd" ]] || return 1

  [[ -f "$expected_lock" && ! -L "$expected_lock" ]] || return 1
  metadata="$(/usr/bin/stat -Lc '%d:%i:%h:%F' -- "$expected_lock" 2>/dev/null)" || return 1
  [[ "$metadata" == *":1:regular "* ]] || return 1
  expected_identity="${metadata%:*:*}"
  inherited_identity="$(/usr/bin/stat -Lc '%d:%i:%h:%F' -- "/proc/self/fd/$lock_fd" 2>/dev/null)" || return 1
  [[ "$inherited_identity" == "$expected_identity:1:regular "* ]] || return 1

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
  [[ -f "$expected_lock" && ! -L "$expected_lock" ]] || return 1
  metadata="$(/usr/bin/stat -c '%d:%i:%h:%F' -- "$expected_lock" 2>/dev/null)" || return 1
  [[ "$metadata" == *":1:regular "* ]] || return 1
  [[ "${metadata%:*:*}" == "$expected_identity" ]] || return 1
  inherited_identity="$(/usr/bin/stat -Lc '%d:%i:%h:%F' -- "/proc/self/fd/$lock_fd" 2>/dev/null)" || return 1
  [[ "$inherited_identity" == "$expected_identity:1:regular "* ]] || return 1
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
  if [[ ! -x /usr/bin/python3 ]]; then
    error "/usr/bin/python3 is required for kanban-tool Cargo build locking"
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

  export CARGO_TARGET_DIR="$target_dir"
  export KANBAN_CARGO_BUILD_LOCK_HELD=1
  export KANBAN_CARGO_BUILD_LOCK_FD="$LOCK_FD"
  export KANBAN_CARGO_BUILD_LOCK_PATH="$lock_file"
  /usr/bin/setsid /usr/bin/python3 - "$lock_file" "$LOCK_FD" \
    "$NEXTEST_RUN" "${COMMAND[@]}" <<'PY' &
import fcntl
import os
import stat
import sys
import tempfile


def fail(message: str) -> "NoReturn":
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(2)


lock_path = sys.argv[1]
requested_fd = int(sys.argv[2])
nextest_run = sys.argv[3] == "1"
command = sys.argv[4:]
if not command:
    fail("missing command for Cargo build lock")

flags = (
    os.O_RDWR
    | os.O_CREAT
    | os.O_CLOEXEC
    | os.O_NOFOLLOW
    | os.O_NONBLOCK
)
descriptor = -1
try:
    try:
        descriptor = os.open(lock_path, flags, 0o666)
    except OSError as error:
        fail(f"cannot open Cargo build lock without following symlinks: {lock_path}: {error}")

    def assert_identity() -> None:
        try:
            path_metadata = os.lstat(lock_path)
        except OSError as error:
            fail(f"cannot inspect Cargo build lock safely: {lock_path}: {error}")
        descriptor_metadata = os.fstat(descriptor)
        try:
            proc_metadata = os.stat(f"/proc/self/fd/{descriptor}")
        except OSError as error:
            fail(f"cannot inspect inherited Cargo build lock descriptor: {error}")
        for label, metadata in (
            ("path", path_metadata),
            ("descriptor", descriptor_metadata),
            ("proc descriptor", proc_metadata),
        ):
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
                fail(
                    "Cargo build lock must be a single-linked regular file "
                    f"({label}): {lock_path}"
                )
        expected = (path_metadata.st_dev, path_metadata.st_ino)
        if any(
            (metadata.st_dev, metadata.st_ino) != expected
            for metadata in (descriptor_metadata, proc_metadata)
        ):
            fail(f"Cargo build lock path and descriptor identity differ: {lock_path}")

    assert_identity()
    try:
        fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        print(f"正在等待其他构建/测试释放 Cargo target 锁：{lock_path}", file=sys.stderr)
        fcntl.flock(descriptor, fcntl.LOCK_EX)
    assert_identity()

    if descriptor != requested_fd:
        os.dup2(descriptor, requested_fd, inheritable=True)
        os.close(descriptor)
        descriptor = requested_fd
    else:
        os.set_inheritable(descriptor, True)
    assert_identity()

    if nextest_run:
        target_dir = os.environ["CARGO_TARGET_DIR"]
        config_path = os.path.join(target_dir, ".nextest.toml")
        store_dir = os.path.join(target_dir, "nextest")
        store_dir = store_dir.replace("\\", "\\\\").replace('"', '\\"')
        with open(".config/nextest.toml", encoding="utf-8") as source:
            config = f'[store]\ndir = "{store_dir}"\n\n' + source.read()
        temporary_fd, temporary_path = tempfile.mkstemp(
            prefix=".nextest.toml.", dir=target_dir
        )
        try:
            with os.fdopen(temporary_fd, "w", encoding="utf-8") as output:
                output.write(config)
            os.replace(temporary_path, config_path)
        except BaseException:
            try:
                os.unlink(temporary_path)
            except FileNotFoundError:
                pass
            raise
        command = [
            "cargo",
            "nextest",
            "run",
            "--config-file",
            config_path,
            "--target-dir",
            target_dir,
            *command[3:],
        ]

    try:
        child_pid = os.fork()
    except OSError as error:
        fail(f"cannot fork Cargo build command: {error}")
    if child_pid == 0:
        try:
            os.execvpe(command[0], command, os.environ)
        except OSError as error:
            fail(f"cannot execute Cargo build command: {command[0]}: {error}")
    _, child_status = os.waitpid(child_pid, 0)
    if os.WIFEXITED(child_status):
        raise SystemExit(os.WEXITSTATUS(child_status))
    if os.WIFSIGNALED(child_status):
        raise SystemExit(128 + os.WTERMSIG(child_status))
    fail("Cargo build command ended with an unknown wait status")
finally:
    if descriptor >= 0:
        os.close(descriptor)
PY
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
