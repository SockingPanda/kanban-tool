#!/usr/bin/env bash
set -euo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
ROOT="${KANBAN_RELEASE_SOURCE_ROOT:-$SCRIPT_ROOT}"
SAFE_PATH="${KANBAN_RELEASE_SAFE_PATH:-$ROOT/scripts/release-safe-path.py}"
SOURCE_MAP_REL="docs/release/derived-projection-v2-source-map.json"
SOURCE_MAP="$ROOT/$SOURCE_MAP_REL"
LOCKFILE_REL="Cargo.lock"
LOCKFILE="$ROOT/$LOCKFILE_REL"
REGISTRY_CLOSURE_REL="policy/schema-tool-registry-closure.json"
REGISTRY_CLOSURE="$ROOT/$REGISTRY_CLOSURE_REL"
RELEASE_FEATURES_DEFAULT="tantivy-backend,oxigraph-backend"
RELEASE_NO_DEFAULT_FEATURES_DEFAULT="1"
SAVED_SOURCE_REF="refs/remotes/origin/derived-projection-v2"
LOCK="$SCRIPT_ROOT/scripts/cargo-build-lock.sh"

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

# Release provenance is deliberately fail-closed around build-affecting
# environment.  The identity records the effective Cargo/Rust settings, so a
# caller must not be able to smuggle an unrecorded wrapper, target, flag,
# profile, registry, or native-toolchain override into one of the release
# commands.  The shared target root is owned by cargo-build-lock.sh and is
# therefore the one intentional exception to the CARGO_TARGET_* family.
reject_build_environment() {
  local name expected_target
  local -a names
  mapfile -t names < <(compgen -A variable)
  for name in "${names[@]}"; do
    case "$name" in
      RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER|RUSTC|CARGO|RUSTDOC|RUSTFLAGS|\
      CARGO_ENCODED_RUSTFLAGS|RUSTDOCFLAGS|CARGO_ENCODED_RUSTDOCFLAGS|\
      CARGO_HOME|CARGO_BUILD_RUSTC|CARGO_BUILD_RUSTC_WRAPPER|\
      CARGO_BUILD_RUSTDOC|RUSTC_BOOTSTRAP|SOURCE_DATE_EPOCH|\
      RUSTUP_TOOLCHAIN|RUSTUP_HOME|RUSTUP_DIST_SERVER|RUSTUP_UPDATE_ROOT|\
      CC|CXX|AR|CFLAGS|CXXFLAGS|CPPFLAGS|LDFLAGS|\
      PKG_CONFIG_PATH|PKG_CONFIG_LIBDIR)
        if [[ "${!name+x}" == "x" ]]; then
          fail "release refuses build-affecting environment override: $name"
        fi
        ;;
      CARGO_TARGET_*)
        # cargo-build-lock.sh creates CARGO_TARGET_DIR only after the wrapper
        # has acquired the exclusive lock.  It is never accepted as caller
        # input here; all other CARGO_TARGET_* variables are always rejected.
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
      CARGO_BUILD_TARGET)
        # release-cohort exports the canonical target while nested recipes
        # run.  It must still be the host target captured from the pinned
        # rustc; arbitrary target overrides remain fail-closed here.
        expected_target="$(rustc -vV 2>/dev/null | awk '/^host:[[:space:]]+/ { print $2; count += 1 } END { if (count != 1) exit 1 }')" ||
          fail "cannot derive canonical target while checking CARGO_BUILD_TARGET"
        [[ "${!name}" == "$expected_target" ]] ||
          fail "release refuses non-canonical CARGO_BUILD_TARGET: ${!name}"
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
    "${KANBAN_RELEASE_FEATURES}" != "$RELEASE_FEATURES_DEFAULT" ]]; then
    fail "KANBAN_RELEASE_FEATURES must be the canonical release feature set"
  fi
  if [[ "${KANBAN_RELEASE_NO_DEFAULT_FEATURES+x}" == "x" &&
    "${KANBAN_RELEASE_NO_DEFAULT_FEATURES}" != "$RELEASE_NO_DEFAULT_FEATURES_DEFAULT" ]]; then
    fail "KANBAN_RELEASE_NO_DEFAULT_FEATURES must remain enabled for release"
  fi
}

usage() {
  cat <<'EOF'
Usage:
  scripts/release-source-gate.sh prepare --output <manifest.json>
  scripts/release-source-gate.sh verify --manifest <manifest.json>
  scripts/release-source-gate.sh validate --manifest <manifest.json>

The gate accepts only a clean symbolic main whose HEAD exactly matches the
live origin/main tip. It also proves that the saved and live
origin/derived-projection-v2 tips exactly match the canonical source map, that
all three source slices are ancestors of that tip, and that main and the saved
source history have no merge base.
EOF
}

