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
SOURCE_GATE="$ROOT/scripts/release-source-gate.sh"
ORIGINAL_ARGS=("$@")
RELEASE_SOURCE_MANIFEST="${KANBAN_RELEASE_SOURCE_MANIFEST:-}"
RELEASE_SOURCE_MAP="${KANBAN_RELEASE_SOURCE_MAP:-}"
RELEASE_BUILD_ID="${KANBAN_BUILD_ID:-}"
RELEASE_PROVENANCE_ENABLED=0
RELEASE_SOURCE_MANIFEST_STABLE=""
RELEASE_SOURCE_MAP_STABLE=""
RELEASE_VERSION=""
RELEASE_TARGET_TRIPLE=""
RELEASE_FEATURES=""
RELEASE_NO_DEFAULT_FEATURES=""
RELEASE_RUSTC_VV_SHA256=""
RELEASE_CARGO_VERSION_SHA256=""
RELEASE_RUSTC_PATH=""
RELEASE_CARGO_PATH=""
RELEASE_BINARY_STAGE_DIR=""

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
    echo "error: release package requires an inherited Cargo build lock proof" >&2
    exit 1
  fi
}

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
          echo "error: release package refuses build-affecting environment override: $name" >&2
          exit 1
        fi
        ;;
      CARGO_TARGET_*)
        if [[ "$name" != "CARGO_TARGET_DIR" ]] ||
          ! lock_environment_is_internal; then
          if [[ "${!name+x}" == "x" ]]; then
            echo "error: release package refuses build-affecting environment override: $name" >&2
            exit 1
          fi
        fi
        ;;
      CARGO_BUILD_JOBS|NEXTEST_TEST_THREADS|RUST_TEST_THREADS)
        if ! lock_environment_is_internal; then
          if [[ "${!name+x}" == "x" ]]; then
            echo "error: release package refuses build-affecting environment override: $name" >&2
            exit 1
          fi
        fi
        ;;
      CARGO_BUILD_TARGET)
        # release-cohort exports the canonical target while nested recipes
        # run; prepare_release_provenance checks it against the manifest.
        ;;
      CARGO_BUILD_*|CARGO_HTTP_*|CARGO_NET_*|CARGO_PROFILE_*|CARGO_REGISTRIES_*|\
      CARGO_SOURCE_*|RUSTUP_*|CC_*|CXX_*|PKG_CONFIG_*)
        if [[ "${!name+x}" == "x" ]]; then
          echo "error: release package refuses build-affecting environment override: $name" >&2
          exit 1
        fi
        ;;
    esac
  done
}

