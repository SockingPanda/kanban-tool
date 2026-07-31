#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
SAFE_PATH="$ROOT/scripts/release-safe-path.py"
SOURCE_MAP_REL="docs/release/derived-projection-v2-source-map.json"
# The injector is always paired with the source-gate copy beside this sealed
# script.  A caller cannot substitute a no-op gate through the environment;
# the cohort wrapper points the gate at the live evidence root separately.
SOURCE_GATE="$ROOT/scripts/release-source-gate.sh"

usage() {
  cat <<'EOF'
Usage:
  scripts/embed-release-provenance-deb.sh --deb <package.deb> --doc-dir <name>

Inject the release source manifest and semantic source map into an existing
Debian package. KANBAN_BUILD_ID, KANBAN_RELEASE_SOURCE_MANIFEST, and
KANBAN_RELEASE_SOURCE_MAP must come from scripts/release-cohort.sh.
EOF
}

fail() {
  echo "error: $*" >&2
  exit 1
}

stage_regular_input() {
  local source="$1" destination="$2" label="$3"
  python3 - "$source" "$destination" "$label" <<'PY'
import os
import pathlib
import stat
import sys

source = pathlib.Path(sys.argv[1])
destination = pathlib.Path(sys.argv[2])
label = sys.argv[3]

def read_regular(path: pathlib.Path) -> bytes:
    if not path.is_absolute() or ".." in path.parts:
        raise SystemExit(f"error: {label} path must be absolute and traversal-free: {path}")
    current = pathlib.Path(path.anchor)
    for component in path.parts[1:]:
        current /= component
        try:
            metadata = os.lstat(current)
        except OSError as error:
            raise SystemExit(f"error: cannot inspect {label}: {path}: {error}")
        if stat.S_ISLNK(metadata.st_mode):
            raise SystemExit(f"error: {label} contains a symlink component: {current}")
        if current == path and (
            not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1
        ):
            raise SystemExit(f"error: {label} is not a single-link regular file: {path}")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
    )
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
        fields = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns", "st_nlink")
        if any(getattr(before, field) != getattr(after, field) for field in fields):
            raise SystemExit(f"error: {label} changed while being sampled: {path}")
        return b"".join(chunks)
    finally:
        os.close(descriptor)

payload = read_regular(source)
parent = destination.parent
parent_fd = os.open(
    parent,
    os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0),
)
try:
    descriptor = os.open(
        destination.name,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL |
        getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
        0o600,
        dir_fd=parent_fd,
    )
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            view = view[written:]
        os.fchmod(descriptor, 0o444)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    os.fsync(parent_fd)
finally:
    os.close(parent_fd)
PY
}

rewrite_and_verify_control() {
  local control="$1" mode="${2:-rewrite}"
  python3 - "$control" "$BUILD_ID" "$mode" <<'PY'
import os
import pathlib
import re
import stat
import sys

path = pathlib.Path(sys.argv[1])
build_id = sys.argv[2]
mode = sys.argv[3]
if not build_id or any(char in build_id for char in "\r\n\x00"):
    raise SystemExit("error: build identity cannot be represented in Debian control")

if not path.is_absolute() or ".." in path.parts:
    raise SystemExit(f"error: Debian control path is unsafe: {path}")
current = pathlib.Path(path.anchor)
for component in path.parts[1:-1]:
    current /= component
    try:
        metadata = os.lstat(current)
    except OSError as error:
        raise SystemExit(f"error: cannot inspect Debian control parent: {current}: {error}")
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise SystemExit(f"error: Debian control parent is unsafe: {current}")
try:
    final_metadata = os.lstat(path)
except OSError as error:
    raise SystemExit(f"error: cannot inspect Debian control: {path}: {error}")
if stat.S_ISLNK(final_metadata.st_mode):
    raise SystemExit(f"error: Debian control is a symlink: {path}")

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
        fields = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns", "st_nlink")
        if any(getattr(metadata, field) != getattr(after, field) for field in fields):
            raise SystemExit("error: Debian control changed while being read")
        return b"".join(chunks)
    finally:
        os.close(descriptor)

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

payload = read_regular()
lines, fields = parse(payload)
matches = [field for field in fields if field["key"].lower() == "x-kanban-build-id"]
if mode == "verify":
    if len(matches) != 1 or matches[0]["key"] != "X-Kanban-Build-Id" or \
        matches[0]["value"] != " " + build_id or len(matches[0]["lines"]) != 1:
        raise SystemExit("error: Debian control does not contain exactly one canonical build identity field")
    raise SystemExit(0)
if len(matches) > 1:
    raise SystemExit("error: Debian control contains duplicate X-Kanban-Build-Id fields")
if matches:
    field = matches[0]
    if len(field["lines"]) != 1 or field["value"] != " " + build_id:
        raise SystemExit("error: Debian control contains a conflicting X-Kanban-Build-Id field")
remove = {index for field in matches for index, _ in field["lines"]}
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
    raise SystemExit("error: Debian control rewrite is not canonical")
PY
}

