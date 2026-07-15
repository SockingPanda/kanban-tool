#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGE_NAME="kanban-tool-cli"
BIN_NAME="kanban"
HELPER_BINARIES=("kanban-vector-lancedb" "kanban-graph-oxigraph")
HELPER_INSTALL_DIR="usr/lib/kanban"
REVISION="1"
BUILD_ARGS=()
LOCK="$ROOT/scripts/cargo-build-lock.sh"
PROVENANCE="$ROOT/scripts/package-source-provenance.sh"
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

if [[ "${KANBAN_PACKAGE_BUILD_LOCK_HELD:-}" != "1" ]]; then
  exec env KANBAN_PACKAGE_BUILD_LOCK_HELD=1 "$LOCK" -- "$0" "${ORIGINAL_ARGS[@]}"
fi

command -v cargo >/dev/null 2>&1 || { echo "error: cargo is required" >&2; exit 1; }

VERSION="$(cargo pkgid -p kanban-cli | sed 's/.*#//')"
TARGET_TRIPLE="$(rustc -vV | awk '/^host:/ { print $2 }')"
RUST_ARCH="${TARGET_TRIPLE%%-*}"
TARGET_DIR="$("$LOCK" --print-target-dir)/release"
BIN_PATH="$TARGET_DIR/$BIN_NAME"
BUNDLE_DIR="$TARGET_DIR/bundle/cli"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

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

  local arch package_root control_dir out_dir out_file installed_size depends
  arch="$(deb_arch)"
  package_root="$TMPDIR/deb-root"
  control_dir="$package_root/DEBIAN"
  out_dir="$BUNDLE_DIR/deb"
  out_file="$out_dir/${PACKAGE_NAME}_${VERSION}-${REVISION}_${arch}.deb"

  rm -rf "$package_root"
  install -d -m 0755 "$control_dir"
  mkdir -p "$out_dir"
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
Maintainer: kanban-user
Installed-Size: $installed_size
Description: Local-first Kanban CLI
 Standalone kanban command line client for the Kanban Tool local work queue.
EOF

  dpkg-deb --root-owner-group --build "$package_root" "$out_file"
  echo "$out_file"
}

build_binary
build_deb