fail() {
  echo "error: $*" >&2
  exit 1
}

require_inherited_lock_if_marked
reject_build_environment

require_sha1() {
  local label="$1" value="$2"
  [[ "$value" =~ ^[0-9a-f]{40}$ ]] ||
    fail "$label must be one 40-character lowercase Git object id, got: $value"
}

validate_source_map() {
  local -a rows
  local kind source integrated

  [[ -f "$SOURCE_MAP" && ! -L "$SOURCE_MAP" ]] ||
    fail "release source map is missing or not a regular file: $SOURCE_MAP_REL"
  mapfile -t rows < <(python3 - "$SOURCE_MAP" <<'PY'
import json
import pathlib
import re
import sys

path = pathlib.Path(sys.argv[1])
try:
    document = json.loads(path.read_text(encoding="utf-8"))
except (OSError, UnicodeError, json.JSONDecodeError) as error:
    raise SystemExit(f"error: invalid release source map {path}: {error}")

if document.get("schema_version") != 1 or document.get("project") != "kanban-tool":
    raise SystemExit("error: release source map has the wrong project or schema version")
if document.get("integration_strategy") != "semantic-port-without-merge-base":
    raise SystemExit("error: release source map must record the no-merge-base semantic-port strategy")
if document.get("source_ref") != "refs/heads/derived-projection-v2":
    raise SystemExit("error: release source map has the wrong exact source ref")

object_id = re.compile(r"^[0-9a-f]{40}$")
source_tip = document.get("source_tip")
if not isinstance(source_tip, str) or not object_id.fullmatch(source_tip):
    raise SystemExit("error: release source map has an invalid exact source tip")

mappings = document.get("mappings")
if not isinstance(mappings, list) or len(mappings) != 3:
    raise SystemExit("error: release source map must contain exactly three source slices")

required_sources = [
    "0e58068fc67913495d8566e0a839e7a068456f81",
    "85e1f797c2d5c2b089d1a2f29827f42e25cd595c",
    "c764706fcae214f58a3a65f5dc565135522bbd81",
]
sources: list[str] = []
destinations: set[str] = set()
for index, mapping in enumerate(mappings):
    if not isinstance(mapping, dict):
        raise SystemExit(f"error: release source map mapping {index} is not an object")
    source = mapping.get("source_commit")
    destination = mapping.get("integrated_commit")
    if not isinstance(source, str) or not object_id.fullmatch(source):
        raise SystemExit(f"error: release source map mapping {index} has an invalid source commit")
    if not isinstance(destination, str) or not object_id.fullmatch(destination):
        raise SystemExit(f"error: release source map mapping {index} has an invalid integrated commit")
    if mapping.get("source_branch") != "origin/derived-projection-v2":
        raise SystemExit(f"error: release source map mapping {index} has the wrong source branch")
    subject = mapping.get("subject")
    if not isinstance(subject, str) or not subject.strip():
        raise SystemExit(f"error: release source map mapping {index} has no subject")
    if destination in destinations:
        raise SystemExit("error: release source map contains duplicate integrated commits")
    sources.append(source)
    destinations.add(destination)

if sources != required_sources:
    raise SystemExit("error: release source map does not contain the three canonical source slices")
if source_tip != required_sources[-1]:
    raise SystemExit("error: release source map tip is not the final canonical source slice")

print(f"META\t{document['source_ref']}\t{source_tip}")
for mapping in mappings:
    print(f"MAP\t{mapping['source_commit']}\t{mapping['integrated_commit']}")
PY
  ) || exit 1

  [[ "${#rows[@]}" -eq 4 ]] ||
    fail "release source map validator emitted an unexpected mapping count"
  IFS=$'\t' read -r kind SOURCE_REF SOURCE_TIP <<<"${rows[0]}"
  [[ "$kind" == "META" ]] || fail "release source map validator omitted metadata"
  require_sha1 "source map tip" "$SOURCE_TIP"

  SOURCE_COMMITS=()
  INTEGRATED_COMMITS=()
  for row in "${rows[@]:1}"; do
    IFS=$'\t' read -r kind source integrated <<<"$row"
    [[ "$kind" == "MAP" ]] || fail "release source map validator emitted an invalid row"
    SOURCE_COMMITS+=("$source")
    INTEGRATED_COMMITS+=("$integrated")
  done
}

workspace_version() {
  python3 - "$ROOT/Cargo.toml" <<'PY'
import pathlib
import sys
import tomllib

path = pathlib.Path(sys.argv[1])
try:
    with path.open("rb") as handle:
        version = tomllib.load(handle)["workspace"]["package"]["version"]
except (OSError, KeyError, TypeError, tomllib.TOMLDecodeError) as error:
    raise SystemExit(f"error: cannot read workspace.package.version from {path}: {error}")
if not isinstance(version, str):
    raise SystemExit(f"error: workspace.package.version is not a string: {version!r}")
print(version)
PY
}