prepare_release_provenance() {
  local metadata_line
  local -a metadata
  [[ "$RELEASE_SOURCE_MANIFEST" == /* && "$RELEASE_SOURCE_MAP" == /* ]] || {
    echo "error: release provenance paths must be absolute" >&2
    exit 1
  }
  [[ "$RELEASE_SOURCE_MANIFEST" != *"/../"* && "$RELEASE_SOURCE_MAP" != *"/../"* &&
    "$RELEASE_SOURCE_MANIFEST" != */.. && "$RELEASE_SOURCE_MAP" != */.. ]] || {
    echo "error: release provenance paths must not contain parent traversal" >&2
    exit 1
  }
  [[ -f "$RELEASE_SOURCE_MANIFEST" && ! -L "$RELEASE_SOURCE_MANIFEST" ]] || {
    echo "error: KANBAN_RELEASE_SOURCE_MANIFEST is missing or unsafe" >&2
    exit 1
  }
  [[ -f "$RELEASE_SOURCE_MAP" && ! -L "$RELEASE_SOURCE_MAP" ]] || {
    echo "error: KANBAN_RELEASE_SOURCE_MAP is missing or unsafe" >&2
    exit 1
  }
  metadata_line="$(python3 - "$RELEASE_SOURCE_MANIFEST" "$RELEASE_SOURCE_MAP" \
    "$RELEASE_BUILD_ID" "$ROOT" "$TARGET_ROOT" "$TMPDIR" <<'PY'
import hashlib
import json
import os
import pathlib
import stat
import sys

manifest_path = pathlib.Path(sys.argv[1])
source_map_raw = sys.argv[2]
build_id = sys.argv[3]
root = pathlib.Path(sys.argv[4])
target_root = pathlib.Path(sys.argv[5])
stage = pathlib.Path(sys.argv[6])
source_map_rel = pathlib.PurePosixPath("docs/release/derived-projection-v2-source-map.json")

def safe_read(path: pathlib.Path, label: str) -> bytes:
    if not path.is_absolute() or ".." in path.parts:
        raise SystemExit(f"error: {label} path is not absolute/traversal-free: {path}")
    current = pathlib.Path(path.anchor)
    for component in path.parts[1:]:
        current /= component
        try:
            metadata = os.lstat(current)
        except OSError as error:
            raise SystemExit(f"error: cannot inspect {label} path: {path}: {error}")
        if stat.S_ISLNK(metadata.st_mode):
            raise SystemExit(f"error: {label} path contains a symlink: {current}")
        if current == path and (
            not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1
        ):
            raise SystemExit(f"error: {label} path is not a single-link regular file: {path}")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise SystemExit(f"error: cannot open {label} safely: {path}: {error}")
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
            raise SystemExit(f"error: {label} is not a single-link regular file: {path}")
        chunks = []
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            chunks.append(chunk)
        after = os.fstat(descriptor)
        identity = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns", "st_nlink")
        if any(getattr(before, field) != getattr(after, field) for field in identity):
            raise SystemExit(f"error: {label} changed while being sampled: {path}")
        return b"".join(chunks)
    finally:
        os.close(descriptor)

def write_stable(path: pathlib.Path, payload: bytes, label: str) -> None:
    flags = (
        os.O_WRONLY | os.O_CREAT | os.O_EXCL |
        getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(path, flags, 0o600)
    except OSError as error:
        raise SystemExit(f"error: cannot create stable {label}: {path}: {error}")
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            view = view[written:]
        os.fchmod(descriptor, 0o444)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)

expected_map = root / source_map_rel
if source_map_raw != str(expected_map):
    raise SystemExit("error: release source map is not the canonical source-tree path")
source_map_path = pathlib.Path(source_map_raw)
manifest_bytes = safe_read(manifest_path, "source provenance manifest")
source_map_bytes = safe_read(source_map_path, "release source map")
try:
    manifest = json.loads(manifest_bytes.decode("utf-8"))
except (UnicodeDecodeError, json.JSONDecodeError) as error:
    raise SystemExit(f"error: release source provenance is not valid JSON: {error}")
if manifest.get("build_id") != build_id:
    raise SystemExit("error: KANBAN_BUILD_ID does not match source provenance")
source = manifest.get("source_map")
if not isinstance(source, dict) or source.get("path") != source_map_rel.as_posix():
    raise SystemExit("error: source provenance does not name the canonical source map")
source_map_hash = hashlib.sha256(source_map_bytes).hexdigest()
if source.get("sha256") != source_map_hash:
    raise SystemExit("error: release source map hash does not match source provenance")
identity = manifest.get("identity")
if not isinstance(identity, dict):
    raise SystemExit("error: source provenance lacks release identity")
features = identity.get("features")
target = identity.get("target")
toolchain = identity.get("toolchain")
if features != {"effective": [], "no_default_features": False}:
    raise SystemExit("error: source provenance does not carry the canonical feature contract")
if not isinstance(target, dict) or not isinstance(target.get("triple"), str):
    raise SystemExit("error: source provenance lacks a target triple")
if not isinstance(toolchain, dict) or not isinstance(toolchain.get("rustc_vv_sha256"), str) or not isinstance(toolchain.get("cargo_version_sha256"), str):
    raise SystemExit("error: source provenance lacks toolchain output hashes")
write_stable(stage / "source-provenance.json", manifest_bytes, "source provenance")
write_stable(stage / "derived-projection-v2-source-map.json", source_map_bytes, "source map")
print("\t".join((
    str(manifest.get("version", "")),
    target["triple"],
    "",
    "0",
    toolchain["rustc_vv_sha256"],
    toolchain["cargo_version_sha256"],
)))
PY
  )"
  mapfile -t metadata < <(printf '%s\n' "$metadata_line")
  [[ "${#metadata[@]}" -eq 1 ]] || {
    echo "error: release provenance metadata probe returned an unexpected result" >&2
    exit 1
  }
  IFS=$'\t' read -r RELEASE_VERSION RELEASE_TARGET_TRIPLE RELEASE_FEATURES \
    RELEASE_NO_DEFAULT_FEATURES RELEASE_RUSTC_VV_SHA256 RELEASE_CARGO_VERSION_SHA256 \
    <<<"${metadata[0]}"
  [[ -n "$RELEASE_VERSION" && -n "$RELEASE_TARGET_TRIPLE" &&
    -z "$RELEASE_FEATURES" && "$RELEASE_NO_DEFAULT_FEATURES" == "0" ]] || {
    echo "error: release provenance metadata is incomplete" >&2
    exit 1
  }
  "$SOURCE_GATE" validate --manifest "$TMPDIR/source-provenance.json"
  RELEASE_SOURCE_MANIFEST_STABLE="$TMPDIR/source-provenance.json"
  RELEASE_SOURCE_MAP_STABLE="$TMPDIR/derived-projection-v2-source-map.json"
  if [[ "${CARGO_BUILD_TARGET+x}" == "x" && "$CARGO_BUILD_TARGET" != "$RELEASE_TARGET_TRIPLE" ]]; then
    echo "error: CARGO_BUILD_TARGET does not match release provenance target" >&2
    exit 1
  fi
  export CARGO_BUILD_TARGET="$RELEASE_TARGET_TRIPLE"
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

