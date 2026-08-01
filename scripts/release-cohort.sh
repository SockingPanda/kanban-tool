#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
SOURCE_MAP_REL="docs/release/derived-projection-v2-source-map.json"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
SAFE_PATH="$ROOT/scripts/release-safe-path.py"
SOURCE_GATE="$ROOT/scripts/release-source-gate.sh"
ARTIFACT_MANIFEST="$ROOT/scripts/release-artifact-manifest.sh"
EMBED_DEB="$ROOT/scripts/embed-release-provenance-deb.sh"
SEALED_ARTIFACT_MANIFEST=""
SEALED_EMBED_DEB=""
SEALED_SOURCE_GATE=""
SEALED_SAFE_PATH=""
RELEASE_FINISHED=0

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
    fail "KANBAN_CARGO_BUILD_LOCK_HELD requires an inherited lock proof"
  fi
}

fail() {
  echo "error: $*" >&2
  exit 1
}

reject_release_environment() {
  local name
  local -a names
  mapfile -t names < <(compgen -A variable)
  for name in "${names[@]}"; do
    case "$name" in
      RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER|RUSTC|CARGO|RUSTDOC|RUSTFLAGS|\
      CARGO_ENCODED_RUSTFLAGS|RUSTDOCFLAGS|CARGO_ENCODED_RUSTDOCFLAGS|\
      CARGO_HOME|RUSTC_BOOTSTRAP|SOURCE_DATE_EPOCH|RUSTUP_TOOLCHAIN|\
      RUSTUP_HOME|RUSTUP_DIST_SERVER|RUSTUP_UPDATE_ROOT|CC|CXX|AR|\
      CFLAGS|CXXFLAGS|CPPFLAGS|LDFLAGS)
        if [[ "${!name+x}" == "x" ]]; then
          fail "release refuses build-affecting environment override: $name"
        fi
        ;;
      CARGO_TARGET_*)
        if [[ "$name" != "CARGO_TARGET_DIR" ]] ||
          ! lock_environment_is_internal; then
          if [[ "${!name+x}" == "x" ]]; then
            fail "release refuses build-affecting environment override: $name"
          fi
        fi
        ;;
      CARGO_BUILD_JOBS|NEXTEST_TEST_THREADS|RUST_TEST_THREADS)
        if ! lock_environment_is_internal; then
          if [[ "${!name+x}" == "x" ]]; then
            fail "release refuses build-affecting environment override: $name"
          fi
        fi
        ;;
      CARGO_BUILD_*|CARGO_HTTP_*|CARGO_NET_*|CARGO_PROFILE_*|\
      CARGO_REGISTRIES_*|CARGO_SOURCE_*|RUSTUP_*|CC_*|CXX_*|PKG_CONFIG_*)
        if [[ "${!name+x}" == "x" ]]; then
          fail "release refuses build-affecting environment override: $name"
        fi
        ;;
    esac
  done
  if [[ "${KANBAN_RELEASE_FEATURES+x}" == "x" &&
    "$KANBAN_RELEASE_FEATURES" != "tantivy-backend,oxigraph-backend" ]]; then
    fail "KANBAN_RELEASE_FEATURES must be the canonical release feature set"
  fi
  if [[ "${KANBAN_RELEASE_NO_DEFAULT_FEATURES+x}" == "x" &&
    "$KANBAN_RELEASE_NO_DEFAULT_FEATURES" != "1" ]]; then
    fail "KANBAN_RELEASE_NO_DEFAULT_FEATURES must remain enabled for release"
  fi
  for name in KANBAN_BUILD_ID KANBAN_RELEASE_SOURCE_MANIFEST \
    KANBAN_RELEASE_SOURCE_MAP KANBAN_RELEASE_GENERATION_KEY \
    KANBAN_RELEASE_IDENTITY_SHA256 KANBAN_RELEASE_SOURCE_GATE \
    KANBAN_RELEASE_SOURCE_ROOT KANBAN_RELEASE_SAFE_PATH; do
    if [[ "${!name+x}" == "x" ]]; then
      fail "release refuses caller-supplied provenance environment: $name"
    fi
  done
}

require_inherited_lock_if_marked
reject_release_environment