sha256_file() {
  local path="$1" digest
  digest="$(sha256sum "$path" | awk '{print $1}')" ||
    fail "cannot hash release identity file: $path"
  [[ "$digest" =~ ^[0-9a-f]{64}$ ]] ||
    fail "release identity file hash is not a lowercase SHA-256: $path"
  printf '%s\n' "$digest"
}

release_architecture() {
  local target="$1" arch="${target%%-*}"
  case "$arch" in
    x86_64) printf 'amd64\n' ;;
    aarch64) printf 'arm64\n' ;;
    armv7*|arm) printf 'armhf\n' ;;
    i686|i586) printf 'i386\n' ;;
    *) fail "unsupported Debian architecture for target triple: $target" ;;
  esac
}

capture_release_identity() {
  local lock_hash closure_hash rustc_output cargo_output platform machine target deb_arch
  local feature_csv no_default

  [[ -f "$LOCKFILE" && ! -L "$LOCKFILE" ]] ||
    fail "release identity Cargo.lock is missing or unsafe"
  python3 "$SAFE_PATH" validate-file --root "$ROOT" --path "$LOCKFILE"
  [[ -f "$REGISTRY_CLOSURE" && ! -L "$REGISTRY_CLOSURE" ]] ||
    fail "release identity registry closure is missing or unsafe"
  python3 "$SAFE_PATH" validate-file --root "$ROOT" --path "$REGISTRY_CLOSURE"
  lock_hash="$(sha256_file "$LOCKFILE")"
  closure_hash="$(sha256_file "$REGISTRY_CLOSURE")"

  command -v rustc >/dev/null 2>&1 || fail "rustc is required for release identity"
  command -v cargo >/dev/null 2>&1 || fail "cargo is required for release identity"
  command -v uname >/dev/null 2>&1 || fail "uname is required for release identity"
  rustc_output="$(rustc -vV)" || fail "rustc -vV failed while capturing release identity"
  cargo_output="$(cargo --version)" || fail "cargo --version failed while capturing release identity"
  platform="$(uname -s)" || fail "uname -s failed while capturing release identity"
  machine="$(uname -m)" || fail "uname -m failed while capturing release identity"
  target="$(awk '/^host:[[:space:]]+/ { print $2; count += 1 } END { if (count != 1) exit 1 }' <<<"$rustc_output")" ||
    fail "rustc -vV must contain exactly one host target"
  [[ "$target" =~ ^[A-Za-z0-9][A-Za-z0-9_.-]*$ ]] ||
    fail "rustc host target is invalid: $target"
  deb_arch="$(release_architecture "$target")"

  feature_csv="$RELEASE_FEATURES_DEFAULT"
  no_default="$RELEASE_NO_DEFAULT_FEATURES_DEFAULT"
  if [[ "${KANBAN_RELEASE_FEATURES+x}" == "x" ]]; then
    [[ "$KANBAN_RELEASE_FEATURES" == "$feature_csv" ]] ||
      fail "KANBAN_RELEASE_FEATURES must be the canonical release feature set"
  fi
  if [[ "${KANBAN_RELEASE_NO_DEFAULT_FEATURES+x}" == "x" ]]; then
    [[ "$KANBAN_RELEASE_NO_DEFAULT_FEATURES" == "$no_default" ]] ||
      fail "KANBAN_RELEASE_NO_DEFAULT_FEATURES must remain enabled for release"
  fi

  local -a identity_lines
  mapfile -t identity_lines < <(
    LOCK_HASH="$lock_hash" \
    CLOSURE_HASH="$closure_hash" \
    RUSTC_VV="$rustc_output" \
    CARGO_VERSION="$cargo_output" \
    PLATFORM="$platform" \
    MACHINE_ARCH="$machine" \
    TARGET_TRIPLE="$target" \
    DEB_ARCH="$deb_arch" \
    FEATURE_CSV="$feature_csv" \
    NO_DEFAULT_FEATURES="$no_default" \
      python3 - <<'PY'
import hashlib
import json
import os
import re

def required(name: str) -> str:
    value = os.environ.get(name, "")
    if not value or "\x00" in value:
        raise SystemExit(f"error: release identity {name} is empty or contains NUL")
    # Tool output is evidence, not a place to smuggle local paths or secrets.
    # The real rustc/cargo version commands are path-free; reject absolute
    # path-looking output so a future toolchain cannot leak cache locations.
    if any(token.startswith("/") for token in re.split(r"[\s=()]", value) if token):
        raise SystemExit(f"error: release identity {name} contains an absolute path")
    return value

lock_hash = required("LOCK_HASH")
closure_hash = required("CLOSURE_HASH")
rustc_vv = required("RUSTC_VV").rstrip("\n")
cargo_version = required("CARGO_VERSION").rstrip("\n")
platform = required("PLATFORM")
machine_arch = required("MACHINE_ARCH")
target_triple = required("TARGET_TRIPLE")
deb_arch = required("DEB_ARCH")
feature_csv = os.environ.get("FEATURE_CSV", "")
no_default_raw = os.environ.get("NO_DEFAULT_FEATURES", "")
if no_default_raw not in {"0", "1"}:
    raise SystemExit("error: release identity default-feature mode is invalid")
if platform != "Linux":
    raise SystemExit(f"error: release Debian cohort requires Linux, got platform {platform!r}")
if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", machine_arch):
    raise SystemExit("error: release machine architecture is invalid")
if feature_csv != "tantivy-backend,oxigraph-backend":
    raise SystemExit("error: release feature set is not the canonical release set")
features = feature_csv.split(",")
if any(not re.fullmatch(r"[a-z0-9][a-z0-9_-]*", feature) for feature in features):
    raise SystemExit("error: effective release feature set contains an invalid feature")
if len(set(features)) != len(features):
    raise SystemExit("error: effective release feature set contains duplicates")
features = sorted(features)
if features != ["oxigraph-backend", "tantivy-backend"]:
    raise SystemExit("error: effective release feature set is not the canonical release set")
for label, digest in (("Cargo.lock", lock_hash), ("registry closure", closure_hash)):
    if not re.fullmatch(r"[0-9a-f]{64}", digest):
        raise SystemExit(f"error: {label} hash is not a lowercase SHA-256")
if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", target_triple):
    raise SystemExit("error: release target triple is invalid")
if not re.fullmatch(r"[a-z0-9][a-z0-9_-]*", deb_arch):
    raise SystemExit("error: release Debian architecture is invalid")

payload = {
    "cargo_lock": {"path": "Cargo.lock", "sha256": lock_hash},
    "features": {
        "effective": features,
        "no_default_features": no_default_raw == "1",
    },
    "registry_closure": {
        "path": "policy/schema-tool-registry-closure.json",
        "sha256": closure_hash,
    },
    "target": {
        "deb_arch": deb_arch,
        "machine_arch": machine_arch,
        "platform": platform,
        "triple": target_triple,
    },
    "toolchain": {
        "cargo_version": cargo_version,
        "cargo_version_sha256": hashlib.sha256(cargo_version.encode()).hexdigest(),
        "rustc_vv": rustc_vv,
        "rustc_vv_sha256": hashlib.sha256(rustc_vv.encode()).hexdigest(),
    },
}
canonical = json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
identity_sha256 = hashlib.sha256(canonical.encode()).hexdigest()
identity = dict(payload)
identity["identity_sha256"] = identity_sha256
print(json.dumps(identity, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
print(identity_sha256)
PY
  )
  [[ "${#identity_lines[@]}" -eq 2 ]] ||
    fail "release identity probe emitted an unexpected result"
  RELEASE_IDENTITY_JSON="${identity_lines[0]}"
  IDENTITY_SHA256="${identity_lines[1]}"
  [[ "$IDENTITY_SHA256" =~ ^[0-9a-f]{64}$ ]] ||
    fail "release identity hash is invalid"
  GENERATION_KEY="$COMMIT-$TREE-$IDENTITY_SHA256"
}

remote_tip() {
  local ref="$1" label="$2" output
  local -a lines
  output="$(git -C "$ROOT" ls-remote --exit-code origin "$ref")" ||
    fail "cannot resolve $label with git ls-remote"
  mapfile -t lines <<<"$output"
  [[ "${#lines[@]}" -eq 1 ]] ||
    fail "$label must resolve to exactly one remote ref"
  read -r RESOLVED_REMOTE_TIP RESOLVED_REMOTE_REF RESOLVED_REMOTE_EXTRA <<<"${lines[0]}"
  [[ "$RESOLVED_REMOTE_REF" == "$ref" && -z "${RESOLVED_REMOTE_EXTRA:-}" ]] ||
    fail "git ls-remote returned an unexpected $label record"
  require_sha1 "$label commit" "$RESOLVED_REMOTE_TIP"
}

capture_source_state() {
  local git_root branch status commit tree version saved_source_tip resolved
  local merge_base_output merge_base_status index

  git_root="$(git -C "$ROOT" rev-parse --show-toplevel)"
  git_root="$(cd "$git_root" && pwd -P)"
  [[ "$git_root" == "$ROOT" ]] ||
    fail "release script root is not the active Git root: script=$ROOT git=$git_root"

  branch="$(git -C "$ROOT" symbolic-ref --quiet HEAD)" ||
    fail "release requires a symbolic main branch; detached HEAD is not allowed"
  [[ "$branch" == "refs/heads/main" ]] ||
    fail "release requires symbolic main, got: $branch"

  status="$(git -C "$ROOT" status --porcelain=v1 --untracked-files=all)"
  [[ -z "$status" ]] ||
    fail "release requires a clean tree including untracked files"

  commit="$(git -C "$ROOT" rev-parse --verify 'HEAD^{commit}')"
  tree="$(git -C "$ROOT" rev-parse --verify 'HEAD^{tree}')"
  require_sha1 "HEAD commit" "$commit"
  require_sha1 "HEAD tree" "$tree"

  remote_tip "refs/heads/main" "origin/main"
  REMOTE_MAIN_TIP="$RESOLVED_REMOTE_TIP"
  [[ "$REMOTE_MAIN_TIP" == "$commit" ]] ||
    fail "HEAD does not exactly match origin/main: HEAD=$commit origin/main=$REMOTE_MAIN_TIP"

  version="$(workspace_version)"
  [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.+-][0-9A-Za-z.-]+)?$ ]] ||
    fail "workspace version cannot form a release cohort identity: $version"

  validate_source_map
  remote_tip "$SOURCE_REF" "origin/derived-projection-v2"
  REMOTE_SOURCE_TIP="$RESOLVED_REMOTE_TIP"
  [[ "$REMOTE_SOURCE_TIP" == "$SOURCE_TIP" ]] ||
    fail "live origin/derived-projection-v2 tip does not match the canonical source map"

  saved_source_tip="$(
    git -C "$ROOT" rev-parse --verify "$SAVED_SOURCE_REF^{commit}"
  )" || fail "cannot resolve saved origin/derived-projection-v2 tracking ref"
  require_sha1 "saved origin/derived-projection-v2 tip" "$saved_source_tip"
  [[ "$saved_source_tip" == "$SOURCE_TIP" ]] ||
    fail "saved origin/derived-projection-v2 tip does not match the canonical source map"

  for index in "${!SOURCE_COMMITS[@]}"; do
    resolved="$(
      git -C "$ROOT" rev-parse --verify "${SOURCE_COMMITS[$index]}^{commit}"
    )"
    [[ "$resolved" == "${SOURCE_COMMITS[$index]}" ]] ||
      fail "release source object does not resolve exactly: ${SOURCE_COMMITS[$index]}"
    git -C "$ROOT" merge-base --is-ancestor \
      "${SOURCE_COMMITS[$index]}" "$SOURCE_TIP" ||
      fail "source slice is not an ancestor of the exact saved source tip: ${SOURCE_COMMITS[$index]}"

    resolved="$(
      git -C "$ROOT" rev-parse --verify "${INTEGRATED_COMMITS[$index]}^{commit}"
    )"
    [[ "$resolved" == "${INTEGRATED_COMMITS[$index]}" ]] ||
      fail "release integrated object does not resolve exactly: ${INTEGRATED_COMMITS[$index]}"
    git -C "$ROOT" merge-base --is-ancestor "${INTEGRATED_COMMITS[$index]}" HEAD ||
      fail "release source map integrated commit is not an ancestor of HEAD: ${INTEGRATED_COMMITS[$index]}"
  done

  set +e
  merge_base_output="$(git -C "$ROOT" merge-base HEAD "$SOURCE_TIP" 2>&1)"
  merge_base_status=$?
  set -e
  [[ "$merge_base_status" -eq 1 && -z "$merge_base_output" ]] ||
    fail "main and saved origin/derived-projection-v2 must have no merge base"

  SOURCE_MAP_SHA256="$(sha256sum "$SOURCE_MAP" | awk '{print $1}')"
  VERSION="$version"
  COMMIT="$commit"
  TREE="$tree"
  SAVED_SOURCE_TIP="$saved_source_tip"
  capture_release_identity
  BUILD_ID="kanban-tool/$VERSION;commit=$COMMIT;tree=$TREE;identity=$IDENTITY_SHA256"
}

