#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
PACKAGE_NAME="kanban-tool-cli"
BIN_NAME="kanban"
REVISION="1"
BUILD_ARGS=()
LOCK="$ROOT/scripts/cargo-build-lock.sh"
PROVENANCE="$ROOT/scripts/package-source-provenance.sh"
SAFE_PATH="$ROOT/scripts/release-safe-path.py"
ORIGINAL_ARGS=("$@")

lock_environment_is_internal() {
  local expected_target

  [[ "${KANBAN_CARGO_BUILD_LOCK_HELD:-0}" == "1" ]] || return 1
  [[ -n "${CARGO_TARGET_DIR:-}" ]] || return 1
  expected_target="$("$LOCK" --print-target-dir 2>/dev/null)" || return 1
  [[ "$CARGO_TARGET_DIR" == "$expected_target" ]] || return 1
  "$LOCK" --verify-inherited-lock >/dev/null 2>&1
  lock_resource_environment_is_internal
}

lock_resource_environment_is_internal() {
  local build_policy="${KANBAN_CARGO_BUILD_JOBS:-}"
  local test_policy="${KANBAN_TEST_THREADS:-}"
  local build_set=0 nextest_set=0 rust_set=0

  [[ -n "${CARGO_BUILD_JOBS+x}" ]] && build_set=1
  [[ -n "${NEXTEST_TEST_THREADS+x}" ]] && nextest_set=1
  [[ -n "${RUST_TEST_THREADS+x}" ]] && rust_set=1
  case "$build_policy" in ""|2|auto|AUTO) ;; *) return 1 ;; esac
  case "$test_policy" in ""|2|auto|AUTO) ;; *) return 1 ;; esac

  if [[ "$build_set" == "0" && "$nextest_set" == "0" && "$rust_set" == "0" ]]; then
    [[ "$build_policy" != "2" && "$test_policy" != "2" ]]
    return
  fi
  [[ "$build_set" == "1" && "$nextest_set" == "1" && "$rust_set" == "1" ]] || return 1
  [[ "${CARGO_BUILD_JOBS:-}" == "2" &&
    "${NEXTEST_TEST_THREADS:-}" == "2" &&
    "${RUST_TEST_THREADS:-}" == "2" ]] || return 1
  [[ "$build_policy" != "auto" && "$build_policy" != "AUTO" &&
    "$test_policy" != "auto" && "$test_policy" != "AUTO" ]]
}

require_inherited_lock_if_marked() {
  if [[ "${KANBAN_CARGO_BUILD_LOCK_HELD:-}" == "1" ]] &&
    ! lock_environment_is_internal; then
    echo "error: release package 必须拥有 inherited Cargo build lock proof" >&2
    exit 1
  fi
}

usage() {
  cat <<'EOF'
Usage: scripts/package-cli-linux.sh [OPTIONS]

构建 kanban 的 standalone Linux CLI package。

Options:
  --format <deb>               要构建的 package format。默认值：deb。
  --features <features>        将逗号分隔的 feature list 传给 cargo。
  --all-features               将 --all-features 传给 cargo。
  --no-default-features        将 --no-default-features 传给 cargo。
  -h, --help                   显示此帮助。

Outputs:
  在 scripts/cargo-build-lock.sh --print-target-dir 打印的 target directory 下：
  release/bundle/cli/deb/*.deb
EOF
}

reject_release_build_environment() {
  local name
  local -a names
  mapfile -t names < <(compgen -A variable)
  for name in "${names[@]}"; do
    case "$name" in
      RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER|RUSTFLAGS|CARGO_ENCODED_RUSTFLAGS|\
      RUSTDOCFLAGS|CARGO_ENCODED_RUSTDOCFLAGS|RUSTC_BOOTSTRAP|SOURCE_DATE_EPOCH|\
      RUSTUP_TOOLCHAIN|RUSTUP_HOME|RUSTUP_DIST_SERVER|RUSTUP_UPDATE_ROOT|\
      CARGO_HOME|CARGO_BUILD_RUSTC|CARGO_BUILD_RUSTC_WRAPPER|CARGO_BUILD_RUSTDOC|\
      CC|CXX|AR|CFLAGS|CXXFLAGS|CPPFLAGS|LDFLAGS|PKG_CONFIG_PATH|PKG_CONFIG_LIBDIR)
        if [[ "${!name+x}" == "x" ]]; then
          echo "error: release package 拒绝会影响构建的 environment override: $name" >&2
          exit 1
        fi
        ;;
      CARGO_TARGET_*)
        if [[ "$name" != "CARGO_TARGET_DIR" ]] ||
          ! lock_environment_is_internal; then
          if [[ "${!name+x}" == "x" ]]; then
            echo "error: release package 拒绝会影响构建的 environment override: $name" >&2
            exit 1
          fi
        fi
        ;;
      CARGO_BUILD_JOBS|NEXTEST_TEST_THREADS|RUST_TEST_THREADS)
        if ! lock_environment_is_internal; then
          if [[ "${!name+x}" == "x" ]]; then
            echo "error: release package 拒绝会影响构建的 environment override: $name" >&2
            exit 1
          fi
        fi
        ;;
      CARGO_BUILD_TARGET)
        ;;
      CARGO_BUILD_*|CARGO_HTTP_*|CARGO_NET_*|CARGO_PROFILE_*|CARGO_REGISTRIES_*|\
      CARGO_SOURCE_*|RUSTUP_*|CC_*|CXX_*|PKG_CONFIG_*)
        if [[ "${!name+x}" == "x" ]]; then
          echo "error: release package 拒绝会影响构建的 environment override: $name" >&2
          exit 1
        fi
        ;;
    esac
  done
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --format)
      [[ $# -ge 2 ]] || { echo "error: --format 需要一个值" >&2; exit 2; }
      [[ "$2" == "deb" ]] || { echo "error: 仅支持 --format deb" >&2; exit 2; }
      shift 2
      ;;
    --features)
      [[ $# -ge 2 ]] || { echo "error: --features 需要一个值" >&2; exit 2; }
      BUILD_ARGS+=(--features "$2")
      shift 2
      ;;
    --all-features)
      BUILD_ARGS+=(--all-features)
      shift
      ;;
    --no-default-features)
      BUILD_ARGS+=(--no-default-features)
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: 未知选项：$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

require_inherited_lock_if_marked

reject_release_build_environment

validate_target_root_hint() {
  local target_hint="$1"
  python3 - "$target_hint" "$ROOT" <<'PY'
import os
import pathlib
import stat
import sys

raw_target = sys.argv[1]
source_root = pathlib.Path(sys.argv[2])
lexical_target = pathlib.PurePath(raw_target)

if not lexical_target.is_absolute():
    raise SystemExit(
        f"error: Cargo target root 在 lock handoff 前必须是绝对路径：{raw_target}"
    )
if lexical_target.anchor != os.path.sep or raw_target.startswith("//"):
    raise SystemExit(
        "error: Cargo target root 必须恰好使用一个前导斜杠："
        f"{raw_target}"
    )
if ".." in lexical_target.parts:
    raise SystemExit(
        f"error: Cargo target root 不得包含 parent traversal：{raw_target}"
    )

normalized_target = pathlib.Path(os.path.normpath(raw_target))
if normalized_target == pathlib.Path(normalized_target.anchor):
    raise SystemExit("error: Cargo target root 不得是 filesystem root")

# 只遍历已存在的前缀。缺失的后缀是允许的，因为 lock wrapper 会在这个只读
# gate 之后创建 target；每个已存在的组件都必须是 no-follow directory。
current = pathlib.Path(lexical_target.anchor)
for component in lexical_target.parts[1:]:
    current /= component
    try:
        metadata = os.lstat(current)
    except FileNotFoundError:
        break
    except OSError as error:
        raise SystemExit(
            f"error: 无法安全检查 Cargo target root：{current}: {error}"
        )
    if stat.S_ISLNK(metadata.st_mode):
        raise SystemExit(
            f"error: Cargo target root 包含 symlink component：{current}"
        )
    if not stat.S_ISDIR(metadata.st_mode):
        raise SystemExit(
            f"error: Cargo target root 包含 non-directory component：{current}"
        )

lock_path = normalized_target / ".build.lock"
try:
    lock_metadata = os.lstat(lock_path)
except FileNotFoundError:
    pass
except OSError as error:
    raise SystemExit(
        f"error: 无法安全检查 Cargo build lock：{lock_path}: {error}"
    )
else:
    if not stat.S_ISREG(lock_metadata.st_mode):
        raise SystemExit(
            f"error: Cargo build lock 不是 no-follow regular file：{lock_path}"
        )
    if lock_metadata.st_nlink != 1:
        raise SystemExit(
            f"error: Cargo build lock 必须是 single-linked：{lock_path}"
        )

try:
    common = os.path.commonpath((os.fspath(source_root), os.fspath(normalized_target)))
except ValueError as error:
    raise SystemExit(
        f"error: Cargo target root 无法与 source root 比较：{raw_target}"
    ) from error
if common == os.fspath(source_root):
    raise SystemExit(
        "error: CLI package 拒绝位于 source tree 内的 Cargo target root"
    )
PY
}

if [[ "${KANBAN_CARGO_BUILD_LOCK_HELD:-}" != "1" ]]; then
  TARGET_ROOT_HINT="$("$LOCK" --print-target-dir)"
  validate_target_root_hint "$TARGET_ROOT_HINT"
  exec "$LOCK" -- "$0" "${ORIGINAL_ARGS[@]}"
fi

command -v cargo >/dev/null 2>&1 || { echo "error: 需要 cargo" >&2; exit 1; }
TARGET_ROOT="$("$LOCK" --print-target-dir)"
[[ "$TARGET_ROOT" == /* && -d "$TARGET_ROOT" ]] || {
  echo "error: Cargo target root 必须是已存在的绝对 directory" >&2
  exit 1
}
[[ -f "$SAFE_PATH" && ! -L "$SAFE_PATH" && -x "$SAFE_PATH" ]] || {
  echo "error: release safe-path helper 缺失或不安全：$SAFE_PATH" >&2
  exit 1
}
read -r TARGET_ROOT_DEV TARGET_ROOT_INO < <(
  python3 "$SAFE_PATH" dir-identity --root "$TARGET_ROOT" --path "$TARGET_ROOT"
)
TARGET_ROOT="$(cd "$TARGET_ROOT" && pwd -P)"
python3 "$SAFE_PATH" dir-identity --root "$TARGET_ROOT" --path "$TARGET_ROOT" \
  --expected-dev "$TARGET_ROOT_DEV" --expected-ino "$TARGET_ROOT_INO" >/dev/null
case "$TARGET_ROOT/" in
  "$ROOT/"*)
    echo "error: CLI package 拒绝位于 source tree 内的 Cargo target root" >&2
    exit 1
    ;;
esac
TARGET_DIR="$TARGET_ROOT/release"
BIN_PATH="$TARGET_DIR/$BIN_NAME"
BUNDLE_DIR="$TARGET_DIR/bundle/cli"

validate_target_layout() {
  # 在任何 invalidation 或 Cargo 写入前校验完整的现有 release tree。这样可在
  # 每一层拒绝 symlink/non-regular entry，包括 provenance invalidator 可能删除
  # 或 Cargo 可能覆盖的 package fingerprint/build/dependency 子项。
  if [[ -e "$TARGET_DIR" || -L "$TARGET_DIR" ]]; then
    local entry
    while IFS= read -r -d '' entry; do
      if [[ -L "$entry" ]]; then
        echo "error: Cargo release tree 包含 symlink：$entry" >&2
        return 1
      fi
      if [[ -d "$entry" ]]; then
        python3 "$SAFE_PATH" dir-identity \
          --root "$TARGET_ROOT" --path "$entry" >/dev/null
      elif [[ -f "$entry" ]]; then
        python3 "$SAFE_PATH" validate-file \
          --root "$TARGET_ROOT" --path "$entry"
      else
        echo "error: Cargo release tree 包含 non-regular entry：$entry" >&2
        return 1
      fi
    done < <(find -P "$TARGET_DIR" -print0)
  fi

  # 在创建缺失的 sibling 前检查 invalidation 和 Cargo 将要修改的 directory
  # root。这样列表后面的 file/symlink 不会等到前面的 ensure-dir 修改后才被发现。
  local directory
  for directory in "$TARGET_DIR" \
    "$TARGET_DIR/.fingerprint" "$TARGET_DIR/build" "$TARGET_DIR/deps"; do
    if [[ -e "$directory" || -L "$directory" ]]; then
      python3 "$SAFE_PATH" dir-identity \
        --root "$TARGET_ROOT" --path "$directory" >/dev/null
    fi
  done

  # Cargo 和 provenance invalidator 都需要这些 directory。上面的 no-follow
  # preflight 已覆盖现有 entry；ensure-dir 随后通过同一个 dirfd-anchored
  # primitive 只创建缺失 directory。
  for directory in "$TARGET_DIR" \
    "$TARGET_DIR/.fingerprint" "$TARGET_DIR/build" "$TARGET_DIR/deps"; do
    python3 "$SAFE_PATH" ensure-dir \
      --root "$TARGET_ROOT" --path "$directory" --mode 0755
  done
}

validate_target_layout

TMPDIR_PARENT="${TMPDIR:-/tmp}"
[[ "$TMPDIR_PARENT" == /* && -d "$TMPDIR_PARENT" ]] || {
  echo "error: package temp parent 必须是已存在的绝对 directory" >&2
  exit 1
}
python3 "$SAFE_PATH" dir-identity --root "$TMPDIR_PARENT" \
  --path "$TMPDIR_PARENT" >/dev/null
TMPDIR_PARENT="$(cd "$TMPDIR_PARENT" && pwd -P)"
declare -a TMPDIR_IDENTITY
mapfile -t TMPDIR_IDENTITY < <(
  python3 "$SAFE_PATH" private-dir --root "$TMPDIR_PARENT" \
    --parent "$TMPDIR_PARENT" --prefix ".kanban-cli-package." --print-identity
)
[[ "${#TMPDIR_IDENTITY[@]}" -eq 3 ]] || {
  echo "error: private CLI package temp 返回了无效 identity token" >&2
  exit 1
}
TMPDIR="${TMPDIR_IDENTITY[0]}"
TMPDIR_IDENTITY_DEV="${TMPDIR_IDENTITY[1]}"
TMPDIR_IDENTITY_INO="${TMPDIR_IDENTITY[2]}"
OUTPUT_STAGE=""
OUTPUT_STAGE_IDENTITY_DEV=""
OUTPUT_STAGE_IDENTITY_INO=""
cleanup() {
  if [[ -n "$OUTPUT_STAGE" && "$(dirname "$OUTPUT_STAGE")" == "$BUNDLE_DIR/deb" &&
    "$(basename "$OUTPUT_STAGE")" == .kanban-cli-deb.* ]]; then
    python3 "$SAFE_PATH" remove-tree --root "$TARGET_ROOT" \
      --path "$OUTPUT_STAGE" --expected-dev "$OUTPUT_STAGE_IDENTITY_DEV" \
      --expected-ino "$OUTPUT_STAGE_IDENTITY_INO" ||
      printf 'warning: 保留未验证的 CLI output stage：%s\n' "$OUTPUT_STAGE" >&2
  fi
  python3 "$SAFE_PATH" remove-tree --root "$TMPDIR_PARENT" \
    --path "$TMPDIR" --expected-dev "$TMPDIR_IDENTITY_DEV" \
    --expected-ino "$TMPDIR_IDENTITY_INO" ||
    printf 'warning: 保留未验证的 CLI package temp tree：%s\n' "$TMPDIR" >&2
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

VERSION="$(cargo pkgid --locked -p kanban-cli | sed 's/.*#//')"
TARGET_TRIPLE="$(rustc -vV | awk '/^host:/ { print $2 }')"
RUST_ARCH="${TARGET_TRIPLE%%-*}"

deb_arch() {
  case "$RUST_ARCH" in
    x86_64) echo "amd64" ;;
    aarch64) echo "arm64" ;;
    armv7*|arm) echo "armhf" ;;
    i686|i586) echo "i386" ;;
    *)
      echo "error: target triple 的 Debian architecture 不受支持：$TARGET_TRIPLE" >&2
      exit 1
      ;;
  esac
}

install_payload() {
  local root="$1"
  install -Dm755 "$BIN_PATH" "$root/usr/bin/$BIN_NAME"
  install -Dm644 "$ROOT/README.md" "$root/usr/share/doc/$PACKAGE_NAME/README.md"
}

build_binary() {
  local workspace_packages
  mapfile -t workspace_packages < <(
    cd "$ROOT"
    "cargo" metadata --locked --no-deps --format-version 1 |
      python3 -c 'import json,sys; print("\n".join(p["name"] for p in json.load(sys.stdin)["packages"]))'
  )
  "$PROVENANCE" --invalidate-packages "$TARGET_DIR" "${workspace_packages[@]}"
  rm -f "$BIN_PATH" "$TARGET_DIR/$BIN_NAME.d"
  (
    cd "$ROOT"
    "$LOCK" -- "cargo" build --locked -p kanban-cli --release "${BUILD_ARGS[@]}"
  )
  [[ -x "$BIN_PATH" ]] || { echo "error: 未找到预期 binary：$BIN_PATH" >&2; exit 1; }

  "$PROVENANCE" --verify-dep-info "$ROOT" "$TARGET_DIR/$BIN_NAME.d"
}

deb_depends() {
  local package_root="$1"
  local dep_workspace output depends

  if ! command -v dpkg-shlibdeps >/dev/null 2>&1; then
    echo "warning: 未找到 dpkg-shlibdeps；使用保守的 Debian runtime dependencies" >&2
    echo "libc6, libgcc-s1"
    return
  fi

  dep_workspace="$TMPDIR/shlibdeps"
  mkdir -p "$dep_workspace/debian"
  cat > "$dep_workspace/debian/control" <<EOF
Source: $PACKAGE_NAME
Package: $PACKAGE_NAME
Architecture: any
EOF

  local shlib_inputs=("$package_root/usr/bin/$BIN_NAME")

  output="$(
    cd "$dep_workspace"
    dpkg-shlibdeps -O "-S$package_root" "${shlib_inputs[@]}"
  )" || {
    echo "error: dpkg-shlibdeps 生成 shared-library dependencies 失败" >&2
    exit 1
  }

  depends="$(printf '%s\n' "$output" | sed -n 's/^shlibs:Depends=//p')"
  [[ -n "$depends" ]] || {
    echo "error: dpkg-shlibdeps 未返回 shlibs:Depends 值" >&2
    exit 1
  }
  echo "$depends"
}

build_deb() {
  command -v dpkg-deb >/dev/null 2>&1 || { echo "error: --format deb 需要 dpkg-deb" >&2; exit 1; }

  local arch package_root control_dir out_dir out_file staged_dir staged_file
  local installed_size depends
  arch="$(deb_arch)"
  package_root="$TMPDIR/deb-root"
  control_dir="$package_root/DEBIAN"
  out_dir="$BUNDLE_DIR/deb"
  out_file="$out_dir/${PACKAGE_NAME}_${VERSION}-${REVISION}_${arch}.deb"

  install -d -m 0755 "$control_dir"
  python3 "$SAFE_PATH" ensure-dir --root "$TARGET_ROOT" --path "$out_dir" --mode 0755
  install_payload "$package_root"
  installed_size="$(du -sk "$package_root/usr" | awk '{ print $1 }')"
  depends="$(deb_depends "$package_root")"

  cat > "$control_dir/control" <<EOF
Package: $PACKAGE_NAME
Version: $VERSION-$REVISION
Section: utils
Priority: optional
Architecture: $arch
Depends: $depends
Maintainer: SockingPanda <42059910+SockingPanda@users.noreply.github.com>
Installed-Size: $installed_size
EOF
  cat >> "$control_dir/control" <<EOF
Description: Local-first Kanban CLI
 Standalone kanban command line client for the Kanban Tool local work queue.
EOF

  local -a stage_identity
  mapfile -t stage_identity < <(
    python3 "$SAFE_PATH" private-dir --root "$TARGET_ROOT" \
      --parent "$out_dir" --prefix ".kanban-cli-deb." --print-identity
  )
  [[ "${#stage_identity[@]}" -eq 3 ]] || {
    echo "error: private CLI output stage 返回了无效 identity token" >&2
    exit 1
  }
  staged_dir="${stage_identity[0]}"
  OUTPUT_STAGE_IDENTITY_DEV="${stage_identity[1]}"
  OUTPUT_STAGE_IDENTITY_INO="${stage_identity[2]}"
  OUTPUT_STAGE="$staged_dir"
  staged_file="$staged_dir/$(basename "$out_file")"
  dpkg-deb --root-owner-group --build "$package_root" "$staged_file"
  python3 "$SAFE_PATH" validate-file --root "$TARGET_ROOT" --path "$staged_file"

  # 在发布前立即重新校验 creation token；不要把 replacement directory
  # 重新采样为新的 baseline。
  python3 "$SAFE_PATH" dir-identity --root "$TARGET_ROOT" --path "$OUTPUT_STAGE" \
    --expected-dev "$OUTPUT_STAGE_IDENTITY_DEV" \
    --expected-ino "$OUTPUT_STAGE_IDENTITY_INO" >/dev/null
  if [[ -e "$out_file" || -L "$out_file" ]]; then
    python3 "$SAFE_PATH" publish-file --root "$TARGET_ROOT" \
      --source "$staged_file" --destination "$out_file" --replace \
      --expected-source-parent-dev "$OUTPUT_STAGE_IDENTITY_DEV" \
      --expected-source-parent-ino "$OUTPUT_STAGE_IDENTITY_INO"
  else
    python3 "$SAFE_PATH" publish-file --root "$TARGET_ROOT" \
      --source "$staged_file" --destination "$out_file" \
      --expected-source-parent-dev "$OUTPUT_STAGE_IDENTITY_DEV" \
      --expected-source-parent-ino "$OUTPUT_STAGE_IDENTITY_INO"
  fi
  python3 "$SAFE_PATH" remove-tree --root "$TARGET_ROOT" \
    --path "$staged_dir" --expected-dev "$OUTPUT_STAGE_IDENTITY_DEV" \
    --expected-ino "$OUTPUT_STAGE_IDENTITY_INO"
  OUTPUT_STAGE=""
  echo "$out_file"
}

build_binary
build_deb
