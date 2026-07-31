#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
SAFE_PATH="$ROOT/scripts/release-safe-path.py"
# The artifact verifier may be executed from an immutable source snapshot.  A
# snapshot is intentionally not a Git checkout, so source-state verification
# remains delegated to the pinned gate beside this sealed script.  The cohort
# wrapper points that gate at the live evidence root separately.
SOURCE_GATE="$ROOT/scripts/release-source-gate.sh"

usage() {
  cat <<'EOF'
Usage:
  scripts/release-artifact-manifest.sh prepare \
    --source-manifest <source-provenance.json> \
    --stage-dir <private-cohort-directory> \
    --published-dir <final-cohort-directory> \
    --output <release-artifacts.pending.json>

  scripts/release-artifact-manifest.sh verify \
    --manifest <release-artifacts.pending.json> \
    --stage-dir <private-cohort-directory>

  scripts/release-artifact-manifest.sh verify-final \
    --manifest <release-artifacts.json> \
    --stage-dir <private-cohort-directory>

prepare copies every artifact into one private same-filesystem generation before
hashing. verify rehashes those stable copies. verify-final is invoked only by
release-safe-path.py while its kernel lease transaction is active; it rechecks
both live source state and staged artifact semantics before and after rename.
EOF
}

fail() {
  echo "error: $*" >&2
  exit 1
}

COMMAND="${1:-}"
[[ -n "$COMMAND" ]] || { usage >&2; exit 2; }
shift
SOURCE_MANIFEST=""
STAGE_DIR=""
PUBLISHED_DIR=""
OUTPUT=""
MANIFEST=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --source-manifest)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      SOURCE_MANIFEST="$2"
      shift 2
      ;;
    --stage-dir)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      STAGE_DIR="$2"
      shift 2
      ;;
    --published-dir)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      PUBLISHED_DIR="$2"
      shift 2
      ;;
    --output)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      OUTPUT="$2"
      shift 2
      ;;
    --manifest)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      MANIFEST="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
done