validate_source_manifest() {
  local manifest="$1"
  python3 - "$manifest" <<'PY'
import hashlib
import json
import pathlib
import re
import sys

path = pathlib.Path(sys.argv[1])
try:
    document = json.loads(path.read_text(encoding="utf-8"))
except (OSError, UnicodeError, json.JSONDecodeError) as error:
    raise SystemExit(f"error: invalid source provenance manifest: {error}")

def exact(value, keys, label):
    if not isinstance(value, dict) or set(value) != set(keys):
        raise SystemExit(f"error: {label} has missing or extra fields")

def digest(value, label):
    if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
        raise SystemExit(f"error: {label} must be a lowercase SHA-256")

def object_id(value, label):
    if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{40}", value):
        raise SystemExit(f"error: {label} must be one exact Git object id")

expected_top = {
    "branch", "build_id", "commit", "generation_key", "identity",
    "identity_sha256", "project", "remote", "schema_version",
    "semantic_source", "source_map", "tree", "version",
}
exact(document, expected_top, "source provenance manifest")
if document["project"] != "kanban-tool" or document["schema_version"] != 3:
    raise SystemExit("error: source provenance manifest has the wrong project or schema version")
if document["branch"] != "main":
    raise SystemExit("error: source provenance manifest has the wrong branch")
object_id(document["commit"], "source provenance commit")
object_id(document["tree"], "source provenance tree")
if not isinstance(document["version"], str) or not re.fullmatch(
    r"[0-9]+\.[0-9]+\.[0-9]+([.+-][0-9A-Za-z.-]+)?", document["version"]
):
    raise SystemExit("error: source provenance version is invalid")
if document["generation_key"] != (
    f"{document['commit']}-{document['tree']}-{document['identity_sha256']}"
):
    raise SystemExit("error: source provenance generation key is not canonical")
digest(document["identity_sha256"], "source provenance identity")
if document["build_id"] != (
    f"kanban-tool/{document['version']};commit={document['commit']};"
    f"tree={document['tree']};identity={document['identity_sha256']}"
):
    raise SystemExit("error: source provenance build identity is not canonical")

identity = document["identity"]
exact(
    identity,
    {"cargo_lock", "features", "identity_sha256", "registry_closure", "target", "toolchain"},
    "release identity",
)
if identity["identity_sha256"] != document["identity_sha256"]:
    raise SystemExit("error: source provenance identity hash does not match identity")
identity_payload = dict(identity)
del identity_payload["identity_sha256"]
canonical_identity = json.dumps(
    identity_payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")
)
if hashlib.sha256(canonical_identity.encode()).hexdigest() != identity["identity_sha256"]:
    raise SystemExit("error: source provenance identity hash is not canonical")

for field, expected_path in (
    ("cargo_lock", "Cargo.lock"),
    ("registry_closure", "policy/schema-tool-registry-closure.json"),
):
    entry = identity[field]
    exact(entry, {"path", "sha256"}, f"release identity {field}")
    if entry["path"] != expected_path:
        raise SystemExit(f"error: release identity {field} path is not canonical")
    digest(entry["sha256"], f"release identity {field}")

features = identity["features"]
exact(features, {"effective", "no_default_features"}, "release identity features")
if not isinstance(features["effective"], list) or not features["effective"]:
    raise SystemExit("error: release identity feature set is empty")
if any(
    not isinstance(feature, str) or not re.fullmatch(r"[a-z0-9][a-z0-9_-]*", feature)
    for feature in features["effective"]
):
    raise SystemExit("error: release identity feature set is invalid")
if len(set(features["effective"])) != len(features["effective"]) or features["effective"] != sorted(features["effective"]):
    raise SystemExit("error: release identity feature set is not canonical")
if features["effective"] != ["oxigraph-backend", "tantivy-backend"]:
    raise SystemExit("error: release identity feature set is not the canonical release set")
if features["no_default_features"] is not True:
    raise SystemExit("error: release identity default-feature mode is invalid")

target = identity["target"]
exact(target, {"deb_arch", "machine_arch", "platform", "triple"}, "release identity target")
if target["platform"] != "Linux":
    raise SystemExit("error: release identity platform is not Linux")
for field, pattern in (
    ("deb_arch", r"[a-z0-9][a-z0-9_-]*"),
    ("machine_arch", r"[A-Za-z0-9][A-Za-z0-9_.-]*"),
    ("triple", r"[A-Za-z0-9][A-Za-z0-9_.-]*"),
):
    if not isinstance(target[field], str) or not re.fullmatch(pattern, target[field]):
        raise SystemExit(f"error: release identity target {field} is invalid")

toolchain = identity["toolchain"]
exact(
    toolchain,
    {"cargo_version", "cargo_version_sha256", "rustc_vv", "rustc_vv_sha256"},
    "release identity toolchain",
)
for field in ("cargo_version", "rustc_vv"):
    value = toolchain[field]
    if not isinstance(value, str) or not value or "\x00" in value:
        raise SystemExit(f"error: release identity toolchain {field} is invalid")
    if any(token.startswith("/") for token in re.split(r"[\s=()]", value) if token):
        raise SystemExit(f"error: release identity toolchain {field} leaks an absolute path")
for value_field, hash_field in (
    ("cargo_version", "cargo_version_sha256"),
    ("rustc_vv", "rustc_vv_sha256"),
):
    digest(toolchain[hash_field], f"release identity toolchain {hash_field}")
    if hashlib.sha256(toolchain[value_field].encode()).hexdigest() != toolchain[hash_field]:
        raise SystemExit(f"error: release identity toolchain {value_field} hash drifted")

remote = document["remote"]
exact(remote, {"commit", "name", "ref"}, "source provenance remote")
if remote["name"] != "origin" or remote["ref"] != "refs/heads/main":
    raise SystemExit("error: source provenance remote is not canonical")
object_id(remote["commit"], "source provenance remote commit")
semantic = document["semantic_source"]
exact(
    semantic,
    {"name", "no_merge_base_with_main", "remote_ref", "remote_tip", "saved_ref", "saved_tip", "verified_source_commits"},
    "source provenance semantic source",
)
if (
    semantic["name"] != "origin/derived-projection-v2"
    or semantic["remote_ref"] != "refs/heads/derived-projection-v2"
    or semantic["saved_ref"] != "refs/remotes/origin/derived-projection-v2"
    or semantic["no_merge_base_with_main"] is not True
):
    raise SystemExit("error: source provenance semantic source is not canonical")
object_id(semantic["remote_tip"], "semantic source remote tip")
object_id(semantic["saved_tip"], "semantic source saved tip")
if (
    not isinstance(semantic["verified_source_commits"], list)
    or len(semantic["verified_source_commits"]) != 3
):
    raise SystemExit("error: source provenance source commit list is invalid")
for index, value in enumerate(semantic["verified_source_commits"]):
    object_id(value, f"semantic source commit {index}")

source_map = document["source_map"]
exact(source_map, {"path", "sha256"}, "source provenance source map")
if source_map["path"] != "docs/release/derived-projection-v2-source-map.json":
    raise SystemExit("error: source provenance source map path is not canonical")
digest(source_map["sha256"], "source provenance source map")

def reject_absolute(value):
    if isinstance(value, str) and value.startswith("/"):
        raise SystemExit("error: source provenance contains an absolute path")
    if isinstance(value, dict):
        for child in value.values():
            reject_absolute(child)
    elif isinstance(value, list):
        for child in value:
            reject_absolute(child)

reject_absolute(document)
PY
}