require_inherited_lock_if_marked

if [[ -n "$RELEASE_SOURCE_MANIFEST" || -n "$RELEASE_SOURCE_MAP" || -n "$RELEASE_BUILD_ID" ]]; then
  RELEASE_PROVENANCE_ENABLED=1
  [[ -n "$RELEASE_BUILD_ID" && -n "$RELEASE_SOURCE_MANIFEST" &&
    -n "$RELEASE_SOURCE_MAP" ]] || {
    echo "error: release provenance requires build id, source manifest, and source map" >&2
    exit 1
  }
  reject_release_build_environment
fi

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

if [[ "$RELEASE_PROVENANCE_ENABLED" == "1" ]]; then
  prepare_release_provenance
  RELEASE_RUSTC_PATH="$(command -v rustc 2>/dev/null || true)"
  RELEASE_CARGO_PATH="$(command -v cargo 2>/dev/null || true)"
  [[ -x "$RELEASE_RUSTC_PATH" && -x "$RELEASE_CARGO_PATH" ]] || {
    echo "error: release package requires executable rustc and cargo" >&2
    exit 1
  }
  if [[ "${RUSTC+x}" == "x" && "$RUSTC" != "$RELEASE_RUSTC_PATH" ]]; then
    echo "error: RUSTC does not resolve to the release identity executable" >&2
    exit 1
  fi
  if [[ "${CARGO+x}" == "x" && "$CARGO" != "$RELEASE_CARGO_PATH" ]]; then
    echo "error: CARGO does not resolve to the release identity executable" >&2
    exit 1
  fi
  actual_rustc_vv="$($RELEASE_RUSTC_PATH -vV)" || {
    echo "error: recorded rustc -vV failed during release package" >&2
    exit 1
  }
  actual_cargo_version="$($RELEASE_CARGO_PATH --version)" || {
    echo "error: recorded cargo --version failed during release package" >&2
    exit 1
  }
  [[ "$(printf '%s' "$actual_rustc_vv" | sha256sum | awk '{print $1}')" == \
    "$RELEASE_RUSTC_VV_SHA256" ]] || {
    echo "error: rustc output does not match source provenance" >&2
    exit 1
  }
  [[ "$(printf '%s' "$actual_cargo_version" | sha256sum | awk '{print $1}')" == \
    "$RELEASE_CARGO_VERSION_SHA256" ]] || {
    echo "error: cargo output does not match source provenance" >&2
    exit 1
  }
  TARGET_TRIPLE="$(awk '/^host:[[:space:]]+/ { print $2; count += 1 } END { if (count != 1) exit 1 }' <<<"$actual_rustc_vv")" || {
    echo "error: rustc output has no unique host target" >&2
    exit 1
  }
  [[ "$TARGET_TRIPLE" == "$RELEASE_TARGET_TRIPLE" ]] || {
    echo "error: rustc host target does not match source provenance" >&2
    exit 1
  }
  VERSION="$($RELEASE_CARGO_PATH pkgid --locked -p kanban-cli | sed 's/.*#//')"
  [[ "$VERSION" == "$RELEASE_VERSION" ]] || {
    echo "error: Cargo package version does not match source provenance" >&2
    exit 1
  }
else
  VERSION="$(cargo pkgid --locked -p kanban-cli | sed 's/.*#//')"
  TARGET_TRIPLE="$(rustc -vV | awk '/^host:/ { print $2 }')"
fi
RUST_ARCH="${TARGET_TRIPLE%%-*}"

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
  local cli_source="$BIN_PATH"
  if [[ "$RELEASE_PROVENANCE_ENABLED" == "1" ]]; then
    cli_source="$RELEASE_BINARY_STAGE_DIR/$BIN_NAME"
  fi
  install -Dm755 "$cli_source" "$root/usr/bin/$BIN_NAME"
  install -Dm644 "$ROOT/README.md" "$root/usr/share/doc/$PACKAGE_NAME/README.md"
  if [[ "$RELEASE_PROVENANCE_ENABLED" == "1" ]]; then
    install -Dm644 "$RELEASE_SOURCE_MANIFEST_STABLE" \
      "$root/usr/share/doc/$PACKAGE_NAME/source-provenance.json"
    install -Dm644 "$RELEASE_SOURCE_MAP_STABLE" \
      "$root/usr/share/doc/$PACKAGE_NAME/derived-projection-v2-source-map.json"
  fi
}

rewrite_and_verify_control() {
  local control="$1"
  python3 - "$control" "$RELEASE_BUILD_ID" <<'PY'
import os
import pathlib
import re
import stat
import sys

path = pathlib.Path(sys.argv[1])
build_id = sys.argv[2]
if not build_id or any(char in build_id for char in "\r\n\x00"):
    raise SystemExit("error: build identity cannot be represented in Debian control")

def parse(payload: bytes):
    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError as error:
        raise SystemExit(f"error: Debian control is not valid UTF-8: {error}")
    if "\x00" in text:
        raise SystemExit("error: Debian control contains NUL")
    lines = text.splitlines()
    fields = []
    current = None
    for index, line in enumerate(lines):
        if line.startswith((" ", "\t")):
            if current is None:
                raise SystemExit(f"error: orphan Debian control continuation at line {index + 1}")
            current["lines"].append((index, line))
            continue
        if not line:
            current = None
            continue
        match = re.fullmatch(r"([A-Za-z0-9][A-Za-z0-9-]*):(.*)", line)
        if match is None:
            raise SystemExit(f"error: malformed Debian control field at line {index + 1}")
        current = {
            "key": match.group(1),
            "value": match.group(2),
            "index": index,
            "lines": [(index, line)],
        }
        fields.append(current)
    return lines, fields

def read_regular() -> bytes:
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
    )
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
            raise SystemExit("error: Debian control is not a single-link regular file")
        chunks = []
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            chunks.append(chunk)
        after = os.fstat(descriptor)
        if (metadata.st_dev, metadata.st_ino, metadata.st_size, metadata.st_mtime_ns,
            metadata.st_ctime_ns, metadata.st_nlink) != (
            after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns,
            after.st_ctime_ns, after.st_nlink
        ):
            raise SystemExit("error: Debian control changed while being read")
        return b"".join(chunks)
    finally:
        os.close(descriptor)

payload = read_regular()
lines, fields = parse(payload)
matches = [field for field in fields if field["key"].lower() == "x-kanban-build-id"]
if len(matches) > 1:
    raise SystemExit("error: Debian control contains duplicate X-Kanban-Build-Id fields")
if matches:
    field = matches[0]
    if field["lines"] != [(field["index"], field["lines"][0][1])] or field["value"] != " " + build_id:
        raise SystemExit("error: Debian control contains a conflicting X-Kanban-Build-Id field")

remove = set()
for field in matches:
    remove.update(index for index, _ in field["lines"])
remaining = [line for index, line in enumerate(lines) if index not in remove]
description_at = next(
    (index for index, line in enumerate(remaining) if line.startswith("Description:")),
    len(remaining),
)
remaining.insert(description_at, f"X-Kanban-Build-Id: {build_id}")
rewritten = ("\n".join(remaining) + "\n").encode("utf-8")

parent = path.parent
parent_fd = os.open(parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0))
temporary_name = f".control-rewrite.{os.getpid()}"
try:
    descriptor = os.open(
        temporary_name,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
        0o600,
        dir_fd=parent_fd,
    )
    try:
        view = memoryview(rewritten)
        while view:
            written = os.write(descriptor, view)
            view = view[written:]
        os.fchmod(descriptor, 0o644)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    os.replace(temporary_name, path.name, src_dir_fd=parent_fd, dst_dir_fd=parent_fd)
    os.fsync(parent_fd)