[[ $# -eq 0 ]] || {
  echo "error: scripts/release-cohort.sh does not accept arguments" >&2
  exit 2
}

# This lock is intentionally acquired before inspecting Git or the target
# directory. Every nested just/package invocation is reentrant through
# KANBAN_CARGO_BUILD_LOCK_HELD, so no other cohort can replace live build
# outputs between compilation, stable staging, hashing, and publication.
if [[ "${KANBAN_CARGO_BUILD_LOCK_HELD:-}" != "1" ]]; then
  exec "$LOCK" -- "$0"
fi

cd "$ROOT"
TARGET_ROOT="$("$LOCK" --print-target-dir)"
[[ "$TARGET_ROOT" == /* && -d "$TARGET_ROOT" ]] || {
  echo "error: Cargo target root must be an existing absolute directory" >&2
  exit 1
}
python3 "$SAFE_PATH" ensure-dir --root "$TARGET_ROOT" --path "$TARGET_ROOT" --mode 0755
TARGET_ROOT="$(cd "$TARGET_ROOT" && pwd -P)"
case "$TARGET_ROOT/" in
  "$ROOT/"*)
    echo "error: release cohort refuses a Cargo target root inside the source tree" >&2
    exit 1
    ;;
esac

BUNDLE_ROOT="$TARGET_ROOT/release/bundle"
COHORT_ROOT="$BUNDLE_ROOT/cohort"
python3 "$SAFE_PATH" ensure-dir --root "$TARGET_ROOT" \
  --path "$COHORT_ROOT" --mode 0755
BOOTSTRAP_DIR="$(
  python3 "$SAFE_PATH" private-dir --root "$TARGET_ROOT" \
    --parent "$COHORT_ROOT" --prefix ".cohort-bootstrap."
)"
read -r BOOTSTRAP_DEV BOOTSTRAP_INO < <(
  python3 "$SAFE_PATH" dir-identity --root "$TARGET_ROOT" \
    --path "$BOOTSTRAP_DIR"
)
STAGE_DIR=""
SNAPSHOT_DIR=""
SNAPSHOT_DEV=""
SNAPSHOT_INO=""
cleanup_bootstrap() {
  if [[ -n "${BOOTSTRAP_DIR:-}" ]]; then
    python3 "$SAFE_PATH" remove-tree --root "$TARGET_ROOT" \
      --path "$BOOTSTRAP_DIR" --expected-dev "$BOOTSTRAP_DEV" \
      --expected-ino "$BOOTSTRAP_INO" ||
      printf 'warning: failed source-identity bootstrap retained: %s\n' \
        "$BOOTSTRAP_DIR" >&2
  fi
}
cleanup_snapshot() {
  local keep_snapshot=0
  # Keep the sealed source snapshot only while a durable stage/publish intent
  # is actually retryable.  A crash can leave the generation under its
  # published name while the post-rename verifier still needs absolute paths
  # to the pinned tools; deleting the snapshot on EXIT would strand those
  # paths.  Ordinary validation failures retain only their diagnostic stage,
  # so a new release attempt is not confused by an unrelated source snapshot.
  if [[ "${RELEASE_FINISHED:-0}" != "1" &&
    -n "${PUBLISHED_DIR:-}" && -e "$PUBLISHED_DIR.publishing" &&
    ! -L "$PUBLISHED_DIR.publishing" ]]; then
    keep_snapshot=1
  elif [[ "${RELEASE_FINISHED:-0}" != "1" &&
    -n "${PUBLISHED_MARKER:-}" && -f "$PUBLISHED_MARKER" &&
    ! -L "$PUBLISHED_MARKER" ]]; then
    keep_snapshot=1
  elif [[ "${RELEASE_FINISHED:-0}" != "1" &&
    -n "${STAGE_DIR:-}" && -d "$STAGE_DIR" &&
    ! -L "$STAGE_DIR" && "$(stat -c '%a' "$STAGE_DIR" 2>/dev/null || true)" == "555" &&
    -n "${ARTIFACT_MANIFEST_FINAL:-}" && -f "$ARTIFACT_MANIFEST_FINAL" ]]; then
    keep_snapshot=1
  fi
  if [[ "$keep_snapshot" == "1" ]]; then
    return 0
  fi
  if [[ -n "${SNAPSHOT_DIR:-}" && -n "${SNAPSHOT_DEV:-}" &&
    -n "${SNAPSHOT_INO:-}" && "$(dirname "$SNAPSHOT_DIR")" == "$COHORT_ROOT" &&
    "$(basename "$SNAPSHOT_DIR")" == .cohort-source.* && -d "$SNAPSHOT_DIR" &&
    ! -L "$SNAPSHOT_DIR" ]]; then
    python3 "$SAFE_PATH" remove-tree --root "$TARGET_ROOT" \
      --path "$SNAPSHOT_DIR" --expected-dev "$SNAPSHOT_DEV" \
      --expected-ino "$SNAPSHOT_INO" ||
      printf 'warning: retained verified source snapshot: %s\n' "$SNAPSHOT_DIR" >&2
  fi
}
cleanup_stage() {
  if [[ -n "${STAGE_DIR:-}" && "$(dirname "$STAGE_DIR")" == "$COHORT_ROOT" &&
    "$(basename "$STAGE_DIR")" == .cohort-stage.* && -d "$STAGE_DIR" &&
    ! -L "$STAGE_DIR" ]]; then
    printf 'warning: failed release stage retained for pinned manual cleanup: %s\n' \
      "$STAGE_DIR" >&2
  fi
}
cleanup_release_state() {
  cleanup_bootstrap
  cleanup_snapshot
  cleanup_stage
}
trap cleanup_release_state EXIT

BOOTSTRAP_MANIFEST="$BOOTSTRAP_DIR/source-provenance.json"
SOURCE_MAP="$ROOT/docs/release/derived-projection-v2-source-map.json"

create_source_snapshot() {
  local archive source_root
  [[ -n "$SNAPSHOT_DIR" ]] || fail "source snapshot path was not initialized"
  if [[ -e "$SNAPSHOT_DIR" || -L "$SNAPSHOT_DIR" ]]; then
    fail "deterministic source snapshot already exists and cannot be re-used blindly: $SNAPSHOT_DIR"
  fi
  python3 "$SAFE_PATH" ensure-dir --root "$TARGET_ROOT" \
    --path "$SNAPSHOT_DIR" --mode 0700
  read -r SNAPSHOT_DEV SNAPSHOT_INO < <(
    python3 "$SAFE_PATH" dir-identity --root "$TARGET_ROOT" \
      --path "$SNAPSHOT_DIR"
  )
  archive="$SNAPSHOT_DIR/source.tar"
  source_root="$SNAPSHOT_DIR/source"

  # Archive the pinned Git object rather than the live working tree.  The
  # resulting tree is independent of later source edits and is the only
  # directory used as the Just/Cargo working directory during the build.
  git -C "$ROOT" archive --format=tar --prefix=source/ "$COMMIT" >"$archive" ||
    fail "cannot create the pinned immutable release source archive"
  python3 "$SAFE_PATH" validate-file --root "$TARGET_ROOT" --path "$archive"
  python3 - "$archive" <<'PY'
import pathlib
import sys
import tarfile

archive = pathlib.Path(sys.argv[1])
try:
    members = tarfile.open(archive, "r:").getmembers()
except (OSError, tarfile.TarError) as error:
    raise SystemExit(f"error: release source archive is invalid: {error}")
if not members:
    raise SystemExit("error: release source archive is empty")
for member in members:
    path = pathlib.PurePosixPath(member.name)
    if path.is_absolute() or ".." in path.parts or not path.parts or path.parts[0] != "source":
        raise SystemExit(f"error: release source archive has an unsafe member: {member.name}")
    if member.issym() or member.islnk() or not (member.isdir() or member.isfile()):
        raise SystemExit(f"error: release source archive has a non-regular member: {member.name}")
PY
  tar --extract --file "$archive" --directory "$SNAPSHOT_DIR" \
    --no-same-owner --no-same-permissions
  [[ -d "$source_root" && ! -L "$source_root" ]] ||
    fail "release source archive did not produce a real source directory"
  python3 - "$source_root/justfile" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
try:
    text = path.read_text(encoding="utf-8")
except (OSError, UnicodeError) as error:
    raise SystemExit(f"error: immutable release source lacks a readable justfile: {error}")
required = (
    'cli-package:\n    scripts/package-cli-linux.sh --format deb --no-default-features --features "tantivy-backend,oxigraph-backend"',
    'projection-release-cohort:\n    just feature-p kanban-cli "tantivy-backend,oxigraph-backend"',
    '    just feature-p kanban-server "tantivy-backend,oxigraph-backend"',
)
for fragment in required:
    if fragment not in text:
        raise SystemExit(
            "error: immutable release justfile does not carry the canonical effective build contract"
        )
PY

  # The build may create only the known frontend/sidecar output directories;
  # every versioned source file remains read-only.  No command writes to the
  # live repository while the cohort is running.
  python3 "$SAFE_PATH" ensure-dir --root "$SNAPSHOT_DIR" \
    --path "$source_root/apps/desktop/dist" --mode 0700
  python3 "$SAFE_PATH" ensure-dir --root "$SNAPSHOT_DIR" \
    --path "$source_root/apps/desktop/src-tauri/binaries" --mode 0700
  python3 "$SAFE_PATH" seal-tree --root "$TARGET_ROOT" --path "$SNAPSHOT_DIR"
  python3 "$SAFE_PATH" ensure-dir --root "$TARGET_ROOT" \
    --path "$source_root/apps/desktop/dist" --mode 0700
  python3 "$SAFE_PATH" ensure-dir --root "$TARGET_ROOT" \
    --path "$source_root/apps/desktop/src-tauri/binaries" --mode 0700
}

assert_toolchain_unchanged() {
  local rustc_output cargo_output rustc_hash cargo_hash
  [[ -x "$RELEASE_RUSTC" && -x "$RELEASE_CARGO" ]] ||
    fail "pinned release rustc/cargo executable disappeared"
  [[ "$(command -v rustc 2>/dev/null || true)" == "$RELEASE_RUSTC" ]] ||
    fail "release rustc command no longer resolves to the recorded executable"
  [[ "$(command -v cargo 2>/dev/null || true)" == "$RELEASE_CARGO" ]] ||
    fail "release cargo command no longer resolves to the recorded executable"
  rustc_output="$($RELEASE_RUSTC -vV)" || fail "recorded rustc -vV failed during release"
  cargo_output="$($RELEASE_CARGO --version)" || fail "recorded cargo --version failed during release"
  rustc_hash="$(printf '%s' "$rustc_output" | sha256sum | awk '{print $1}')"
  cargo_hash="$(printf '%s' "$cargo_output" | sha256sum | awk '{print $1}')"
  [[ "$rustc_hash" == "$RELEASE_RUSTC_VV_SHA256" ]] ||
    fail "recorded rustc output changed during release"
  [[ "$cargo_hash" == "$RELEASE_CARGO_VERSION_SHA256" ]] ||
    fail "recorded cargo output changed during release"
  [[ "$(sha256sum "$RELEASE_RUSTC" | awk '{print $1}')" == "$RELEASE_RUSTC_SHA256" ]] ||
    fail "recorded rustc executable changed during release"
  [[ "$(sha256sum "$RELEASE_CARGO" | awk '{print $1}')" == "$RELEASE_CARGO_SHA256" ]] ||
    fail "recorded cargo executable changed during release"
}

"$SOURCE_GATE" prepare --output "$BOOTSTRAP_MANIFEST"
read -r VERSION COMMIT TREE KANBAN_BUILD_ID GENERATION_KEY IDENTITY_SHA256 < <(
  python3 - "$BOOTSTRAP_MANIFEST" <<'PY'
import json
import pathlib
import sys

document = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
values = [
    document.get("version"),
    document.get("commit"),
    document.get("tree"),
    document.get("build_id"),
    document.get("generation_key"),
    document.get("identity_sha256"),
]
if not all(isinstance(value, str) and value for value in values):
    raise SystemExit("error: source provenance manifest lacks cohort identity")
print(*values)
PY
)
[[ "$GENERATION_KEY" == "$COMMIT-$TREE-$IDENTITY_SHA256" ]] || {
  echo "error: source provenance generation identity is not canonical" >&2
  exit 1
}
[[ "$IDENTITY_SHA256" =~ ^[0-9a-f]{64}$ ]] || {
  echo "error: source provenance identity hash is invalid" >&2
  exit 1
}
GENERATION="$GENERATION_KEY"
STAGE_DIR="$COHORT_ROOT/.cohort-stage.$GENERATION"
SNAPSHOT_DIR="$COHORT_ROOT/.cohort-source.$GENERATION"
PUBLISHED_DIR="$COHORT_ROOT/$GENERATION"
SEALED_TOOLS_DIR="$STAGE_DIR/.release-tools"
PUBLISHED_MARKER="$PUBLISHED_DIR.published"
SOURCE_MANIFEST="$STAGE_DIR/source-provenance.json"
ARTIFACT_MANIFEST_PENDING="$STAGE_DIR/release-artifacts.pending.json"
ARTIFACT_MANIFEST_FINAL="$STAGE_DIR/release-artifacts.json"
RESUME_RELEASE=0
RESUME_PUBLISHED=0

if [[ -e "$SNAPSHOT_DIR" || -L "$SNAPSHOT_DIR" ]]; then
  [[ -d "$SNAPSHOT_DIR" && ! -L "$SNAPSHOT_DIR" ]] ||
    fail "deterministic source snapshot is unsafe: $SNAPSHOT_DIR"
  read -r SNAPSHOT_DEV SNAPSHOT_INO < <(
    python3 "$SAFE_PATH" dir-identity --root "$TARGET_ROOT" \
      --path "$SNAPSHOT_DIR"
  )
fi

RELEASE_RUSTC="$(command -v rustc 2>/dev/null || true)"
RELEASE_CARGO="$(command -v cargo 2>/dev/null || true)"
[[ -x "$RELEASE_RUSTC" && -x "$RELEASE_CARGO" ]] ||
  fail "release requires executable rustc and cargo on PATH"
RELEASE_RUSTC_SHA256="$(sha256sum "$RELEASE_RUSTC" | awk '{print $1}')"
RELEASE_CARGO_SHA256="$(sha256sum "$RELEASE_CARGO" | awk '{print $1}')"
read -r RELEASE_TARGET_TRIPLE RELEASE_FEATURES_EFFECTIVE RELEASE_NO_DEFAULT \
  RELEASE_RUSTC_VV_SHA256 RELEASE_CARGO_VERSION_SHA256 < <(
  python3 - "$BOOTSTRAP_MANIFEST" <<'PY'
import json
import pathlib
import sys

document = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
identity = document["identity"]
features = identity["features"]
toolchain = identity["toolchain"]
print(
    identity["target"]["triple"],
    ",".join(features["effective"]),
    "1" if features["no_default_features"] else "0",
    toolchain["rustc_vv_sha256"],
    toolchain["cargo_version_sha256"],
)
PY
)
RELEASE_FEATURES_CSV="tantivy-backend,oxigraph-backend"
[[ "$RELEASE_TARGET_TRIPLE" =~ ^[A-Za-z0-9][A-Za-z0-9_.-]*$ &&
  "$RELEASE_FEATURES_EFFECTIVE" == "oxigraph-backend,tantivy-backend" &&
  "$RELEASE_NO_DEFAULT" == "1" ]] ||
  fail "source provenance does not carry the canonical release build settings"

# A commit/tree pair is not a sufficient resume key.  Refuse to reuse any
# legacy generation or sibling identity for the same pair; this keeps an old
# v1/v2 manifest, or a cohort made by a different toolchain/feature/target,
# from being silently adopted by a new release attempt.
python3 - "$COHORT_ROOT" "$COMMIT-$TREE" "$GENERATION" <<'PY'
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
commit_tree = sys.argv[2]
current = sys.argv[3]
allowed = {
    current,
    current + ".published",
    current + ".publishing",
    ".cohort-stage." + current,
    ".cohort-source." + current,
}
for entry in root.iterdir():
    name = entry.name
    if name in allowed or name.startswith(".cohort-bootstrap."):
        continue
    if (
      name == commit_tree
      or name.startswith(commit_tree + "-")
      or name.startswith(".cohort-stage." + commit_tree)
      or name.startswith(".cohort-source." + commit_tree)
    ):
        raise SystemExit(
            "error: release commit/tree has a different or legacy identity: " + name
        )
PY

if [[ -e "$PUBLISHED_MARKER" || -L "$PUBLISHED_MARKER" ]]; then
  [[ -f "$PUBLISHED_MARKER" && ! -L "$PUBLISHED_MARKER" &&
    -d "$PUBLISHED_DIR" && ! -L "$PUBLISHED_DIR" &&
    ! -e "$STAGE_DIR" && ! -L "$STAGE_DIR" &&
    ! -e "$PUBLISHED_DIR.publishing" &&
    ! -L "$PUBLISHED_DIR.publishing" ]] || {
    echo "error: ambiguous committed release recovery state for $GENERATION" >&2
    exit 1
  }
  RESUME_PUBLISHED=1
  SOURCE_MANIFEST="$PUBLISHED_DIR/source-provenance.json"
elif [[ -e "$PUBLISHED_DIR" || -L "$PUBLISHED_DIR" ]]; then
  [[ -d "$PUBLISHED_DIR" && ! -L "$PUBLISHED_DIR" &&
    ! -e "$STAGE_DIR" && ! -L "$STAGE_DIR" ]] || {
    echo "error: ambiguous release recovery state for $GENERATION" >&2
    exit 1
  }
  python3 "$SAFE_PATH" recover-publish-dir --root "$TARGET_ROOT" \
    --source "$STAGE_DIR" --destination "$PUBLISHED_DIR" >/dev/null
fi
if [[ "$RESUME_PUBLISHED" == "1" ]]; then
  :
elif [[ -e "$STAGE_DIR" || -L "$STAGE_DIR" ]]; then
  [[ -d "$STAGE_DIR" && ! -L "$STAGE_DIR" ]] || {
    echo "error: deterministic release stage is unsafe: $STAGE_DIR" >&2
    exit 1
  }
  python3 "$SAFE_PATH" dir-identity --root "$TARGET_ROOT" \
    --path "$STAGE_DIR" >/dev/null
  [[ "$(stat -c '%a' "$STAGE_DIR")" == "555" ]] || {
    echo "error: deterministic release stage is incomplete: $STAGE_DIR" >&2
    exit 1
  }
  python3 "$SAFE_PATH" validate-file --root "$TARGET_ROOT" \
    --path "$SOURCE_MANIFEST"
  cmp -s "$BOOTSTRAP_MANIFEST" "$SOURCE_MANIFEST" || {
    echo "error: durable release stage source identity drifted" >&2
    exit 1
  }
  RESUME_RELEASE=1
else
  python3 "$SAFE_PATH" ensure-dir --root "$TARGET_ROOT" \
    --path "$STAGE_DIR" --mode 0700
  python3 "$SAFE_PATH" copy-file --root "$TARGET_ROOT" \
    --source "$BOOTSTRAP_MANIFEST" --destination "$SOURCE_MANIFEST" \
    --mode 0644
fi
cleanup_bootstrap
BOOTSTRAP_DIR=""

SOURCE_SNAPSHOT_ROOT="$SNAPSHOT_DIR/source"

bind_sealed_release_tools() {
  local tools_root
  if [[ -d "$SOURCE_SNAPSHOT_ROOT" && ! -L "$SOURCE_SNAPSHOT_ROOT" ]]; then
    tools_root="$SOURCE_SNAPSHOT_ROOT"
    SEALED_TOOLS_DIR="$STAGE_DIR/.release-tools"
  elif [[ "$RESUME_RELEASE" == "1" && -d "$STAGE_DIR/.release-tools" &&
    ! -L "$STAGE_DIR/.release-tools" ]]; then
    tools_root="$STAGE_DIR/.release-tools"
    SEALED_TOOLS_DIR="$STAGE_DIR/.release-tools"
  elif [[ "$RESUME_PUBLISHED" == "1" && -d "$PUBLISHED_DIR/.release-tools" &&
    ! -L "$PUBLISHED_DIR/.release-tools" ]]; then
    tools_root="$PUBLISHED_DIR/.release-tools"
    SEALED_TOOLS_DIR="$PUBLISHED_DIR/.release-tools"
  else
    fail "immutable release tooling snapshot is unavailable"
  fi
  SEALED_ARTIFACT_MANIFEST="$tools_root/scripts/release-artifact-manifest.sh"
  SEALED_EMBED_DEB="$tools_root/scripts/embed-release-provenance-deb.sh"
  SEALED_SOURCE_GATE="$tools_root/scripts/release-source-gate.sh"
  SEALED_SAFE_PATH="$tools_root/scripts/release-safe-path.py"
  for tool in "$SEALED_ARTIFACT_MANIFEST" "$SEALED_EMBED_DEB" \
    "$SEALED_SOURCE_GATE" "$SEALED_SAFE_PATH"; do
    python3 "$SAFE_PATH" validate-file --root "$TARGET_ROOT" --path "$tool"
    [[ -x "$tool" ]] || fail "sealed release tool is not executable: $tool"
  done
  ARTIFACT_MANIFEST="$SEALED_ARTIFACT_MANIFEST"
  EMBED_DEB="$SEALED_EMBED_DEB"
}

persist_sealed_release_tools() {
  [[ "$SEALED_TOOLS_DIR" == "$STAGE_DIR/.release-tools" ]] ||
    fail "sealed release tooling destination is not the deterministic stage"
  python3 "$SAFE_PATH" ensure-dir --root "$TARGET_ROOT" \
    --path "$SEALED_TOOLS_DIR/scripts" --mode 0700
  python3 "$SAFE_PATH" copy-file --root "$TARGET_ROOT" \
    --source "$SEALED_ARTIFACT_MANIFEST" \
    --destination "$SEALED_TOOLS_DIR/scripts/release-artifact-manifest.sh" --mode 0555
  python3 "$SAFE_PATH" copy-file --root "$TARGET_ROOT" \
    --source "$SEALED_EMBED_DEB" \
    --destination "$SEALED_TOOLS_DIR/scripts/embed-release-provenance-deb.sh" --mode 0555
  for dependency in cargo-build-lock.sh release-safe-path.py release-source-gate.sh; do
    python3 "$SAFE_PATH" copy-file --root "$TARGET_ROOT" \
      --source "$SOURCE_SNAPSHOT_ROOT/scripts/$dependency" \
      --destination "$SEALED_TOOLS_DIR/scripts/$dependency" --mode 0555
  done
}

if [[ "$RESUME_PUBLISHED" != "1" && "$RESUME_RELEASE" != "1" ]]; then
  create_source_snapshot
  bind_sealed_release_tools
  persist_sealed_release_tools
elif [[ "$RESUME_RELEASE" == "1" ]]; then
  bind_sealed_release_tools
elif [[ "$RESUME_PUBLISHED" == "1" ]]; then
  bind_sealed_release_tools
fi

# Every post-bind safe-path operation must use the immutable copy selected
# above.  The live source root remains exported separately for Git/source
# evidence, but a mutable live helper must never control validation or
# publication after the sealed tooling boundary.
SAFE_PATH="$SEALED_SAFE_PATH"

# All checks after the immutable snapshot is bound use the sealed source-gate
# code while explicitly pointing it at the live tree for Git/source evidence.
# This keeps a live script replacement out of both the recheck and the final
# publish verifier without pretending that the archived source is a Git repo.
SOURCE_GATE="$SEALED_SOURCE_GATE"
export KANBAN_RELEASE_SOURCE_ROOT="$ROOT"
export KANBAN_RELEASE_SAFE_PATH="$SEALED_SAFE_PATH"

if [[ "$RESUME_PUBLISHED" != "1" && "$RESUME_RELEASE" != "1" ]]; then
  # The archive is made from the pinned object, but source-gate still runs
  # immediately afterwards so a live-tree edit during archive creation stops
  # the release before any build starts.  It must use the sealed script now
  # that the snapshot has been bound; only the Git/source evidence root stays
  # pointed at the live tree.
  "$SOURCE_GATE" verify --manifest "$STAGE_DIR/source-provenance.json"
fi

export KANBAN_BUILD_ID
export KANBAN_RELEASE_GENERATION_KEY="$GENERATION_KEY"
export KANBAN_RELEASE_IDENTITY_SHA256="$IDENTITY_SHA256"
export KANBAN_RELEASE_SOURCE_MANIFEST="$SOURCE_MANIFEST"
export KANBAN_RELEASE_SOURCE_MAP="$SOURCE_MAP"

JUST_BIN="$(command -v just 2>/dev/null || true)"
[[ -x "$JUST_BIN" ]] || fail "release requires just on PATH"
RELEASE_ORIGINAL_PATH="$PATH"
RELEASE_TOOLCHAIN_PATH="$(dirname "$RELEASE_CARGO"):$(dirname "$RELEASE_RUSTC")"

# Keep the literal `just <recipe>` calls below: the recipe witness and the
# release trace treat that sequence as the public orchestration contract.  The
# function redirects those calls to the sealed source snapshot, while passing
# the canonical target/features/default-mode and the exact recorded toolchain.
just() {
  local recipe="${1:-}"
  [[ -n "$recipe" ]] || fail "release recipe name is required"
  assert_toolchain_unchanged
  if [[ "$recipe" == "diff-check" ]]; then
    (
      cd "$ROOT"
      env \
        PATH="$RELEASE_TOOLCHAIN_PATH:$RELEASE_ORIGINAL_PATH" \
        RUSTC="$RELEASE_RUSTC" \
        CARGO="$RELEASE_CARGO" \
        CARGO_BUILD_TARGET="$RELEASE_TARGET_TRIPLE" \
        KANBAN_RELEASE_FEATURES="$RELEASE_FEATURES_CSV" \
        KANBAN_RELEASE_NO_DEFAULT_FEATURES="$RELEASE_NO_DEFAULT" \
        KANBAN_RELEASE_SOURCE_MANIFEST="$SOURCE_MANIFEST" \
        KANBAN_RELEASE_SOURCE_MAP="$SOURCE_MAP" \
        "$JUST_BIN" "$@"
    )
    return
  fi
  [[ -d "$SOURCE_SNAPSHOT_ROOT" && ! -L "$SOURCE_SNAPSHOT_ROOT" ]] ||
    fail "immutable release source snapshot is unavailable"
  (
    cd "$SOURCE_SNAPSHOT_ROOT"
    env \
      PATH="$RELEASE_TOOLCHAIN_PATH:$RELEASE_ORIGINAL_PATH" \
      RUSTC="$RELEASE_RUSTC" \
      CARGO="$RELEASE_CARGO" \
      CARGO_BUILD_TARGET="$RELEASE_TARGET_TRIPLE" \
      KANBAN_RELEASE_FEATURES="$RELEASE_FEATURES_CSV" \
      KANBAN_RELEASE_NO_DEFAULT_FEATURES="$RELEASE_NO_DEFAULT" \
      KANBAN_RELEASE_SOURCE_MANIFEST="$SOURCE_MANIFEST" \
      KANBAN_RELEASE_SOURCE_MAP="$SOURCE_SNAPSHOT_ROOT/$SOURCE_MAP_REL" \
      "$JUST_BIN" --justfile "$SOURCE_SNAPSHOT_ROOT/justfile" "$@"
  )
}

publish_generation() {
  "$SOURCE_GATE" verify --manifest "$SOURCE_MANIFEST"
  "$ARTIFACT_MANIFEST" verify \
    --manifest "$ARTIFACT_MANIFEST_FINAL" \
    --stage-dir "$STAGE_DIR"
  GENERATION_SHA256="$(
    python3 "$SAFE_PATH" tree-digest --root "$TARGET_ROOT" \
      --path "$STAGE_DIR" --require-sealed
  )"
  python3 "$SAFE_PATH" publish-dir --root "$TARGET_ROOT" \
    --source "$STAGE_DIR" --destination "$PUBLISHED_DIR" \
    --expected-tree-sha256 "$GENERATION_SHA256" \
    --verify-command "$ARTIFACT_MANIFEST" verify-final \
      --manifest "$ARTIFACT_MANIFEST_FINAL" --stage-dir "$STAGE_DIR"
  python3 "$SAFE_PATH" validate-published-dir --root "$TARGET_ROOT" \
    --path "$PUBLISHED_DIR" --marker "$PUBLISHED_MARKER" \
    --expected-tree-sha256 "$GENERATION_SHA256"
}

resume_published_generation() {
  local generation_sha256
  generation_sha256="$(
    python3 "$SAFE_PATH" validate-published-dir --root "$TARGET_ROOT" \
      --path "$PUBLISHED_DIR" --marker "$PUBLISHED_MARKER" \
      --verify-command "$ARTIFACT_MANIFEST" verify-final \
        --manifest "$PUBLISHED_DIR/release-artifacts.json" \
        --stage-dir "$PUBLISHED_DIR"
  )"
  [[ "$generation_sha256" =~ ^[0-9a-f]{64}$ ]] || {
    echo "error: committed release recovery returned an invalid tree digest" >&2
    exit 1
  }
}

finish_release() {
  RELEASE_FINISHED=1
  cleanup_snapshot
  SNAPSHOT_DIR=""
  STAGE_DIR=""
  trap - EXIT
  printf 'release cohort published: %s\n' "$PUBLISHED_DIR"
}

if [[ "$RESUME_PUBLISHED" == "1" ]]; then
  resume_published_generation
  finish_release
  exit 0
fi

if [[ "$RESUME_RELEASE" == "1" ]]; then
  publish_generation
  finish_release
  exit 0
fi

just affected-self-test
just schema-contract
just audit
just rust-full
just check-windows-p kanban-local
just projection-release-cohort
just bench-check
just target-tools
just cli-package
just cli-package-layout
just desktop-package-config
just desktop-package

mapfile -d '' DESKTOP_DEBS < <(
  find "$TARGET_ROOT/release/bundle/deb" -maxdepth 1 -type f \
    -name "Kanban Tool_${VERSION}_*.deb" -print0 2>/dev/null
)
[[ "${#DESKTOP_DEBS[@]}" -eq 1 ]] || {
  echo "error: Desktop Debian package must resolve to exactly one artifact" >&2
  exit 1
}
KANBAN_RELEASE_SOURCE_MAP="$SOURCE_SNAPSHOT_ROOT/$SOURCE_MAP_REL" \
  "$EMBED_DEB" --deb "${DESKTOP_DEBS[0]}" --doc-dir kanban-tool-desktop

just desktop-package-layout
just smoke
just diff-check

"$ARTIFACT_MANIFEST" prepare \
  --source-manifest "$SOURCE_MANIFEST" \
  --stage-dir "$STAGE_DIR" \
  --published-dir "$PUBLISHED_DIR" \
  --output "$ARTIFACT_MANIFEST_PENDING"

# A source recheck is intentionally between hash and artifact rehash. Any
# concurrent mutation of either Git state or staged files is detected before a
# final manifest or generation becomes visible.
"$SOURCE_GATE" verify --manifest "$SOURCE_MANIFEST"
"$ARTIFACT_MANIFEST" verify \
  --manifest "$ARTIFACT_MANIFEST_PENDING" \
  --stage-dir "$STAGE_DIR"

python3 "$SAFE_PATH" publish-file --root "$TARGET_ROOT" \
  --source "$ARTIFACT_MANIFEST_PENDING" \
  --destination "$ARTIFACT_MANIFEST_FINAL"
python3 "$SAFE_PATH" seal-tree --root "$TARGET_ROOT" --path "$STAGE_DIR"
publish_generation
finish_release