write_manifest() (
  set -euo pipefail
  local output="$1" output_dir canonical_output_dir temporary temporary_dir
  local temporary_dev temporary_ino
  output_dir="$(dirname "$output")"
  [[ -d "$output_dir" ]] ||
    fail "release source manifest output directory must already exist: $output_dir"
  canonical_output_dir="$(cd "$output_dir" && pwd -P)"
  case "$canonical_output_dir/" in
    "$ROOT/"*) fail "release source gate refuses to write inside the source tree" ;;
  esac

  temporary_dir="$(
    python3 "$SAFE_PATH" private-dir --root "$canonical_output_dir" \
      --parent "$canonical_output_dir" --prefix ".source-provenance."
  )"
  read -r temporary_dev temporary_ino < <(
    python3 "$SAFE_PATH" dir-identity --root "$canonical_output_dir" \
      --path "$temporary_dir"
  )
  temporary="$temporary_dir/source-provenance.json"
  cleanup_manifest_stage() {
    python3 "$SAFE_PATH" remove-tree --root "$canonical_output_dir" \
      --path "$temporary_dir" --expected-dev "$temporary_dev" \
      --expected-ino "$temporary_ino" ||
      printf 'warning: retained unverified source manifest stage: %s\n' \
        "$temporary_dir" >&2
  }
  trap cleanup_manifest_stage EXIT
  RELEASE_IDENTITY_JSON="$RELEASE_IDENTITY_JSON" \
  IDENTITY_SHA256="$IDENTITY_SHA256" \
  GENERATION_KEY="$GENERATION_KEY" \
  python3 - "$temporary" "$VERSION" "$COMMIT" "$TREE" "$REMOTE_MAIN_TIP" \
    "$BUILD_ID" "$SOURCE_MAP_REL" "$SOURCE_MAP_SHA256" "$SOURCE_REF" \
    "$REMOTE_SOURCE_TIP" "$SAVED_SOURCE_REF" "$SAVED_SOURCE_TIP" \
    "${SOURCE_COMMITS[@]}" <<'PY'