finally:
    try:
        os.unlink(temporary_name, dir_fd=parent_fd)
    except FileNotFoundError:
        pass
    os.close(parent_fd)

_, final_fields = parse(read_regular())
final_matches = [field for field in final_fields if field["key"].lower() == "x-kanban-build-id"]
if len(final_matches) != 1 or final_matches[0]["key"] != "X-Kanban-Build-Id" or \
    final_matches[0]["value"] != " " + build_id or len(final_matches[0]["lines"]) != 1:
    raise SystemExit("error: Debian control does not contain exactly one canonical build identity field")
PY
}

build_binary() {
  local workspace_packages
  mapfile -t workspace_packages < <(
    cd "$ROOT"
    "${RELEASE_CARGO_PATH:-cargo}" metadata --locked --no-deps --format-version 1 |
      python3 -c 'import json,sys; print("\n".join(p["name"] for p in json.load(sys.stdin)["packages"]))'
  )
  "$PROVENANCE" --invalidate-packages "$TARGET_DIR" "${workspace_packages[@]}"
  rm -f "$BIN_PATH" "$TARGET_DIR/$BIN_NAME.d"
  (
    cd "$ROOT"
    "$LOCK" -- "${RELEASE_CARGO_PATH:-cargo}" build --locked -p kanban-cli --release "${BUILD_ARGS[@]}"
  )
  [[ -x "$BIN_PATH" ]] || { echo "error: expected binary not found: $BIN_PATH" >&2; exit 1; }
  if [[ "$RELEASE_PROVENANCE_ENABLED" == "1" ]]; then
    RELEASE_BINARY_STAGE_DIR="$TMPDIR/release-binaries"
    python3 "$SAFE_PATH" ensure-dir --root "$TMPDIR_PARENT" \
      --path "$RELEASE_BINARY_STAGE_DIR" --mode 0700
    python3 "$SAFE_PATH" validate-file --root "$TARGET_ROOT" --path "$BIN_PATH"
    python3 "$SAFE_PATH" copy-file --root "$TMPDIR_PARENT" \
      --source "$BIN_PATH" --destination "$RELEASE_BINARY_STAGE_DIR/$BIN_NAME" --mode 0555
    python3 "$SAFE_PATH" validate-file --root "$TARGET_ROOT" --path "$TARGET_DIR/$BIN_NAME.d"
  fi
  "$PROVENANCE" --verify-dep-info "$ROOT" "$TARGET_DIR/$BIN_NAME.d"
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
EOF
  if [[ "$RELEASE_PROVENANCE_ENABLED" == "1" ]]; then
    printf 'X-Kanban-Build-Id: %s\n' "$RELEASE_BUILD_ID" >> "$control_dir/control"
  fi
  cat >> "$control_dir/control" <<EOF
Description: Local-first Kanban CLI
 Standalone kanban command line client for the Kanban Tool local work queue.
EOF
  if [[ "$RELEASE_PROVENANCE_ENABLED" == "1" ]]; then
    rewrite_and_verify_control "$control_dir/control"
  fi

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
  python3 "$SAFE_PATH" validate-file --root "$TARGET_ROOT" --path "$staged_file"
  if [[ "$RELEASE_PROVENANCE_ENABLED" == "1" ]]; then
    verify_dir="$TMPDIR/deb-control-verify"
    mkdir -m 0700 "$verify_dir"
    dpkg-deb -e "$staged_file" "$verify_dir"
    rewrite_and_verify_control "$verify_dir/control"
    payload_verify_dir="$TMPDIR/deb-payload-verify"
    mkdir -m 0700 "$payload_verify_dir"
    dpkg-deb -x "$staged_file" "$payload_verify_dir"
    cmp -s "$payload_verify_dir/usr/share/doc/$PACKAGE_NAME/source-provenance.json" \
      "$RELEASE_SOURCE_MANIFEST_STABLE" || {
      echo "error: CLI package source provenance payload drifted" >&2
      exit 1
    }
    cmp -s "$payload_verify_dir/usr/share/doc/$PACKAGE_NAME/derived-projection-v2-source-map.json" \
      "$RELEASE_SOURCE_MAP_STABLE" || {
      echo "error: CLI package source map payload drifted" >&2
      exit 1
    }
  fi
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