TARGET_ROOT="$("$LOCK" --print-target-dir)"
[[ "$TARGET_ROOT" == /* && -d "$TARGET_ROOT" ]] ||
  fail "Cargo target root must be an existing absolute directory"
TARGET_ROOT="$(cd "$TARGET_ROOT" && pwd -P)"
case "$TARGET_ROOT/" in
  "$ROOT/"*) fail "artifact manifest refuses a Cargo target root inside the source tree" ;;
esac

if [[ "$COMMAND" == "verify-final" ]]; then
  PUBLISH_PHASE="${KANBAN_RELEASE_PUBLISH_PHASE:-}"
  PUBLISH_SOURCE="${KANBAN_RELEASE_PUBLISH_SOURCE:-}"
  PUBLISH_DESTINATION="${KANBAN_RELEASE_PUBLISH_DESTINATION:-}"
  PINNED_STAGE_FD="${KANBAN_RELEASE_PINNED_STAGE_FD:-}"
  PINNED_STAGE_DEV="${KANBAN_RELEASE_PINNED_STAGE_DEV:-}"
  PINNED_STAGE_INO="${KANBAN_RELEASE_PINNED_STAGE_INO:-}"
  [[ "$PUBLISH_SOURCE" == /* && "$PUBLISH_DESTINATION" == /* ]] ||
    fail "verify-final requires anchored source/destination publication paths"
  [[ "$PUBLISH_PHASE" == "pre" || "$PUBLISH_PHASE" == "post" ||
    "$PUBLISH_PHASE" == "resume" ]] ||
    fail "verify-final requires a pre/post/resume lease publication phase"
  [[ "$PINNED_STAGE_FD" =~ ^[0-9]+$ &&
    "$PINNED_STAGE_DEV" =~ ^[0-9]+$ && "$PINNED_STAGE_INO" =~ ^[0-9]+$ ]] ||
    fail "verify-final requires an inherited pinned stage fd identity"
  python3 - "$PINNED_STAGE_FD" "$PINNED_STAGE_DEV" "$PINNED_STAGE_INO" <<'PY'
import os
import stat
import sys

descriptor, expected_dev, expected_ino = map(int, sys.argv[1:])
metadata = os.fstat(descriptor)
if (
    not stat.S_ISDIR(metadata.st_mode)
    or (metadata.st_dev, metadata.st_ino) != (expected_dev, expected_ino)
):
    raise SystemExit("error: inherited pinned stage fd identity drifted")
PY
  STAGE_DIR="/proc/self/fd/$PINNED_STAGE_FD"
  MANIFEST="$STAGE_DIR/release-artifacts.json"
fi

canonical_under_target() {
  python3 - "$TARGET_ROOT" "$1" <<'PY'
import os
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
path = pathlib.Path(sys.argv[2])
if not path.is_absolute():
    raise SystemExit(f"error: release path must be absolute: {path}")
if ".." in path.parts:
    raise SystemExit(f"error: release path contains parent traversal: {path}")
if os.path.commonpath((os.fspath(root), os.fspath(path))) != os.fspath(root):
    raise SystemExit(f"error: release path escapes target root: {path}")
print(path)
PY
}

if [[ "$COMMAND" == "verify-final" ]]; then
  [[ -d "$STAGE_DIR" ]] || fail "inherited pinned cohort stage is unavailable"
  STAGE_MODE="$(stat -Lc '%a' "$STAGE_DIR")"
else
  STAGE_DIR="$(canonical_under_target "$STAGE_DIR")"
  [[ -d "$STAGE_DIR" && ! -L "$STAGE_DIR" ]] ||
    fail "private cohort stage is missing or unsafe"
  python3 "$SAFE_PATH" ensure-dir --root "$TARGET_ROOT" --path "$STAGE_DIR" --mode 0700
  STAGE_MODE="$(stat -c '%a' "$STAGE_DIR")"
fi
if [[ "$COMMAND" == "prepare" ]]; then
  [[ "$STAGE_MODE" == "700" ]] ||
    fail "private cohort stage must have mode 0700 while preparing artifacts"
else
  [[ "$STAGE_MODE" == "700" || "$STAGE_MODE" == "555" ]] ||
    fail "cohort stage must be private or sealed while verifying artifacts"
fi

validate_staged_file() {
  local path="$1"
  if [[ "$COMMAND" == "verify-final" ]]; then
    [[ "$path" == "$STAGE_DIR/"* ]] ||
      fail "final verifier file is outside the inherited pinned stage"
    python3 "$SAFE_PATH" validate-tree-file --tree-fd "$PINNED_STAGE_FD" \
      --relative "${path#"$STAGE_DIR/"}"
  else
    python3 "$SAFE_PATH" validate-file --root "$TARGET_ROOT" --path "$path"
  fi
}

verify_helper_identity() {
  local helper="$1"
  python3 - "$helper" "${KANBAN_BUILD_ID:?}" <<'PY'
import pathlib
import os
import subprocess
import sys

helper = pathlib.Path(sys.argv[1])
expected = sys.argv[2].encode()
pinned_fd = os.environ.get("KANBAN_RELEASE_PINNED_STAGE_FD")
pass_fds = (int(pinned_fd),) if pinned_fd is not None else ()
result = subprocess.run(
    [helper, "__build-identity"],
    check=False,
    close_fds=True,
    pass_fds=pass_fds,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
)
if result.returncode != 0:
    raise SystemExit(
        f"error: helper build-identity command failed ({result.returncode}): {helper}"
    )
if result.stderr:
    raise SystemExit(f"error: helper build-identity command wrote stderr: {helper}")
if result.stdout != expected:
    raise SystemExit(f"error: helper runtime identity does not exactly match KANBAN_BUILD_ID: {helper}")
PY
}

verify_staged_cohort() (
  set -euo pipefail
  local stage="$1" source_manifest="$2" source_map="$3"
  local artifacts="$stage/artifacts" unpacked unpacked_parent unpacked_dev unpacked_ino
  local cli_root desktop_root binary

  for binary in \
    "$artifacts/bin/kanban" \
    "$artifacts/bin/kanban-vector-lancedb" \
    "$artifacts/bin/kanban-graph-oxigraph" \
    "$artifacts/bin/kanban-desktop"
  do
    validate_staged_file "$binary"
    [[ -x "$binary" ]] || fail "staged release binary is not executable: $binary"
    grep -aFq "${KANBAN_BUILD_ID:?}" "$binary" ||
      fail "staged release binary is not bound to KANBAN_BUILD_ID: $binary"
  done
  verify_helper_identity "$artifacts/bin/kanban-vector-lancedb"
  verify_helper_identity "$artifacts/bin/kanban-graph-oxigraph"

  unpacked="$(mktemp -d)"
  unpacked="$(cd "$unpacked" && pwd -P)"
  unpacked_parent="$(dirname "$unpacked")"
  read -r unpacked_dev unpacked_ino < <(
    python3 "$SAFE_PATH" dir-identity --root "$unpacked_parent" --path "$unpacked"
  )
  cleanup_unpacked() {
    python3 "$SAFE_PATH" remove-tree --root "$unpacked_parent" \
      --path "$unpacked" --expected-dev "$unpacked_dev" \
      --expected-ino "$unpacked_ino" ||
      printf 'warning: retained unverified package extraction: %s\n' "$unpacked" >&2
  }
  trap cleanup_unpacked EXIT
  cli_root="$unpacked/cli"
  desktop_root="$unpacked/desktop"
  dpkg-deb -x "$artifacts/deb/kanban-tool-cli.deb" "$cli_root"
  dpkg-deb -x "$artifacts/deb/kanban-tool-desktop.deb" "$desktop_root"

  compare_payload() {
    local packaged="$1" staged="$2"
    [[ -f "$packaged" && ! -L "$packaged" ]] ||
      fail "packaged cohort payload is missing or unsafe: $packaged"
    cmp -s "$packaged" "$staged" ||
      fail "packaged cohort payload does not match its staged artifact: $packaged"
  }
  compare_payload "$cli_root/usr/bin/kanban" "$artifacts/bin/kanban"
  compare_payload "$cli_root/usr/lib/kanban/kanban-vector-lancedb" \
    "$artifacts/bin/kanban-vector-lancedb"
  compare_payload "$cli_root/usr/lib/kanban/kanban-graph-oxigraph" \
    "$artifacts/bin/kanban-graph-oxigraph"
  compare_payload "$desktop_root/usr/bin/kanban-desktop" \
    "$artifacts/bin/kanban-desktop"
  compare_payload "$desktop_root/usr/bin/kanban-vector-lancedb" \
    "$artifacts/bin/kanban-vector-lancedb"
  compare_payload "$desktop_root/usr/bin/kanban-graph-oxigraph" \
    "$artifacts/bin/kanban-graph-oxigraph"
  for package_root in \
    "$cli_root/usr/share/doc/kanban-tool-cli" \
    "$desktop_root/usr/share/doc/kanban-tool-desktop"
  do
    compare_payload "$package_root/source-provenance.json" "$source_manifest"
    compare_payload "$package_root/derived-projection-v2-source-map.json" "$source_map"
  done
)

verify_manifest_hashes() {
  local manifest="$1" stage="$2"
  "$SOURCE_GATE" validate --manifest "$stage/source-provenance.json"
  python3 - "$manifest" "$stage" "$TARGET_ROOT" "${KANBAN_BUILD_ID:?}" \
    "${KANBAN_RELEASE_GENERATION_KEY:-}" <<'PY'
import hashlib
import json
import os
import pathlib
import re
import stat
import sys

manifest_path = pathlib.Path(sys.argv[1])
stage = pathlib.Path(sys.argv[2])
target_root = pathlib.Path(sys.argv[3])
build_id = sys.argv[4]
expected_generation_key = sys.argv[5]

def digest(path: pathlib.Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            value.update(chunk)
    return value.hexdigest()

def exact_file(path: pathlib.Path) -> os.stat_result:
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
        raise SystemExit(f"error: staged release file is unsafe: {path}")
    return metadata

document = json.loads(manifest_path.read_text(encoding="utf-8"))
expected_manifest_keys = {
    "artifacts", "build_id", "commit", "generation_key", "generation_path",
    "identity", "identity_sha256", "project", "schema_version",
    "source_manifest", "source_map", "tree", "version",
}
if not isinstance(document, dict) or set(document) != expected_manifest_keys:
    raise SystemExit("error: release artifact manifest has missing or extra fields")
if (
    document.get("schema_version") != 3
    or document.get("project") != "kanban-tool"
    or document.get("build_id") != build_id
):
    raise SystemExit("error: release artifact manifest has the wrong schema/build identity")
generation_key = document.get("generation_key")
if (
    not isinstance(generation_key, str)
    or not re.fullmatch(r"[0-9a-f]{40}-[0-9a-f]{40}-[0-9a-f]{64}", generation_key)
    or (expected_generation_key and generation_key != expected_generation_key)
):
    raise SystemExit("error: release artifact manifest has the wrong generation identity")
generation_path_raw = document.get("generation_path")
expected_generation_path = f"release/bundle/cohort/{generation_key}"
if not isinstance(generation_path_raw, str) or generation_path_raw != expected_generation_path:
    raise SystemExit(
        "error: release artifact manifest generation path is not the canonical cohort publication directory"
    )
generation_path = pathlib.PurePosixPath(generation_path_raw)
identity = document.get("identity")
if not isinstance(identity, dict):
    raise SystemExit("error: release artifact manifest lacks release identity")
if set(identity) != {
    "cargo_lock", "features", "identity_sha256", "registry_closure", "target", "toolchain"
}:
    raise SystemExit("error: release artifact identity has missing or extra fields")
if document.get("identity_sha256") != identity.get("identity_sha256"):
    raise SystemExit("error: release artifact identity hash drifted")
identity_payload = dict(identity)
identity_digest = identity_payload.pop("identity_sha256", None)
canonical_identity = json.dumps(
    identity_payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")
)
if hashlib.sha256(canonical_identity.encode()).hexdigest() != identity_digest:
    raise SystemExit("error: release artifact identity hash is not canonical")

roles = {
    "cli_binary": pathlib.Path("artifacts/bin/kanban"),
    "lancedb_helper": pathlib.Path("artifacts/bin/kanban-vector-lancedb"),
    "oxigraph_helper": pathlib.Path("artifacts/bin/kanban-graph-oxigraph"),
    "desktop_binary": pathlib.Path("artifacts/bin/kanban-desktop"),
    "cli_deb": pathlib.Path("artifacts/deb/kanban-tool-cli.deb"),
    "desktop_deb": pathlib.Path("artifacts/deb/kanban-tool-desktop.deb"),
}
items = document.get("artifacts")
if (
    not isinstance(items, list)
    or len(items) != len(roles)
    or {item.get("role") for item in items} != set(roles)
):
    raise SystemExit("error: release artifact manifest does not contain the exact cohort roles")
for item in items:
    if not isinstance(item, dict) or set(item) != {"build_id", "path", "role", "sha256", "size"}:
        raise SystemExit("error: release artifact entry has missing or extra fields")
    role = item["role"]
    path = stage / roles[role]
    metadata = exact_file(path)
    published = target_root / item["path"]
    expected_suffix = generation_path / roles[role]
    if published != target_root / expected_suffix:
        raise SystemExit(f"error: release artifact path does not match generation: {role}")
    if (
        not isinstance(item["path"], str)
        or pathlib.PurePosixPath(item["path"]).is_absolute()
        or ".." in pathlib.PurePosixPath(item["path"]).parts
    ):
        raise SystemExit(f"error: release artifact path is unsafe: {role}")
    if item.get("build_id") != build_id:
        raise SystemExit(f"error: release artifact build identity drifted: {role}")
    if (
        not isinstance(item.get("size"), int)
        or item.get("size") < 0
        or not isinstance(item.get("sha256"), str)
        or not re.fullmatch(r"[0-9a-f]{64}", item["sha256"])
        or item.get("size") != metadata.st_size
        or item.get("sha256") != digest(path)
    ):
        raise SystemExit(f"error: staged release artifact changed after hashing: {role}")

source = json.loads((stage / "source-provenance.json").read_text(encoding="utf-8"))
if (
    source.get("build_id") != build_id
    or document.get("commit") != source.get("commit")
    or document.get("tree") != source.get("tree")
    or document.get("version") != source.get("version")
    or document.get("generation_key") != source.get("generation_key")
    or document.get("identity") != source.get("identity")
    or document.get("identity_sha256") != source.get("identity_sha256")
):
    raise SystemExit("error: release artifact manifest drifted from source provenance")

for key, name in (
    ("source_manifest", "source-provenance.json"),
    ("source_map", "derived-projection-v2-source-map.json"),
):
    entry = document.get(key)
    path = stage / name
    metadata = exact_file(path)
    if not isinstance(entry, dict):
        raise SystemExit(f"error: release manifest lacks {key}")
    expected_entry_keys = {"path", "sha256", "size"}
    if key == "source_map":
        expected_entry_keys.add("canonical_path")
    if set(entry) != expected_entry_keys:
        raise SystemExit(f"error: release manifest {key} has missing or extra fields")
    if entry.get("path") != (generation_path / name).as_posix():
        raise SystemExit(f"error: release manifest has the wrong {key} path")
    if entry.get("size") != metadata.st_size or entry.get("sha256") != digest(path):
        raise SystemExit(f"error: staged {key} changed after hashing")
if document["source_map"].get("canonical_path") != source.get("source_map", {}).get("path"):
    raise SystemExit("error: release manifest has the wrong canonical source map path")
if document["source_map"].get("sha256") != source.get("source_map", {}).get("sha256"):
    raise SystemExit("error: staged source map does not match source provenance")
PY
}

prepare() {
  local release_dir build_id source_map version manifest_build_id
  local cli_binary lance_helper oxigraph_helper desktop_binary cli_deb desktop_deb
  local artifacts_dir temporary expected_published_dir

  [[ -f "$SOURCE_MANIFEST" && ! -L "$SOURCE_MANIFEST" ]] ||
    fail "source provenance manifest is missing or unsafe"
  "$SOURCE_GATE" validate --manifest "$SOURCE_MANIFEST"
  [[ -n "$OUTPUT" && -n "$PUBLISHED_DIR" ]] || { usage >&2; exit 2; }
  PUBLISHED_DIR="$(canonical_under_target "$PUBLISHED_DIR")"
  [[ "$(dirname "$OUTPUT")" == "$STAGE_DIR" ]] ||
    fail "artifact manifest output must stay in the private cohort stage"
  [[ "$(cd "$(dirname "$SOURCE_MANIFEST")" && pwd -P)" == "$STAGE_DIR" ]] ||
    fail "source provenance manifest must stay in the private cohort stage"
  command -v dpkg-deb >/dev/null 2>&1 || fail "dpkg-deb is required"

  release_dir="$TARGET_ROOT/release"
  build_id="${KANBAN_BUILD_ID:-}"
  source_map="${KANBAN_RELEASE_SOURCE_MAP:-}"
  [[ -n "$build_id" ]] || fail "KANBAN_BUILD_ID is required"
  [[ -f "$source_map" && ! -L "$source_map" ]] ||
    fail "KANBAN_RELEASE_SOURCE_MAP is missing or unsafe"
  read -r version manifest_build_id < <(
    python3 - "$SOURCE_MANIFEST" <<'PY'
import json
import pathlib
import sys

document = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
version = document.get("version")
build_id = document.get("build_id")
if not isinstance(version, str) or not isinstance(build_id, str):
    raise SystemExit("error: source provenance lacks version/build_id")
print(version, build_id)
PY
  )
  [[ "$manifest_build_id" == "$build_id" ]] ||
    fail "KANBAN_BUILD_ID does not match source provenance"
  expected_published_dir="$TARGET_ROOT/release/bundle/cohort/$(
    python3 - "$SOURCE_MANIFEST" <<'PY'
import json
import pathlib
import sys

document = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
generation_key = document.get("generation_key")
if not isinstance(generation_key, str):
    raise SystemExit("error: source provenance lacks generation key")
print(generation_key)
PY
  )"
  [[ "$PUBLISHED_DIR" == "$expected_published_dir" ]] ||
    fail "published generation directory is not the canonical cohort path"

  single_match() {
    local label="$1" directory="$2" pattern="$3"
    local -a matches
    mapfile -d '' matches < <(
      find "$directory" -maxdepth 1 -type f -name "$pattern" -print0 2>/dev/null
    )
    [[ "${#matches[@]}" -eq 1 ]] ||
      fail "$label must resolve to exactly one artifact, found ${#matches[@]}"
    printf '%s\n' "${matches[0]}"
  }

  cli_binary="$release_dir/kanban"
  lance_helper="$release_dir/kanban-vector-lancedb"
  oxigraph_helper="$release_dir/kanban-graph-oxigraph"
  desktop_binary="$release_dir/kanban-desktop"
  cli_deb="$(single_match "CLI Debian package" "$release_dir/bundle/cli/deb" \
    "kanban-tool-cli_${version}-*_*.deb")"
  desktop_deb="$(single_match "Desktop Debian package" "$release_dir/bundle/deb" \
    "Kanban Tool_${version}_*.deb")"

  for source in \
    "$cli_binary" "$lance_helper" "$oxigraph_helper" "$desktop_binary" \
    "$cli_deb" "$desktop_deb" "$SOURCE_MANIFEST" "$source_map"
  do
    python3 "$SAFE_PATH" validate-file --root "$TARGET_ROOT" --path "$source" 2>/dev/null ||
      # The canonical source map is outside target_root and is validated by the
      # source gate; validate it against its own no-symlink parent instead.
      if [[ "$source" == "$source_map" ]]; then
        python3 "$SAFE_PATH" validate-file \
          --root "$(cd "$(dirname "$source_map")" && pwd -P)" --path "$source_map"
      else
        fail "release input is missing, multiply linked, or unsafe: $source"
      fi
  done
  for binary in "$cli_binary" "$lance_helper" "$oxigraph_helper" "$desktop_binary"; do
    [[ -x "$binary" ]] || fail "release binary is not executable: $binary"
    grep -aFq "$build_id" "$binary" ||
      fail "release binary is not bound to KANBAN_BUILD_ID: $binary"
  done

  artifacts_dir="$STAGE_DIR/artifacts"
  python3 "$SAFE_PATH" ensure-dir --root "$TARGET_ROOT" \
    --path "$artifacts_dir/bin" --mode 0755
  python3 "$SAFE_PATH" ensure-dir --root "$TARGET_ROOT" \
    --path "$artifacts_dir/deb" --mode 0755
  python3 "$SAFE_PATH" copy-file --root "$TARGET_ROOT" \
    --source "$cli_binary" --destination "$artifacts_dir/bin/kanban" --mode 0555
  python3 "$SAFE_PATH" copy-file --root "$TARGET_ROOT" \
    --source "$lance_helper" \
    --destination "$artifacts_dir/bin/kanban-vector-lancedb" --mode 0555
  python3 "$SAFE_PATH" copy-file --root "$TARGET_ROOT" \
    --source "$oxigraph_helper" \
    --destination "$artifacts_dir/bin/kanban-graph-oxigraph" --mode 0555
  python3 "$SAFE_PATH" copy-file --root "$TARGET_ROOT" \
    --source "$desktop_binary" \
    --destination "$artifacts_dir/bin/kanban-desktop" --mode 0555
  python3 "$SAFE_PATH" copy-file --root "$TARGET_ROOT" \
    --source "$cli_deb" --destination "$artifacts_dir/deb/kanban-tool-cli.deb" --mode 0444
  python3 "$SAFE_PATH" copy-file --root "$TARGET_ROOT" \
    --source "$desktop_deb" \
    --destination "$artifacts_dir/deb/kanban-tool-desktop.deb" --mode 0444
  python3 "$SAFE_PATH" copy-file --root "$TARGET_ROOT" \
    --source "$source_map" \
    --destination "$STAGE_DIR/derived-projection-v2-source-map.json" --mode 0444

  verify_helper_identity "$artifacts_dir/bin/kanban-vector-lancedb"
  verify_helper_identity "$artifacts_dir/bin/kanban-graph-oxigraph"

  verify_staged_cohort "$STAGE_DIR" "$SOURCE_MANIFEST" \
    "$STAGE_DIR/derived-projection-v2-source-map.json"

  temporary="$(mktemp "$STAGE_DIR/.release-artifacts.XXXXXX")"
  trap 'if [[ -n "${temporary:-}" && -e "$temporary" ]]; then
    printf "warning: retained unverified artifact manifest temp: %s\n" "$temporary" >&2
  fi' RETURN
  python3 - "$temporary" "$TARGET_ROOT" "$PUBLISHED_DIR" "$SOURCE_MANIFEST" \
    "$STAGE_DIR/derived-projection-v2-source-map.json" "$build_id" \
    "cli_binary=artifacts/bin/kanban" \
    "lancedb_helper=artifacts/bin/kanban-vector-lancedb" \
    "oxigraph_helper=artifacts/bin/kanban-graph-oxigraph" \
    "desktop_binary=artifacts/bin/kanban-desktop" \
    "cli_deb=artifacts/deb/kanban-tool-cli.deb" \
    "desktop_deb=artifacts/deb/kanban-tool-desktop.deb" <<'PY'
import hashlib
import json
import pathlib
import sys

output = pathlib.Path(sys.argv[1])
target_root = pathlib.Path(sys.argv[2])
published_dir = pathlib.Path(sys.argv[3])
source_manifest = pathlib.Path(sys.argv[4])
source_map = pathlib.Path(sys.argv[5])
build_id = sys.argv[6]
stage = source_manifest.parent

def digest(path: pathlib.Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            value.update(chunk)
    return value.hexdigest()

artifacts = []
for item in sys.argv[7:]:
    role, relative_raw = item.split("=", 1)
    relative = pathlib.Path(relative_raw)
    staged = stage / relative
    published = published_dir / relative
    artifacts.append(
        {
            "build_id": build_id,
            "path": published.relative_to(target_root).as_posix(),
            "role": role,
            "sha256": digest(staged),
            "size": staged.stat().st_size,
        }
    )

source = json.loads(source_manifest.read_text(encoding="utf-8"))
if published_dir.name != source["generation_key"]:
    raise SystemExit("error: published generation path does not match source identity")
document = {
    "artifacts": artifacts,
    "build_id": build_id,
    "commit": source["commit"],
    "generation_key": source["generation_key"],
    "generation_path": published_dir.relative_to(target_root).as_posix(),
    "identity": source["identity"],
    "identity_sha256": source["identity_sha256"],
    "project": "kanban-tool",
    "schema_version": 3,
    "source_manifest": {
        "path": (published_dir / source_manifest.name).relative_to(target_root).as_posix(),
        "sha256": digest(source_manifest),
        "size": source_manifest.stat().st_size,
    },
    "source_map": {
        "canonical_path": source["source_map"]["path"],
        "path": (published_dir / source_map.name).relative_to(target_root).as_posix(),
        "sha256": digest(source_map),
        "size": source_map.stat().st_size,
    },
    "tree": source["tree"],
    "version": source["version"],
}
output.write_text(
    json.dumps(document, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n",
    encoding="utf-8",
)
PY
  chmod 0444 "$temporary"
  python3 "$SAFE_PATH" publish-file --root "$TARGET_ROOT" \
    --source "$temporary" --destination "$OUTPUT"
  trap - RETURN
}

verify() {
  [[ -f "$MANIFEST" && ! -L "$MANIFEST" ]] ||
    fail "release artifact manifest is missing or unsafe"
  [[ "$(dirname "$MANIFEST")" == "$STAGE_DIR" ]] ||
    fail "release artifact manifest must stay in the private cohort stage"
  verify_manifest_hashes "$MANIFEST" "$STAGE_DIR"
  verify_staged_cohort "$STAGE_DIR" "$STAGE_DIR/source-provenance.json" \
    "$STAGE_DIR/derived-projection-v2-source-map.json"
}

verify_final() {
  "$SOURCE_GATE" verify --manifest "$STAGE_DIR/source-provenance.json"
  verify
}

case "$COMMAND" in
  prepare)
    [[ -n "$SOURCE_MANIFEST" && -n "$OUTPUT" && -n "$PUBLISHED_DIR" ]] ||
      { usage >&2; exit 2; }
    prepare
    ;;
  verify)
    [[ -n "$MANIFEST" ]] || { usage >&2; exit 2; }
    verify
    ;;
  verify-final)
    [[ -n "$MANIFEST" ]] || { usage >&2; exit 2; }
    verify_final
    ;;
  -h|--help)
    usage
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac
