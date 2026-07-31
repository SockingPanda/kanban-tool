#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
PACKAGE_NAME="kanban-tool-cli"
BIN_NAME="kanban"
HELPER_BINARIES=("kanban-vector-lancedb" "kanban-graph-oxigraph")
HELPER_INSTALL_DIR="usr/lib/kanban"
REVISION="1"
BUILD_ARGS=()
LOCK="$ROOT/scripts/cargo-build-lock.sh"
PROVENANCE="$ROOT/scripts/package-source-provenance.sh"
SAFE_PATH="$ROOT/scripts/release-safe-path.py"
ORIGINAL_ARGS=("$@")

usage() {
  cat <<'EOF'
Usage: scripts/package-cli-linux.sh [OPTIONS]

Build the standalone Linux CLI package for kanban.

Options:
  --format <deb>               Package format to build. Default: deb.
  --features <features>        Pass a comma-separated feature list to cargo.
  --all-features               Pass --all-features to cargo.
  --no-default-features        Pass --no-default-features to cargo.
  -h, --help                   Show this help.

Outputs:
  Under the target directory printed by scripts/cargo-build-lock.sh --print-target-dir:
  release/bundle/cli/deb/*.deb
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --format)
      [[ $# -ge 2 ]] || { echo "error: --format requires a value" >&2; exit 2; }
      [[ "$2" == "deb" ]] || { echo "error: only --format deb is supported" >&2; exit 2; }
      shift 2
      ;;
    --features)
      [[ $# -ge 2 ]] || { echo "error: --features requires a value" >&2; exit 2; }
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
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

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
        f"error: Cargo target root must be absolute before lock handoff: {raw_target}"
    )
if lexical_target.anchor != os.path.sep or raw_target.startswith("//"):
    raise SystemExit(
        "error: Cargo target root must use exactly one leading slash: "
        f"{raw_target}"
    )
if ".." in lexical_target.parts:
    raise SystemExit(
        f"error: Cargo target root must not contain parent traversal: {raw_target}"
    )

normalized_target = pathlib.Path(os.path.normpath(raw_target))
if normalized_target == pathlib.Path(normalized_target.anchor):
    raise SystemExit("error: Cargo target root must not be a filesystem root")

# Walk only the existing prefix.  Missing suffixes are valid because the lock
# wrapper creates the target after this read-only gate; every existing
# component must already be a no-follow directory.
current = pathlib.Path(lexical_target.anchor)
for component in lexical_target.parts[1:]:
    current /= component
    try:
        metadata = os.lstat(current)
    except FileNotFoundError:
        break
    except OSError as error:
        raise SystemExit(
            f"error: cannot inspect Cargo target root safely: {current}: {error}"
        )
    if stat.S_ISLNK(metadata.st_mode):
        raise SystemExit(
            f"error: Cargo target root contains a symlink component: {current}"
        )
    if not stat.S_ISDIR(metadata.st_mode):
        raise SystemExit(
            f"error: Cargo target root contains a non-directory component: {current}"
        )

lock_path = normalized_target / ".build.lock"
try:
    lock_metadata = os.lstat(lock_path)
except FileNotFoundError:
    pass
except OSError as error:
    raise SystemExit(
        f"error: cannot inspect Cargo build lock safely: {lock_path}: {error}"
    )
else:
    if not stat.S_ISREG(lock_metadata.st_mode):
        raise SystemExit(
            f"error: Cargo build lock is not a no-follow regular file: {lock_path}"
        )
    if lock_metadata.st_nlink != 1:
        raise SystemExit(
            f"error: Cargo build lock must be single-linked: {lock_path}"
        )

try:
    common = os.path.commonpath((os.fspath(source_root), os.fspath(normalized_target)))
except ValueError as error:
    raise SystemExit(
        f"error: Cargo target root is not comparable with the source root: {raw_target}"
    ) from error
if common == os.fspath(source_root):
    raise SystemExit(
        "error: CLI package refuses a Cargo target root inside the source tree"
    )
PY
}

if [[ "${KANBAN_CARGO_BUILD_LOCK_HELD:-}" != "1" ]]; then
  TARGET_ROOT_HINT="$("$LOCK" --print-target-dir)"
  validate_target_root_hint "$TARGET_ROOT_HINT"
  exec "$LOCK" -- "$0" "${ORIGINAL_ARGS[@]}"
fi

command -v cargo >/dev/null 2>&1 || { echo "error: cargo is required" >&2; exit 1; }

VERSION="$(cargo pkgid --locked -p kanban-cli | sed 's/.*#//')"
TARGET_TRIPLE="$(rustc -vV | awk '/^host:/ { print $2 }')"
RUST_ARCH="${TARGET_TRIPLE%%-*}"
TARGET_ROOT="$("$LOCK" --print-target-dir)"
[[ "$TARGET_ROOT" == /* && -d "$TARGET_ROOT" ]] || {
  echo "error: Cargo target root must be an existing absolute directory" >&2
  exit 1
}
[[ -f "$SAFE_PATH" && ! -L "$SAFE_PATH" && -x "$SAFE_PATH" ]] || {
  echo "error: release safe-path helper is missing or unsafe: $SAFE_PATH" >&2
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
    echo "error: CLI package refuses a Cargo target root inside the source tree" >&2
    exit 1
    ;;
esac
TARGET_DIR="$TARGET_ROOT/release"
BIN_PATH="$TARGET_DIR/$BIN_NAME"
BUNDLE_DIR="$TARGET_DIR/bundle/cli"

validate_target_layout() {
  # Validate the complete existing release tree before any invalidation or
  # Cargo write.  This rejects symlink/non-regular entries at every depth,
  # including package fingerprint/build/dependency children that the
  # provenance invalidator may otherwise remove or Cargo may overwrite.
  if [[ -e "$TARGET_DIR" || -L "$TARGET_DIR" ]]; then
    local entry
    while IFS= read -r -d '' entry; do
      if [[ -L "$entry" ]]; then
        echo "error: Cargo release tree contains a symlink: $entry" >&2
        return 1
      fi
      if [[ -d "$entry" ]]; then
        python3 "$SAFE_PATH" dir-identity \
          --root "$TARGET_ROOT" --path "$entry" >/dev/null
      elif [[ -f "$entry" ]]; then
        python3 "$SAFE_PATH" validate-file \
          --root "$TARGET_ROOT" --path "$entry"
      else
        echo "error: Cargo release tree contains a non-regular entry: $entry" >&2
        return 1
      fi
    done < <(find -P "$TARGET_DIR" -print0)
  fi

  # Check the directory roots that invalidation and Cargo will mutate before
  # creating any missing sibling.  This prevents a later file/symlink in the
  # list from being discovered only after an earlier ensure-dir mutation.
  local directory
  for directory in "$TARGET_DIR" \
    "$TARGET_DIR/.fingerprint" "$TARGET_DIR/build" "$TARGET_DIR/deps"; do
    if [[ -e "$directory" || -L "$directory" ]]; then
      python3 "$SAFE_PATH" dir-identity \
        --root "$TARGET_ROOT" --path "$directory" >/dev/null
    fi
  done

  # Cargo and the provenance invalidator both require these directories.  The
  # no-follow preflight above covers existing entries; ensure-dir then creates
  # only missing directories through the same dirfd-anchored primitive.
  for directory in "$TARGET_DIR" \
    "$TARGET_DIR/.fingerprint" "$TARGET_DIR/build" "$TARGET_DIR/deps"; do
    python3 "$SAFE_PATH" ensure-dir \
      --root "$TARGET_ROOT" --path "$directory" --mode 0755
  done
}

validate_target_layout

TMPDIR_PARENT="${TMPDIR:-/tmp}"
[[ "$TMPDIR_PARENT" == /* && -d "$TMPDIR_PARENT" ]] || {
  echo "error: package temp parent must be an existing absolute directory" >&2
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
  echo "error: private CLI package temp returned an invalid identity token" >&2
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
      printf 'warning: retained unverified CLI output stage: %s\n' "$OUTPUT_STAGE" >&2
  fi
  python3 "$SAFE_PATH" remove-tree --root "$TMPDIR_PARENT" \
    --path "$TMPDIR" --expected-dev "$TMPDIR_IDENTITY_DEV" \
    --expected-ino "$TMPDIR_IDENTITY_INO" ||
    printf 'warning: retained unverified CLI package temp tree: %s\n' "$TMPDIR" >&2
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

deb_arch() {
  case "$RUST_ARCH" in
    x86_64) echo "amd64" ;;
    aarch64) echo "arm64" ;;
    armv7*|arm) echo "armhf" ;;
    i686|i586) echo "i386" ;;
    *)
      echo "error: unsupported Debian architecture for target triple: $TARGET_TRIPLE" >&2
      exit 1
      ;;
  esac
}

install_payload() {
  local root="$1"
  install -Dm755 "$BIN_PATH" "$root/usr/bin/$BIN_NAME"
  local helper
  for helper in "${HELPER_BINARIES[@]}"; do
    install -Dm755 "$TARGET_DIR/$helper" "$root/$HELPER_INSTALL_DIR/$helper"
  done
  install -Dm644 "$ROOT/README.md" "$root/usr/share/doc/$PACKAGE_NAME/README.md"
}

build_binary() {
  local workspace_packages
  mapfile -t workspace_packages < <(
    cd "$ROOT"
    cargo metadata --locked --no-deps --format-version 1 |
      python3 -c 'import json,sys; print("\n".join(p["name"] for p in json.load(sys.stdin)["packages"]))'
  )
  "$PROVENANCE" --invalidate-packages "$TARGET_DIR" "${workspace_packages[@]}"
  rm -f "$BIN_PATH" "$TARGET_DIR/$BIN_NAME.d"
  local helper
  for helper in "${HELPER_BINARIES[@]}"; do
    rm -f "$TARGET_DIR/$helper" "$TARGET_DIR/$helper.d"
  done
  (
    cd "$ROOT"
    "$LOCK" -- cargo build --locked -p kanban-cli --release "${BUILD_ARGS[@]}"
    "$LOCK" -- cargo build --locked -p kanban-vector-lancedb -p kanban-graph-oxigraph --release --bins
  )
  [[ -x "$BIN_PATH" ]] || { echo "error: expected binary not found: $BIN_PATH" >&2; exit 1; }
  for helper in "${HELPER_BINARIES[@]}"; do
    [[ -x "$TARGET_DIR/$helper" ]] || { echo "error: expected helper binary not found: $TARGET_DIR/$helper" >&2; exit 1; }
  done
  "$PROVENANCE" --verify-dep-info "$ROOT" "$TARGET_DIR/$BIN_NAME.d"
  for helper in "${HELPER_BINARIES[@]}"; do
    "$PROVENANCE" --verify-dep-info "$ROOT" "$TARGET_DIR/$helper.d"
  done
}

deb_depends() {
  local package_root="$1"
  local dep_workspace output depends

  if ! command -v dpkg-shlibdeps >/dev/null 2>&1; then
    echo "warning: dpkg-shlibdeps not found; using conservative Debian runtime dependencies" >&2
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
  local helper
  for helper in "${HELPER_BINARIES[@]}"; do
    shlib_inputs+=("$package_root/$HELPER_INSTALL_DIR/$helper")
  done

  output="$(
    cd "$dep_workspace"
    dpkg-shlibdeps -O "-S$package_root" "${shlib_inputs[@]}"
  )" || {
    echo "error: dpkg-shlibdeps failed to generate shared-library dependencies" >&2
    exit 1
  }

  depends="$(printf '%s\n' "$output" | sed -n 's/^shlibs:Depends=//p')"
  [[ -n "$depends" ]] || {
    echo "error: dpkg-shlibdeps returned no shlibs:Depends value" >&2
    exit 1
  }
  echo "$depends"
}

build_deb() {
  command -v dpkg-deb >/dev/null 2>&1 || { echo "error: dpkg-deb is required for --format deb" >&2; exit 1; }

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
Description: Local-first Kanban CLI
 Standalone kanban command line client for the Kanban Tool local work queue.
EOF

  local -a stage_identity
  mapfile -t stage_identity < <(
    python3 "$SAFE_PATH" private-dir --root "$TARGET_ROOT" \
      --parent "$out_dir" --prefix ".kanban-cli-deb." --print-identity
  )
  [[ "${#stage_identity[@]}" -eq 3 ]] || {
    echo "error: private CLI output stage returned an invalid identity token" >&2
    exit 1
  }
  staged_dir="${stage_identity[0]}"
  OUTPUT_STAGE_IDENTITY_DEV="${stage_identity[1]}"
  OUTPUT_STAGE_IDENTITY_INO="${stage_identity[2]}"
  OUTPUT_STAGE="$staged_dir"
  staged_file="$staged_dir/$(basename "$out_file")"
  dpkg-deb --root-owner-group --build "$package_root" "$staged_file"
  chmod 0644 "$staged_file"
  python3 "$SAFE_PATH" validate-file --root "$TARGET_ROOT" --path "$staged_file"
  # Revalidate the creation token immediately before publication; do not
  # resample a replacement directory as a new baseline.
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