DEB=""
DOC_DIR=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --deb)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      DEB="$2"
      shift 2
      ;;
    --doc-dir)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      DOC_DIR="$2"
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

[[ -n "$DEB" ]] || fail "Debian package path is required"
if [[ "$DEB" != /* ]]; then
  DEB="$(pwd -P)/$DEB"
fi
TARGET_ROOT="$("$LOCK" --print-target-dir)"
[[ "$TARGET_ROOT" == /* && -d "$TARGET_ROOT" ]] ||
  fail "Cargo target root must be an existing absolute directory"
python3 "$SAFE_PATH" ensure-dir --root "$TARGET_ROOT" --path "$TARGET_ROOT" --mode 0755
TARGET_ROOT="$(cd "$TARGET_ROOT" && pwd -P)"
python3 "$SAFE_PATH" validate-file --root "$TARGET_ROOT" --path "$DEB" ||
  fail "Debian package is missing, multiply linked, or unsafe: $DEB"
DEB_DIR="$(cd "$(dirname "$DEB")" && pwd -P)"
case "$DEB_DIR/" in
  "$ROOT/"*) fail "release provenance injector refuses to rewrite a package inside the source tree" ;;
esac
DEB="$DEB_DIR/$(basename "$DEB")"
[[ "$DOC_DIR" =~ ^[a-z0-9][a-z0-9.+-]*$ ]] ||
  fail "invalid Debian documentation directory: $DOC_DIR"
command -v dpkg-deb >/dev/null 2>&1 || fail "dpkg-deb is required"

SOURCE_MANIFEST="${KANBAN_RELEASE_SOURCE_MANIFEST:-}"
SOURCE_MAP="${KANBAN_RELEASE_SOURCE_MAP:-}"
BUILD_ID="${KANBAN_BUILD_ID:-}"
[[ -n "$BUILD_ID" ]] || fail "KANBAN_BUILD_ID is required"
[[ "$SOURCE_MAP" == "$ROOT/$SOURCE_MAP_REL" ]] ||
  fail "release source map is not the canonical immutable source file"

TMPDIR="$(
  python3 "$SAFE_PATH" private-dir --root "$TARGET_ROOT" \
    --parent "$DEB_DIR" --prefix ".embed-release-provenance."
)"
read -r TMPDIR_DEV TMPDIR_INO < <(
  python3 "$SAFE_PATH" dir-identity --root "$TARGET_ROOT" --path "$TMPDIR"
)
cleanup_tmpdir() {
  python3 "$SAFE_PATH" remove-tree --root "$TARGET_ROOT" \
    --path "$TMPDIR" --expected-dev "$TMPDIR_DEV" --expected-ino "$TMPDIR_INO" ||
    printf 'warning: retained unverified Debian provenance stage: %s\n' "$TMPDIR" >&2
}
trap cleanup_tmpdir EXIT

STAGED_MANIFEST="$TMPDIR/source-provenance.json"
STAGED_SOURCE_MAP="$TMPDIR/derived-projection-v2-source-map.json"
INPUT_DEB="$TMPDIR/input.deb"
python3 "$SAFE_PATH" copy-file --root "$TARGET_ROOT" \
  --source "$SOURCE_MANIFEST" --destination "$STAGED_MANIFEST" --mode 0444
python3 "$SAFE_PATH" copy-file --root "$TARGET_ROOT" \
  --source "$SOURCE_MAP" --destination "$STAGED_SOURCE_MAP" --mode 0444
python3 "$SAFE_PATH" copy-file --root "$TARGET_ROOT" \
  --source "$DEB" --destination "$INPUT_DEB" --mode 0444
"$SOURCE_GATE" validate --manifest "$STAGED_MANIFEST"
python3 - "$STAGED_MANIFEST" "$STAGED_SOURCE_MAP" "$BUILD_ID" <<'PY'
import hashlib
import json
import pathlib
import re
import sys

manifest_path = pathlib.Path(sys.argv[1])
source_map_path = pathlib.Path(sys.argv[2])
build_id = sys.argv[3]
try:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
except (OSError, UnicodeError, json.JSONDecodeError) as error:
    raise SystemExit(f"error: invalid staged source provenance: {error}")

def exact(value, keys, label):
    if not isinstance(value, dict) or set(value) != set(keys):
        raise SystemExit(f"error: {label} has missing or extra fields")

def digest(value, label):
    if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
        raise SystemExit(f"error: {label} must be a lowercase SHA-256")

def object_id(value, label):
    if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{40}", value):
        raise SystemExit(f"error: {label} must be one exact Git object id")

def reject_absolute(value):
    if isinstance(value, str) and value.startswith("/"):
        raise SystemExit("error: staged source provenance contains an absolute path")
    if isinstance(value, dict):
        for child in value.values():
            reject_absolute(child)
    elif isinstance(value, list):
        for child in value:
            reject_absolute(child)

expected_top = {
    "branch", "build_id", "commit", "generation_key", "identity",
    "identity_sha256", "project", "remote", "schema_version",
    "semantic_source", "source_map", "tree", "version",
}
exact(manifest, expected_top, "staged source provenance")
if manifest["schema_version"] != 3 or manifest["project"] != "kanban-tool":
    raise SystemExit("error: staged source provenance has the wrong project or schema version")
if manifest["branch"] != "main":
    raise SystemExit("error: staged source provenance has the wrong branch")
commit = manifest["commit"]
tree = manifest["tree"]
identity_digest = manifest["identity_sha256"]
object_id(commit, "staged source provenance commit")
object_id(tree, "staged source provenance tree")
digest(identity_digest, "staged source provenance identity")
version = manifest["version"]
if not isinstance(version, str) or not re.fullmatch(
    r"[0-9]+\.[0-9]+\.[0-9]+([.+-][0-9A-Za-z.-]+)?", version
):
    raise SystemExit("error: staged source provenance version is invalid")
if manifest["build_id"] != build_id:
    raise SystemExit("error: KANBAN_BUILD_ID does not match source provenance")
if manifest["generation_key"] != f"{commit}-{tree}-{identity_digest}":
    raise SystemExit("error: staged source provenance generation key is not canonical")
if build_id != (
    f"kanban-tool/{version};commit={commit};tree={tree};identity={identity_digest}"
):
    raise SystemExit("error: staged source provenance build identity is not canonical")
identity = manifest.get("identity")
exact(
    identity,
    {"cargo_lock", "features", "identity_sha256", "registry_closure", "target", "toolchain"},
    "staged release identity",
)
if identity["identity_sha256"] != identity_digest:
    raise SystemExit("error: staged source provenance identity hash does not match identity")
identity_payload = dict(identity)
identity_payload.pop("identity_sha256")
canonical_identity = json.dumps(
    identity_payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")
)
if hashlib.sha256(canonical_identity.encode()).hexdigest() != identity_digest:
    raise SystemExit("error: staged source provenance identity hash is not canonical")

for field, expected_path in (
    ("cargo_lock", "Cargo.lock"),
    ("registry_closure", "policy/schema-tool-registry-closure.json"),
):
    entry = identity[field]
    exact(entry, {"path", "sha256"}, f"staged release identity {field}")
    if entry["path"] != expected_path:
        raise SystemExit(f"error: staged release identity {field} path is not canonical")
    digest(entry["sha256"], f"staged release identity {field}")

if identity["features"] != {
    "effective": ["oxigraph-backend", "tantivy-backend"],
    "no_default_features": True,
}:
    raise SystemExit("error: staged source provenance feature contract is not canonical")
target = identity["target"]
exact(target, {"deb_arch", "machine_arch", "platform", "triple"}, "staged release identity target")
if target["platform"] != "Linux":
    raise SystemExit("error: staged source provenance target platform is not Linux")
for field, pattern in (
    ("deb_arch", r"[a-z0-9][a-z0-9_-]*"),
    ("machine_arch", r"[A-Za-z0-9][A-Za-z0-9_.-]*"),
    ("triple", r"[A-Za-z0-9][A-Za-z0-9_.-]*"),
):
    if not isinstance(target[field], str) or not re.fullmatch(pattern, target[field]):
        raise SystemExit(f"error: staged source provenance target {field} is invalid")
toolchain = identity["toolchain"]
exact(
    toolchain,
    {"cargo_version", "cargo_version_sha256", "rustc_vv", "rustc_vv_sha256"},
    "staged release identity toolchain",
)
for field in ("cargo_version", "rustc_vv"):
    value = toolchain[field]
    if not isinstance(value, str) or not value or "\x00" in value:
        raise SystemExit(f"error: staged release identity toolchain {field} is invalid")
    if any(token.startswith("/") for token in re.split(r"[\s=()]", value) if token):
        raise SystemExit(f"error: staged release identity toolchain {field} leaks an absolute path")
for value_name, hash_name in (("rustc_vv", "rustc_vv_sha256"), ("cargo_version", "cargo_version_sha256")):
    value = toolchain.get(value_name)
    hash_value = toolchain.get(hash_name)
    if not isinstance(value, str) or not isinstance(hash_value, str) or \
        hashlib.sha256(value.encode()).hexdigest() != hash_value:
        raise SystemExit(f"error: staged release identity toolchain {value_name} hash is invalid")

remote = manifest["remote"]
exact(remote, {"commit", "name", "ref"}, "staged source provenance remote")
if remote["name"] != "origin" or remote["ref"] != "refs/heads/main" or remote["commit"] != commit:
    raise SystemExit("error: staged source provenance remote is not canonical")
object_id(remote["commit"], "staged source provenance remote commit")

semantic = manifest["semantic_source"]
exact(
    semantic,
    {"name", "no_merge_base_with_main", "remote_ref", "remote_tip", "saved_ref", "saved_tip", "verified_source_commits"},
    "staged source provenance semantic source",
)
if (
    semantic["name"] != "origin/derived-projection-v2"
    or semantic["remote_ref"] != "refs/heads/derived-projection-v2"
    or semantic["saved_ref"] != "refs/remotes/origin/derived-projection-v2"
    or semantic["no_merge_base_with_main"] is not True
):
    raise SystemExit("error: staged source provenance semantic source is not canonical")
object_id(semantic["remote_tip"], "staged semantic source remote tip")
object_id(semantic["saved_tip"], "staged semantic source saved tip")
if not isinstance(semantic["verified_source_commits"], list) or len(semantic["verified_source_commits"]) != 3:
    raise SystemExit("error: staged source provenance source commit list is invalid")
for index, value in enumerate(semantic["verified_source_commits"]):
    object_id(value, f"staged semantic source commit {index}")

source = manifest["source_map"]
exact(source, {"path", "sha256"}, "staged source provenance source map")
if source["path"] != "docs/release/derived-projection-v2-source-map.json":
    raise SystemExit("error: staged source provenance source map path is not canonical")
digest(source["sha256"], "staged source provenance source map")
actual_hash = hashlib.sha256(source_map_path.read_bytes()).hexdigest()
if source["sha256"] != actual_hash:
    raise SystemExit("error: staged source map hash does not match source provenance")

try:
    source_map = json.loads(source_map_path.read_text(encoding="utf-8"))
except (OSError, UnicodeError, json.JSONDecodeError) as error:
    raise SystemExit(f"error: invalid staged semantic source map: {error}")
exact(
    source_map,
    {"description_zh", "integration_strategy", "mappings", "project", "schema_version", "source_ref", "source_tip"},
    "staged semantic source map",
)
if (
    source_map["schema_version"] != 1
    or source_map["project"] != "kanban-tool"
    or source_map["integration_strategy"] != "semantic-port-without-merge-base"
    or source_map["source_ref"] != "refs/heads/derived-projection-v2"
):
    raise SystemExit("error: staged semantic source map metadata is not canonical")
expected_mappings = [
    {
        "integrated_commit": "095a5c2ee88434976ae7f8c8bf8310c8227eec70",
        "source_branch": "origin/derived-projection-v2",
        "source_commit": "0e58068fc67913495d8566e0a839e7a068456f81",
        "subject": "Projection v2 consistency foundation",
    },
    {
        "integrated_commit": "7f218272f4dfe12ee2c85bedc376c13c809a87ae",
        "source_branch": "origin/derived-projection-v2",
        "source_commit": "85e1f797c2d5c2b089d1a2f29827f42e25cd595c",
        "subject": "unified maintenance runtime + DB-scoped multi-board Tantivy v2",
    },
    {
        "integrated_commit": "e1c3b55bf18774cba49d5f2691a4107d1ea73882",
        "source_branch": "origin/derived-projection-v2",
        "source_commit": "c764706fcae214f58a3a65f5dc565135522bbd81",
        "subject": "DB-scoped Oxigraph Projection v2",
    },
]
if source_map["mappings"] != expected_mappings:
    raise SystemExit("error: staged semantic source map mappings are not canonical")
object_id(source_map["source_tip"], "staged semantic source map tip")
if source_map["source_tip"] != expected_mappings[-1]["source_commit"]:
    raise SystemExit("error: staged semantic source map tip is not canonical")
if semantic["remote_tip"] != source_map["source_tip"] or semantic["saved_tip"] != source_map["source_tip"]:
    raise SystemExit("error: staged semantic source tips do not match the source map")
if semantic["verified_source_commits"] != [item["source_commit"] for item in expected_mappings]:
    raise SystemExit("error: staged semantic source commits do not match the source map")
reject_absolute(manifest)
PY

UNPACKED="$TMPDIR/package"
OUTPUT="$TMPDIR/package.deb"
dpkg-deb -R "$INPUT_DEB" "$UNPACKED"
CONTROL="$UNPACKED/DEBIAN/control"
[[ -f "$CONTROL" && ! -L "$CONTROL" ]] || fail "Debian package has no safe control file"
rewrite_and_verify_control "$CONTROL" rewrite

DOC_ROOT="$UNPACKED/usr/share/doc/$DOC_DIR"
python3 "$SAFE_PATH" ensure-dir --root "$TARGET_ROOT" --path "$DOC_ROOT" --mode 0755
python3 "$SAFE_PATH" copy-file --root "$TARGET_ROOT" \
  --source "$STAGED_MANIFEST" \
  --destination "$DOC_ROOT/source-provenance.json" --mode 0644
python3 "$SAFE_PATH" copy-file --root "$TARGET_ROOT" \
  --source "$STAGED_SOURCE_MAP" \
  --destination "$DOC_ROOT/derived-projection-v2-source-map.json" --mode 0644

dpkg-deb --root-owner-group --build "$UNPACKED" "$OUTPUT" >/dev/null
python3 "$SAFE_PATH" validate-file --root "$TARGET_ROOT" --path "$OUTPUT"

verify_extracted_package() {
  local package="$1" extraction="$2" control_dir="$3"
  dpkg-deb -e "$package" "$control_dir"
  rewrite_and_verify_control "$control_dir/control" verify
  dpkg-deb -x "$package" "$extraction"
  python3 - "$extraction/usr/share/doc/$DOC_DIR/source-provenance.json" \
    "$extraction/usr/share/doc/$DOC_DIR/derived-projection-v2-source-map.json" \
    "$STAGED_MANIFEST" "$STAGED_SOURCE_MAP" <<'PY'
import os
import pathlib
import stat
import sys

paths = [pathlib.Path(value) for value in sys.argv[1:]]
for path in paths:
    if not path.is_absolute() or ".." in path.parts:
        raise SystemExit(f"error: extracted provenance path is unsafe: {path}")
    current = pathlib.Path(path.anchor)
    for component in path.parts[1:]:
        current /= component
        metadata = os.lstat(current)
        if current == path:
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
                raise SystemExit(f"error: extracted provenance is not a regular file: {path}")
        elif stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise SystemExit(f"error: extracted provenance parent is unsafe: {current}")
if paths[0].read_bytes() != paths[2].read_bytes():
    raise SystemExit("error: packaged source provenance does not match staged bytes")
if paths[1].read_bytes() != paths[3].read_bytes():
    raise SystemExit("error: packaged source map does not match staged bytes")
PY
}

VERIFY_DIR="$TMPDIR/verify"
VERIFY_CONTROL="$TMPDIR/verify-control"
verify_extracted_package "$OUTPUT" "$VERIFY_DIR" "$VERIFY_CONTROL"
python3 "$SAFE_PATH" validate-file --root "$TARGET_ROOT" --path "$DEB"
python3 "$SAFE_PATH" publish-file --root "$TARGET_ROOT" \
  --source "$OUTPUT" --destination "$DEB" --replace

FINAL_DIR="$TMPDIR/final"
FINAL_CONTROL="$TMPDIR/final-control"
verify_extracted_package "$DEB" "$FINAL_DIR" "$FINAL_CONTROL"