import json
import os
import pathlib
import sys

(
    output,
    version,
    commit,
    tree,
    remote_main_tip,
    build_id,
    source_map_path,
    source_map_sha256,
    source_ref,
    remote_source_tip,
    saved_source_ref,
    saved_source_tip,
    *source_commits,
) = sys.argv[1:]
identity = json.loads(os.environ["RELEASE_IDENTITY_JSON"])
if identity.get("identity_sha256") != os.environ["IDENTITY_SHA256"]:
    raise SystemExit("error: release identity hash is not self-consistent")
document = {
    "branch": "main",
    "build_id": build_id,
    "commit": commit,
    "generation_key": os.environ["GENERATION_KEY"],
    "identity": identity,
    "identity_sha256": os.environ["IDENTITY_SHA256"],
    "project": "kanban-tool",
    "remote": {
        "commit": remote_main_tip,
        "name": "origin",
        "ref": "refs/heads/main",
    },
    "schema_version": 3,
    "semantic_source": {
        "name": "origin/derived-projection-v2",
        "no_merge_base_with_main": True,
        "remote_ref": source_ref,
        "remote_tip": remote_source_tip,
        "saved_ref": saved_source_ref,
        "saved_tip": saved_source_tip,
        "verified_source_commits": source_commits,
    },
    "source_map": {
        "path": source_map_path,
        "sha256": source_map_sha256,
    },
    "tree": tree,
    "version": version,
}
pathlib.Path(output).write_text(
    json.dumps(document, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n",
    encoding="utf-8",
)
PY
  chmod 0644 "$temporary"
  python3 "$SAFE_PATH" publish-file \
    --root "$canonical_output_dir" \
    --source "$temporary" \
    --destination "$canonical_output_dir/$(basename "$output")" \
    --replace
)

prepare() {
  local output="$1"
  [[ "$output" == /* ]] || fail "release source manifest output path must be absolute"
  capture_source_state
  write_manifest "$output"
}

verify() (
  set -euo pipefail
  local manifest="$1" manifest_dir temporary_dir temporary
  local temporary_parent temporary_dev temporary_ino
  local pinned_fd="${KANBAN_RELEASE_PINNED_STAGE_FD:-}"
  [[ -f "$manifest" && ! -L "$manifest" ]] ||
    if [[ -z "$pinned_fd" || ! -f "$manifest" ]]; then
      fail "release source manifest is missing or not a regular file: $manifest"
    fi
  [[ "$manifest" == /* ]] ||
    fail "release source manifest path must be absolute"
  if [[ -n "$pinned_fd" ]]; then
    [[ "$pinned_fd" =~ ^[0-9]+$ &&
      "$manifest" == "/proc/self/fd/$pinned_fd/source-provenance.json" ]] ||
      fail "final source verifier is not bound to the inherited stage fd"
    python3 "$SAFE_PATH" validate-tree-file --tree-fd "$pinned_fd" \
      --relative source-provenance.json
  else
    manifest_dir="$(cd "$(dirname "$manifest")" && pwd -P)"
    python3 "$SAFE_PATH" validate-file --root "$manifest_dir" --path "$manifest"
  fi
  validate_source_manifest "$manifest"
  temporary_dir="$(mktemp -d)"
  temporary_dir="$(cd "$temporary_dir" && pwd -P)"
  temporary_parent="$(dirname "$temporary_dir")"
  read -r temporary_dev temporary_ino < <(
    python3 "$SAFE_PATH" dir-identity --root "$temporary_parent" \
      --path "$temporary_dir"
  )
  cleanup_temporary_dir() {
    python3 "$SAFE_PATH" remove-tree --root "$temporary_parent" \
      --path "$temporary_dir" --expected-dev "$temporary_dev" \
      --expected-ino "$temporary_ino" ||
      printf 'warning: retained unverified source-gate temp tree: %s\n' \
        "$temporary_dir" >&2
  }
  trap cleanup_temporary_dir EXIT
  temporary="$temporary_dir/source-provenance.json"
  capture_source_state
  write_manifest "$temporary"
  cmp -s "$manifest" "$temporary" ||
    fail "release source state drifted after the cohort manifest was created"
)

case "${1:-}" in
  prepare)
    [[ "${2:-}" == "--output" && $# -eq 3 ]] || { usage >&2; exit 2; }
    prepare "$3"
    ;;
  verify)
    [[ "${2:-}" == "--manifest" && $# -eq 3 ]] || { usage >&2; exit 2; }
    verify "$3"
    ;;
  validate)
    [[ "${2:-}" == "--manifest" && $# -eq 3 ]] || { usage >&2; exit 2; }
    [[ "$3" == /* && -f "$3" && ! -L "$3" ]] ||
      fail "release source manifest is missing or unsafe: $3"
    if [[ -n "${KANBAN_RELEASE_PINNED_STAGE_FD:-}" ]]; then
      [[ "${KANBAN_RELEASE_PINNED_STAGE_FD}" =~ ^[0-9]+$ &&
        "$3" == "/proc/self/fd/${KANBAN_RELEASE_PINNED_STAGE_FD}/source-provenance.json" ]] ||
        fail "release source manifest is not bound to the inherited stage fd"
      python3 "$SAFE_PATH" validate-tree-file \
        --tree-fd "$KANBAN_RELEASE_PINNED_STAGE_FD" --relative source-provenance.json
    else
      manifest_dir="$(cd "$(dirname "$3")" && pwd -P)"
      python3 "$SAFE_PATH" validate-file --root "$manifest_dir" --path "$3"
    fi
    validate_source_manifest "$3"
    ;;
  -h|--help)
    usage
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac
