#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
PACKAGE="$ROOT/scripts/package-cli-linux.sh"
SAFE_PATH="$ROOT/scripts/release-safe-path.py"
REAL_LOCK="$ROOT/scripts/cargo-build-lock.sh"
TEST_ROOT="$(mktemp -d)"
trap 'rm -rf "$TEST_ROOT"' EXIT

fail() {
  echo "error: $*" >&2
  exit 1
}

assert_fails() {
  local label="$1"
  shift
  set +e
  "$@" >/dev/null 2>&1
  local status=$?
  set -e
  [[ "$status" -ne 0 ]] || fail "$label unexpectedly succeeded"
}

assert_file_equals() {
  local path="$1" expected="$2" label="$3"
  [[ -f "$path" && ! -L "$path" ]] ||
    fail "$label is missing or unsafe: $path"
  [[ "$(cat "$path")" == "$expected" ]] ||
    fail "$label content changed: $path"
}

make_fixture() {
  local name="$1"
  FIXTURE="$TEST_ROOT/$name"
  FIXTURE_REPO="$FIXTURE/repo"
  FIXTURE_BIN="$FIXTURE/fake-bin"
  FIXTURE_TARGET="$FIXTURE/target root"
  FIXTURE_TEMP_PARENT="$FIXTURE/temp parent"
  mkdir -p "$FIXTURE_REPO/scripts" "$FIXTURE_BIN" \
    "$FIXTURE_TARGET" "$FIXTURE_TEMP_PARENT"
  cp "$PACKAGE" "$FIXTURE_REPO/scripts/package-cli-linux.sh"
  cp "$SAFE_PATH" "$FIXTURE_REPO/scripts/release-safe-path.py"
  cp "$ROOT/scripts/release-source-gate.sh" "$FIXTURE_REPO/scripts/release-source-gate.sh"
  printf '# package safety fixture\n' > "$FIXTURE_REPO/README.md"

  cat > "$FIXTURE_REPO/scripts/cargo-build-lock.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--print-target-dir" ]]; then
  printf '%s\n' "$PACKAGE_TEST_TARGET_ROOT"
  exit 0
fi
if [[ "${1:-}" == "--verify-inherited-lock" ]]; then
  [[ "${KANBAN_CARGO_BUILD_LOCK_HELD:-}" == "1" ]] || exit 2
  [[ "${CARGO_TARGET_DIR:-}" == "$PACKAGE_TEST_TARGET_ROOT" ]] || exit 2
  [[ "${KANBAN_CARGO_BUILD_LOCK_PATH:-}" == "$PACKAGE_TEST_TARGET_ROOT/.build.lock" ]] || exit 2
  lock_fd="${KANBAN_CARGO_BUILD_LOCK_FD:-}"
  [[ "$lock_fd" =~ ^[3-9][0-9]*$ && -e "/proc/self/fd/$lock_fd" ]] || exit 2
  expected="$(stat -Lc '%d:%i:%h:%F' "$PACKAGE_TEST_TARGET_ROOT/.build.lock")" || exit 2
  [[ "$expected" == *":1:regular "* ]] || exit 2
  inherited="$(stat -Lc '%d:%i:%h:%F' "/proc/self/fd/$lock_fd")" || exit 2
  [[ "$inherited" == "$expected" ]] || exit 2
  /usr/bin/flock -n "$lock_fd"
  exit $?
fi
[[ "${1:-}" == "--" ]]
shift
exec "$@"
EOF

  cat > "$FIXTURE_REPO/scripts/package-source-provenance.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  --invalidate-packages|--verify-dep-info) exit 0 ;;
  *) exit 2 ;;
esac
EOF

  cat > "$FIXTURE_BIN/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "$PACKAGE_TEST_CARGO_TRACE"
case "${1:-}" in
  --version)
    printf 'cargo 1.0.0 (fixture)\n'
    ;;
  pkgid)
    printf 'path+file:///package-safety-fixture#1.2.3\n'
    ;;
  metadata)
    printf '%s\n' \
      '{"packages":[{"name":"kanban-cli"},{"name":"kanban-vector-lancedb"},{"name":"kanban-graph-oxigraph"}]}'
    ;;
  build)
    target="$PACKAGE_TEST_TARGET_ROOT/release"
    mkdir -p "$target"
    if [[ " $* " == *' -p kanban-cli '* ]]; then
      printf '#!/usr/bin/env bash\nexit 0\n' > "$target/kanban"
      printf 'dep-info\n' > "$target/kanban.d"
      chmod 0755 "$target/kanban"
    else
      for helper in kanban-vector-lancedb kanban-graph-oxigraph; do
        printf '#!/usr/bin/env bash\nif [[ "${1:-}" == "__build-identity" ]]; then if [[ "${PACKAGE_TEST_HELPER_MISMATCH:-}" == "%s" ]]; then printf wrong-helper-identity; else printf "%%s" "${KANBAN_BUILD_ID:?}"; fi; exit 0; fi\nexit 0\n' "$helper" > "$target/$helper"
        printf 'dep-info\n' > "$target/$helper.d"
        chmod 0755 "$target/$helper"
      done
    fi
    ;;
  *)
    exit 89
    ;;
esac
EOF

  cat > "$FIXTURE_BIN/rustc" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf 'rustc 1.0.0\nhost: x86_64-unknown-linux-gnu\n'
EOF

  cat > "$FIXTURE_BIN/dpkg-shlibdeps" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf 'shlibs:Depends=libc6\n'
EOF

cat > "$FIXTURE_BIN/dpkg-deb" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${PACKAGE_TEST_PROVENANCE:-0}" == "1" ]]; then
  exec /usr/bin/dpkg-deb "$@"
fi
package_root="${@: -2:1}"
output="${@: -1}"
stage="$(dirname "$output")"
case "${PACKAGE_TEST_DPKG_MODE:-normal}" in
  normal)
    printf 'fake deb payload\n' > "$output"
    ;;
  replace-stage-fail)
    mv "$stage" "$PACKAGE_TEST_DETACHED_PATH"
    mkdir -m 0700 "$stage"
    printf 'unknown stage replacement\n' > "$stage/sentinel"
    printf 'unknown replacement artifact\n' > "$output"
    exit 73
    ;;
  replace-temp-signal)
    package_temp="$(dirname "$package_root")"
    mv "$package_temp" "$PACKAGE_TEST_DETACHED_PATH"
    mkdir -m 0700 "$package_temp"
    printf 'unknown temp replacement\n' > "$package_temp/sentinel"
    kill -TERM "$PPID"
    exit 74
    ;;
  *)
    exit 75
    ;;
esac
EOF

  cat > "$FIXTURE_BIN/mktemp" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${PACKAGE_TEST_MKTEMP_SWAP:-0}" != "1" ]]; then
  exec /usr/bin/mktemp "$@"
fi
touch "$PACKAGE_TEST_MKTEMP_CALLED"
path="$PACKAGE_TEST_TEMP_PARENT/.legacy-package-temp"
mkdir -m 0700 "$path"
mv "$path" "$PACKAGE_TEST_DETACHED_PATH"
mkdir -m 0700 "$path"
printf 'unknown pre-observation replacement\n' > "$path/sentinel"
printf '%s\n' "$path"
EOF

  chmod 0755 \
    "$FIXTURE_REPO/scripts/package-cli-linux.sh" \
    "$FIXTURE_REPO/scripts/release-safe-path.py" \
    "$FIXTURE_REPO/scripts/release-source-gate.sh" \
    "$FIXTURE_REPO/scripts/cargo-build-lock.sh" \
    "$FIXTURE_REPO/scripts/package-source-provenance.sh" \
    "$FIXTURE_BIN/cargo" \
    "$FIXTURE_BIN/rustc" \
    "$FIXTURE_BIN/dpkg-shlibdeps" \
    "$FIXTURE_BIN/dpkg-deb" \
    "$FIXTURE_BIN/mktemp"
}

assert_release_provenance_inputs_and_helper_identity() {
  local output="$FIXTURE/provenance-inputs.output"
  local manifest="$FIXTURE/provenance/source-provenance.json"
  local source_map="$FIXTURE_REPO/docs/release/derived-projection-v2-source-map.json"
  local copied_map="$FIXTURE/copied-source-map.json"
  local tampered_map="$FIXTURE/tampered-source-map.json"
  local symlink_manifest="$FIXTURE/symlink-manifest.json"
  local build_id
  mkdir -p "$(dirname "$source_map")"
  cp "$ROOT/docs/release/derived-projection-v2-source-map.json" "$source_map"
  mkdir -p "$(dirname "$manifest")"
  local map_hash
  map_hash="$(sha256sum "$source_map" | awk '{print $1}')"
  python3 - "$manifest" "$map_hash" <<'PY'
import hashlib
import json
import pathlib
import sys

path, map_hash = sys.argv[1:]
commit = "a" * 40
tree = "b" * 40
rustc_vv = "rustc 1.0.0\nhost: x86_64-unknown-linux-gnu"
cargo_version = "cargo 1.0.0 (fixture)"
identity_payload = {
    "cargo_lock": {"path": "Cargo.lock", "sha256": "0" * 64},
    "features": {
        "effective": ["oxigraph-backend", "tantivy-backend"],
        "no_default_features": True,
    },
    "registry_closure": {
        "path": "policy/schema-tool-registry-closure.json",
        "sha256": "1" * 64,
    },
    "target": {
        "deb_arch": "amd64",
        "machine_arch": "x86_64",
        "platform": "Linux",
        "triple": "x86_64-unknown-linux-gnu",
    },
    "toolchain": {
        "cargo_version": cargo_version,
        "cargo_version_sha256": hashlib.sha256(cargo_version.encode()).hexdigest(),
        "rustc_vv": rustc_vv,
        "rustc_vv_sha256": hashlib.sha256(rustc_vv.encode()).hexdigest(),
    },
}
canonical = json.dumps(identity_payload, sort_keys=True, separators=(",", ":"))
identity_sha = hashlib.sha256(canonical.encode()).hexdigest()
identity = dict(identity_payload)
identity["identity_sha256"] = identity_sha
version = "1.2.3"
build_id = f"kanban-tool/{version};commit={commit};tree={tree};identity={identity_sha}"
document = {
    "branch": "main",
    "build_id": build_id,
    "commit": commit,
    "generation_key": f"{commit}-{tree}-{identity_sha}",
    "identity": identity,
    "identity_sha256": identity_sha,
    "project": "kanban-tool",
    "remote": {"commit": commit, "name": "origin", "ref": "refs/heads/main"},
    "schema_version": 3,
    "semantic_source": {
        "name": "origin/derived-projection-v2",
        "no_merge_base_with_main": True,
        "remote_ref": "refs/heads/derived-projection-v2",
        "remote_tip": "c" * 40,
        "saved_ref": "refs/remotes/origin/derived-projection-v2",
        "saved_tip": "c" * 40,
        "verified_source_commits": ["d" * 40, "e" * 40, "f" * 40],
    },
    "source_map": {
        "path": "docs/release/derived-projection-v2-source-map.json",
        "sha256": map_hash,
    },
    "tree": tree,
    "version": version,
}
pathlib.Path(path).write_text(
    json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n"
)
PY
  build_id="$(python3 - "$manifest" <<'PY'
import json
import pathlib
import sys
print(json.loads(pathlib.Path(sys.argv[1]).read_text())["build_id"])
PY
)"

  if ! run_package "$output" env \
    PACKAGE_TEST_PROVENANCE=1 \
    KANBAN_BUILD_ID="$build_id" \
    KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
    KANBAN_RELEASE_SOURCE_MAP="$source_map"; then
    cat "$output" >&2
    fail "provenance-enabled package failed to build"
  fi
  local deb
  deb="$(expected_deb)"
  [[ -f "$deb" ]] || fail "provenance-enabled package was not produced"
  local control="$FIXTURE/control"
  rm -rf "$control"
  mkdir -p "$control"
  /usr/bin/dpkg-deb -e "$deb" "$control"
  [[ "$(grep -Fc 'X-Kanban-Build-Id:' "$control/control")" == "1" ]] ||
    fail "provenance-enabled control does not contain exactly one build identity field"
  grep -Fqx "X-Kanban-Build-Id: $build_id" "$control/control" ||
    fail "provenance-enabled control has the wrong build identity"

  if ! PACKAGE_TEST_AUTO_RESOURCES=1 run_package "$output" env \
    PACKAGE_TEST_PROVENANCE=1 \
    KANBAN_BUILD_ID="$build_id" \
    KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
    KANBAN_RELEASE_SOURCE_MAP="$source_map"; then
    cat "$output" >&2
    fail "provenance-enabled package rejected canonical auto resources"
  fi

  assert_fails "provenance package accepted marker/target lock spoof" \
    run_package_spoofed_lock_environment "$output" env \
    CARGO_BUILD_JOBS=9 \
    PACKAGE_TEST_PROVENANCE=1 \
    KANBAN_BUILD_ID="$build_id" \
    KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
    KANBAN_RELEASE_SOURCE_MAP="$source_map"

  assert_fails "provenance package accepted valid-fd noncanonical CARGO_BUILD_JOBS" \
    run_package "$output" env \
    CARGO_BUILD_JOBS=9 \
    PACKAGE_TEST_PROVENANCE=1 \
    KANBAN_BUILD_ID="$build_id" \
    KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
    KANBAN_RELEASE_SOURCE_MAP="$source_map"

  assert_fails "provenance rejects mismatched helper identity" run_package "$output" env \
    PACKAGE_TEST_PROVENANCE=1 \
    PACKAGE_TEST_HELPER_MISMATCH=kanban-vector-lancedb \
    KANBAN_BUILD_ID="$build_id" \
    KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
    KANBAN_RELEASE_SOURCE_MAP="$source_map"

  cp "$source_map" "$tampered_map"
  printf '\n' >> "$tampered_map"
  assert_fails "provenance rejects source-map hash drift" run_package "$output" env \
    PACKAGE_TEST_PROVENANCE=1 \
    KANBAN_BUILD_ID="$build_id" \
    KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
    KANBAN_RELEASE_SOURCE_MAP="$tampered_map"

  cp "$source_map" "$copied_map"
  assert_fails "provenance rejects noncanonical source-map path" run_package "$output" env \
    PACKAGE_TEST_PROVENANCE=1 \
    KANBAN_BUILD_ID="$build_id" \
    KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
    KANBAN_RELEASE_SOURCE_MAP="$copied_map"

  ln -s "$manifest" "$symlink_manifest"
  assert_fails "provenance rejects symlinked source manifest" run_package "$output" env \
    PACKAGE_TEST_PROVENANCE=1 \
    KANBAN_BUILD_ID="$build_id" \
    KANBAN_RELEASE_SOURCE_MANIFEST="$symlink_manifest" \
    KANBAN_RELEASE_SOURCE_MAP="$source_map"
}

use_real_build_lock() {
  cp "$REAL_LOCK" "$FIXTURE_REPO/scripts/cargo-build-lock.sh"
  chmod 0755 "$FIXTURE_REPO/scripts/cargo-build-lock.sh"
}

run_package() {
  local output="$1"
  local lock_path="$FIXTURE_TARGET/.build.lock"
  shift
  [[ "${1:-}" == "env" ]] || fail "package test runner requires env command"
  shift
  (
    exec 3>"$lock_path"
    /usr/bin/flock -n 3
    export CARGO_TARGET_DIR="$FIXTURE_TARGET"
    export KANBAN_CARGO_BUILD_LOCK_HELD=1
    export KANBAN_CARGO_BUILD_LOCK_PATH="$lock_path"
    export KANBAN_CARGO_BUILD_LOCK_FD=3
    if [[ "${PACKAGE_TEST_AUTO_RESOURCES:-0}" == "1" ]]; then
      export KANBAN_CARGO_BUILD_JOBS=auto KANBAN_TEST_THREADS=auto
      unset CARGO_BUILD_JOBS NEXTEST_TEST_THREADS RUST_TEST_THREADS
    else
      export CARGO_BUILD_JOBS=2
      export NEXTEST_TEST_THREADS=2
      export RUST_TEST_THREADS=2
    fi
    env \
      -u KANBAN_BUILD_ID \
      -u KANBAN_RELEASE_SOURCE_MANIFEST \
      -u KANBAN_RELEASE_SOURCE_MAP \
      PACKAGE_TEST_TARGET_ROOT="$FIXTURE_TARGET" \
      PACKAGE_TEST_CARGO_TRACE="$FIXTURE/cargo.trace" \
      TMPDIR="$FIXTURE_TEMP_PARENT" \
      PATH="$FIXTURE_BIN:$PATH" \
      "$@" "$FIXTURE_REPO/scripts/package-cli-linux.sh" --format deb \
      --no-default-features --features "tantivy-backend,oxigraph-backend"
  ) >"$output" 2>&1
}

run_package_spoofed_lock_environment() {
  local output="$1"
  shift
  env \
    -u KANBAN_CARGO_BUILD_LOCK_PATH \
    -u KANBAN_CARGO_BUILD_LOCK_FD \
    KANBAN_CARGO_BUILD_LOCK_HELD=1 \
    CARGO_TARGET_DIR="$FIXTURE_TARGET" \
    PACKAGE_TEST_TARGET_ROOT="$FIXTURE_TARGET" \
    PACKAGE_TEST_CARGO_TRACE="$FIXTURE/cargo.trace" \
    TMPDIR="$FIXTURE_TEMP_PARENT" \
    PATH="$FIXTURE_BIN:$PATH" \
    "$@" "$FIXTURE_REPO/scripts/package-cli-linux.sh" --format deb \
    --no-default-features --features "tantivy-backend,oxigraph-backend" \
    >"$output" 2>&1
}

run_package_unlocked() {
  local output="$1"
  shift
  env \
    -u CARGO_TARGET_DIR \
    -u KANBAN_CARGO_BUILD_LOCK_HELD \
    -u KANBAN_BUILD_ID \
    -u KANBAN_RELEASE_SOURCE_MANIFEST \
    -u KANBAN_RELEASE_SOURCE_MAP \
    KANBAN_CARGO_TARGET_ROOT="$FIXTURE_TARGET" \
    PACKAGE_TEST_TARGET_ROOT="$FIXTURE_TARGET" \
    PACKAGE_TEST_CARGO_TRACE="$FIXTURE/cargo.trace" \
    TMPDIR="$FIXTURE_TEMP_PARENT" \
    PATH="$FIXTURE_BIN:$PATH" \
    "$@" "$FIXTURE_REPO/scripts/package-cli-linux.sh" --format deb \
    --no-default-features --features "tantivy-backend,oxigraph-backend" \
    >"$output" 2>&1
}

expected_deb() {
  printf '%s\n' \
    "$FIXTURE_TARGET/release/bundle/cli/deb/kanban-tool-cli_1.2.3-1_amd64.deb"
}

assert_legacy_package_with_space_target_succeeds() {
  local output="$FIXTURE/legacy.output"
  run_package "$output" env
  local deb
  deb="$(expected_deb)"
  assert_file_equals "$deb" "fake deb payload" "legacy CLI package"
  [[ -z "$(find "$(dirname "$deb")" -maxdepth 1 \
    -name '.kanban-cli-deb.*' -print -quit)" ]] ||
    fail "successful package retained a private output stage"

  printf 'old regular package\n' > "$deb"
  run_package "$output" env
  assert_file_equals "$deb" "fake deb payload" \
    "atomically replaced legacy CLI package"
  [[ "$(stat -c '%h' "$deb")" == "1" ]] ||
    fail "atomically replaced package is multiply linked"
}

assert_physical_source_root_rejects_in_tree_target() {
  local link="$FIXTURE/repo-link"
  local output="$FIXTURE/in-tree.output"
  local physical_target="$FIXTURE_REPO/target-inside-source"
  mkdir -p "$physical_target"
  ln -s "$FIXTURE_REPO" "$link"
  FIXTURE_TARGET="$physical_target"
  set +e
  env \
    -u CARGO_TARGET_DIR \
    -u KANBAN_BUILD_ID \
    -u KANBAN_RELEASE_SOURCE_MANIFEST \
    -u KANBAN_RELEASE_SOURCE_MAP \
    KANBAN_CARGO_BUILD_LOCK_HELD=1 \
    PACKAGE_TEST_TARGET_ROOT="$FIXTURE_TARGET" \
    PACKAGE_TEST_CARGO_TRACE="$FIXTURE/cargo.trace" \
    TMPDIR="$FIXTURE_TEMP_PARENT" \
    PATH="$FIXTURE_BIN:$PATH" \
    "$link/scripts/package-cli-linux.sh" --format deb >"$output" 2>&1
  status=$?
  set -e
  [[ "$status" -ne 0 ]] ||
    fail "symlink invocation accepted a physical target inside the source tree"
  [[ ! -e "$physical_target/release/kanban" ]] ||
    fail "in-source target rejection occurred after build output mutation"
}

assert_symlinked_safe_path_is_rejected() {
  local output="$FIXTURE/symlink-safe-path.output"
  mv "$FIXTURE_REPO/scripts/release-safe-path.py" \
    "$FIXTURE_REPO/scripts/release-safe-path-real.py"
  ln -s release-safe-path-real.py "$FIXTURE_REPO/scripts/release-safe-path.py"
  set +e
  run_package "$output" env
  status=$?
  set -e
  [[ "$status" -ne 0 ]] ||
    fail "package accepted a symlinked release-safe-path helper"
  [[ ! -e "$(expected_deb)" ]] ||
    fail "symlinked helper rejection published a package"
}

assert_symlinked_target_root_is_rejected() {
  local output="$FIXTURE/symlink-target.output"
  local physical_target="$FIXTURE/physical-target"
  mkdir -p "$physical_target"
  mv "$FIXTURE_TARGET" "$FIXTURE/unused-target"
  ln -s "$physical_target" "$FIXTURE_TARGET"
  set +e
  run_package "$output" env
  status=$?
  set -e
  [[ "$status" -ne 0 ]] ||
    fail "package accepted a symlinked Cargo target root"
  [[ ! -e "$physical_target/release" ]] ||
    fail "symlinked target root rejection mutated the physical target"
}

assert_unlocked_nonexistent_space_target_succeeds() {
  local output="$FIXTURE/unlocked-space-target.output"
  FIXTURE_TARGET="$FIXTURE/nonexistent target root"
  use_real_build_lock
  [[ ! -e "$FIXTURE_TARGET" ]] ||
    fail "unlocked success fixture target unexpectedly exists"

  run_package_unlocked "$output" env
  assert_file_equals "$(expected_deb)" "fake deb payload" \
    "unlocked CLI package"
  [[ -f "$FIXTURE_TARGET/.build.lock" &&
    ! -L "$FIXTURE_TARGET/.build.lock" ]] ||
    fail "real lock entry did not create its expected build lock"

  run_package_unlocked "$output" env
  assert_file_equals "$(expected_deb)" "fake deb payload" \
    "unlocked CLI package with existing regular build lock"
}

assert_unlocked_leaf_symlink_is_rejected_before_lock() {
  local output="$FIXTURE/unlocked-leaf-symlink.output"
  local outside="$FIXTURE/outside-leaf-target"
  use_real_build_lock
  mkdir -p "$outside"
  printf 'outside leaf sentinel\n' > "$outside/sentinel"
  rmdir "$FIXTURE_TARGET"
  ln -s "$outside" "$FIXTURE_TARGET"

  set +e
  run_package_unlocked "$output" env
  local status=$?
  set -e
  [[ "$status" -ne 0 ]] ||
    fail "unlocked package accepted a symlinked Cargo target leaf"
  [[ -L "$FIXTURE_TARGET" ]] ||
    fail "unlocked package replaced the symlinked Cargo target leaf"
  assert_file_equals "$outside/sentinel" "outside leaf sentinel" \
    "outside leaf target sentinel"
  [[ ! -e "$outside/.build.lock" && ! -e "$outside/release" ]] ||
    fail "leaf symlink rejection occurred after real lock target mutation"
}

assert_unlocked_parent_symlink_is_rejected_before_lock() {
  local output="$FIXTURE/unlocked-parent-symlink.output"
  local outside_parent="$FIXTURE/outside-target-parent"
  local outside_target="$outside_parent/nested target"
  local linked_parent="$FIXTURE/linked-target-parent"
  use_real_build_lock
  mkdir -p "$outside_target"
  printf 'outside parent sentinel\n' > "$outside_target/sentinel"
  ln -s "$outside_parent" "$linked_parent"
  FIXTURE_TARGET="$linked_parent/nested target"

  set +e
  run_package_unlocked "$output" env
  local status=$?
  set -e
  [[ "$status" -ne 0 ]] ||
    fail "unlocked package accepted a symlinked Cargo target parent"
  [[ -L "$linked_parent" ]] ||
    fail "unlocked package replaced the symlinked Cargo target parent"
  assert_file_equals "$outside_target/sentinel" "outside parent sentinel" \
    "outside parent target sentinel"
  [[ ! -e "$outside_target/.build.lock" && ! -e "$outside_target/release" ]] ||
    fail "parent symlink rejection occurred after real lock target mutation"
}

assert_unlocked_in_source_target_is_rejected_before_lock() {
  local output="$FIXTURE/unlocked-in-source.output"
  use_real_build_lock
  FIXTURE_TARGET="$FIXTURE_REPO/unsafe target root"
  [[ ! -e "$FIXTURE_TARGET" ]] ||
    fail "unlocked in-source fixture target unexpectedly exists"

  set +e
  run_package_unlocked "$output" env
  local status=$?
  set -e
  [[ "$status" -ne 0 ]] ||
    fail "unlocked package accepted a Cargo target inside the source tree"
  assert_file_equals "$FIXTURE_REPO/README.md" "# package safety fixture" \
    "source tree sentinel"
  [[ ! -e "$FIXTURE_TARGET" ]] ||
    fail "in-source target rejection occurred after real lock target mutation"
}

assert_unlocked_parent_traversal_is_rejected_before_lock() {
  local output="$FIXTURE/unlocked-parent-traversal.output"
  local missing_parent="$FIXTURE/uncreated target parent"
  local normalized_target="$FIXTURE/traversal-target"
  use_real_build_lock
  printf 'parent traversal sentinel\n' > "$FIXTURE/traversal-sentinel"
  FIXTURE_TARGET="$missing_parent/../traversal-target"

  set +e
  run_package_unlocked "$output" env
  local status=$?
  set -e
  [[ "$status" -ne 0 ]] ||
    fail "unlocked package accepted parent traversal in the Cargo target"
  assert_file_equals "$FIXTURE/traversal-sentinel" \
    "parent traversal sentinel" "parent traversal sentinel"
  [[ ! -e "$missing_parent" && ! -e "$normalized_target/.build.lock" &&
    ! -e "$normalized_target/release" ]] ||
    fail "parent traversal rejection occurred after real lock target mutation"
}

assert_unlocked_double_slash_source_target_is_rejected_before_lock() {
  local output="$FIXTURE/unlocked-double-slash-source.output"
  local source_relative="${FIXTURE_REPO#/}"
  local physical_target="$FIXTURE_REPO/double-slash unsafe target"
  use_real_build_lock
  FIXTURE_TARGET="//$source_relative/double-slash unsafe target"

  set +e
  run_package_unlocked "$output" env
  local status=$?
  set -e
  [[ "$status" -ne 0 ]] ||
    fail "unlocked package accepted a double-slash source-tree Cargo target"
  assert_file_equals "$FIXTURE_REPO/README.md" "# package safety fixture" \
    "double-slash source tree sentinel"
  [[ ! -e "$physical_target/.build.lock" &&
    ! -e "$physical_target/release" && ! -e "$physical_target" ]] ||
    fail "double-slash source target rejection occurred after real lock mutation"
}

assert_unlocked_unsafe_build_lock_is_rejected_before_open() {
  local kind="$1"
  local output="$FIXTURE/unlocked-build-lock-$kind.output"
  local sentinel="$FIXTURE/build-lock-$kind-sentinel"
  local lock_path="$FIXTURE_TARGET/.build.lock"
  use_real_build_lock
  printf 'outside build lock sentinel\n' > "$sentinel"
  case "$kind" in
    symlink)
      ln -s "$sentinel" "$lock_path"
      ;;
    hardlink)
      ln "$sentinel" "$lock_path"
      ;;
    *)
      fail "unknown unsafe build lock fixture: $kind"
      ;;
  esac

  set +e
  run_package_unlocked "$output" env
  local status=$?
  set -e
  [[ "$status" -ne 0 ]] ||
    fail "unlocked package accepted an unsafe $kind build lock"
  assert_file_equals "$sentinel" "outside build lock sentinel" \
    "outside $kind build lock sentinel"
  case "$kind" in
    symlink)
      [[ -L "$lock_path" && "$(readlink "$lock_path")" == "$sentinel" ]] ||
        fail "symlinked build lock changed before rejection"
      ;;
    hardlink)
      assert_file_equals "$lock_path" "outside build lock sentinel" \
        "hardlinked build lock"
      [[ "$(stat -c '%h' "$sentinel")" == "2" ]] ||
        fail "hardlinked build lock link count changed before rejection"
      ;;
  esac
  [[ ! -e "$FIXTURE_TARGET/release" ]] ||
    fail "unsafe $kind build lock rejection occurred after release mutation"
}

assert_target_layout_entry_is_rejected() {
  local kind="$1"
  local output="$FIXTURE/target-layout-$kind.output"
  local outside="$FIXTURE/outside-$kind"
  local entry directory_name
  mkdir -p "$outside"
  printf 'external target sentinel\n' > "$outside/sentinel"
  case "$kind" in
    release)
      entry="$FIXTURE_TARGET/release"
      ln -s "$outside" "$entry"
      ;;
    fingerprint|build|deps)
      mkdir -p "$FIXTURE_TARGET/release"
      directory_name="$kind"
      [[ "$directory_name" == fingerprint ]] && directory_name=.fingerprint
      entry="$FIXTURE_TARGET/release/$directory_name"
      ln -s "$outside" "$entry"
      ;;
    *)
      fail "unknown target layout entry: $kind"
      ;;
  esac

  set +e
  run_package "$output" env
  local status=$?
  set -e
  [[ "$status" -ne 0 ]] ||
    fail "package accepted a symlinked Cargo release entry: $kind"
  [[ -L "$entry" ]] ||
    fail "symlinked Cargo release entry was replaced: $entry"
  assert_file_equals "$outside/sentinel" "external target sentinel" \
    "external target sentinel ($kind)"
  [[ ! -e "$FIXTURE_TARGET/release/kanban" ]] ||
    fail "symlinked Cargo release entry failure occurred after binary mutation: $kind"
}

assert_target_layout_nonregular_root_is_rejected() {
  local kind="$1"
  local output="$FIXTURE/target-layout-nonregular-$kind.output"
  local entry directory_name
  case "$kind" in
    release)
      entry="$FIXTURE_TARGET/release"
      printf 'not a release directory\n' > "$entry"
      ;;
    fingerprint|build|deps)
      mkdir -p "$FIXTURE_TARGET/release"
      directory_name="$kind"
      [[ "$directory_name" == fingerprint ]] && directory_name=.fingerprint
      entry="$FIXTURE_TARGET/release/$directory_name"
      printf 'not a Cargo directory\n' > "$entry"
      ;;
    *)
      fail "unknown target layout entry: $kind"
      ;;
  esac

  set +e
  run_package "$output" env
  local status=$?
  set -e
  [[ "$status" -ne 0 ]] ||
    fail "package accepted a non-directory Cargo release entry: $kind"
  [[ -f "$entry" ]] ||
    fail "non-directory Cargo release entry was replaced: $entry"
  [[ ! -e "$FIXTURE_TARGET/release/kanban" ]] ||
    fail "non-directory Cargo release entry failure occurred after binary mutation: $kind"
}

assert_existing_destination_is_preserved() {
  local kind="$1"
  local output="$FIXTURE/existing-$kind.output"
  local deb sentinel outside
  deb="$(expected_deb)"
  mkdir -p "$(dirname "$deb")"
  case "$kind" in
    hardlink)
      sentinel="$FIXTURE/hardlink-sentinel"
      printf 'hardlink sentinel\n' > "$sentinel"
      ln "$sentinel" "$deb"
      ;;
    symlink)
      sentinel="$FIXTURE/symlink-sentinel"
      printf 'symlink sentinel\n' > "$sentinel"
      ln -s "$sentinel" "$deb"
      ;;
    nonregular)
      mkdir "$deb"
      printf 'directory sentinel\n' > "$deb/sentinel"
      ;;
    parent-symlink)
      rmdir "$(dirname "$deb")"
      outside="$FIXTURE/outside-output"
      mkdir "$outside"
      printf 'outside sentinel\n' > "$outside/sentinel"
      ln -s "$outside" "$(dirname "$deb")"
      ;;
    *)
      fail "unknown destination fixture: $kind"
      ;;
  esac

  set +e
  run_package "$output" env
  status=$?
  set -e
  [[ "$status" -ne 0 ]] ||
    fail "package overwrote an existing $kind destination"
  case "$kind" in
    hardlink)
      assert_file_equals "$sentinel" "hardlink sentinel" "hardlink sentinel"
      assert_file_equals "$deb" "hardlink sentinel" "hardlinked destination"
      [[ "$(stat -c '%h' "$sentinel")" == "2" ]] ||
        fail "hardlinked destination link count changed"
      ;;
    symlink)
      [[ -L "$deb" && "$(readlink "$deb")" == "$sentinel" ]] ||
        fail "symlink destination changed"
      assert_file_equals "$sentinel" "symlink sentinel" "symlink sentinel"
      ;;
    nonregular)
      assert_file_equals "$deb/sentinel" "directory sentinel" \
        "nonregular destination sentinel"
      ;;
    parent-symlink)
      [[ -L "$(dirname "$deb")" ]] ||
        fail "symlinked output parent changed"
      assert_file_equals "$outside/sentinel" "outside sentinel" \
        "outside output sentinel"
      ;;
  esac
}

assert_pre_observation_temp_replacement_is_not_adopted() {
  local output="$FIXTURE/pre-observation-temp.output"
  local called="$FIXTURE/mktemp-called"
  local detached="$FIXTURE/detached-pre-observation-temp"
  set +e
  run_package "$output" env \
    PACKAGE_TEST_MKTEMP_SWAP=1 \
    PACKAGE_TEST_MKTEMP_CALLED="$called" \
    PACKAGE_TEST_TEMP_PARENT="$FIXTURE_TEMP_PARENT" \
    PACKAGE_TEST_DETACHED_PATH="$detached"
  status=$?
  set -e
  [[ "$status" -eq 0 ]] ||
    fail "identity-bound temp fixture failed unexpectedly: status=$status"
  [[ ! -e "$called" ]] ||
    fail "package used an unbound mktemp directory"
}

assert_replaced_stage_is_retained_on_failure() {
  local output="$FIXTURE/replaced-stage.output"
  local detached="$FIXTURE/detached-stage"
  set +e
  run_package "$output" env \
    PACKAGE_TEST_DPKG_MODE=replace-stage-fail \
    PACKAGE_TEST_DETACHED_PATH="$detached"
  status=$?
  set -e
  [[ "$status" -ne 0 ]] ||
    fail "replaced output stage fixture unexpectedly succeeded"
  local stage
  stage="$(
    find "$(dirname "$(expected_deb)")" -maxdepth 1 \
      -name '.kanban-cli-deb.*' -type d -print -quit
  )"
  [[ -n "$stage" ]] || fail "replacement output stage was deleted"
  assert_file_equals "$stage/sentinel" "unknown stage replacement" \
    "replacement output stage"
  [[ -d "$detached" ]] || fail "owned output stage was not detached by fixture"
}

assert_signal_cleanup_preserves_replaced_temp() {
  local output="$FIXTURE/replaced-temp-signal.output"
  local detached="$FIXTURE/detached-package-temp"
  set +e
  run_package "$output" env \
    PACKAGE_TEST_DPKG_MODE=replace-temp-signal \
    PACKAGE_TEST_DETACHED_PATH="$detached"
  status=$?
  set -e
  [[ "$status" -ne 0 ]] ||
    fail "signal failure fixture unexpectedly succeeded"
  local replacement
  replacement="$(
    find "$FIXTURE_TEMP_PARENT" -maxdepth 1 \
      -name '.kanban-cli-package.*' -type d -print -quit
  )"
  [[ -n "$replacement" ]] ||
    fail "signal cleanup deleted the replacement package temp directory"
  assert_file_equals "$replacement/sentinel" "unknown temp replacement" \
    "replacement package temp"
  [[ -d "$detached/deb-root" ]] ||
    fail "signal fixture did not retain the detached owned package temp"
}

make_fixture legacy
assert_legacy_package_with_space_target_succeeds

make_fixture unlocked-space-target
assert_unlocked_nonexistent_space_target_succeeds

make_fixture unlocked-leaf-symlink
assert_unlocked_leaf_symlink_is_rejected_before_lock

make_fixture unlocked-parent-symlink
assert_unlocked_parent_symlink_is_rejected_before_lock

make_fixture unlocked-in-source
assert_unlocked_in_source_target_is_rejected_before_lock

make_fixture unlocked-parent-traversal
assert_unlocked_parent_traversal_is_rejected_before_lock

make_fixture unlocked-double-slash-source
assert_unlocked_double_slash_source_target_is_rejected_before_lock

for build_lock_kind in symlink hardlink; do
  make_fixture "unlocked-build-lock-$build_lock_kind"
  assert_unlocked_unsafe_build_lock_is_rejected_before_open "$build_lock_kind"
done

make_fixture physical-root
assert_physical_source_root_rejects_in_tree_target

make_fixture helper-symlink
assert_symlinked_safe_path_is_rejected

make_fixture target-symlink
assert_symlinked_target_root_is_rejected

for target_layout_kind in release fingerprint build deps; do
  make_fixture "target-layout-symlink-$target_layout_kind"
  assert_target_layout_entry_is_rejected "$target_layout_kind"
done

for target_layout_kind in release fingerprint build deps; do
  make_fixture "target-layout-nonregular-$target_layout_kind"
  assert_target_layout_nonregular_root_is_rejected "$target_layout_kind"
done

for destination_kind in hardlink symlink nonregular parent-symlink; do
  make_fixture "destination-$destination_kind"
  assert_existing_destination_is_preserved "$destination_kind"
done

make_fixture pre-observation-temp
assert_pre_observation_temp_replacement_is_not_adopted

make_fixture replaced-stage
assert_replaced_stage_is_retained_on_failure

make_fixture signal-temp
assert_signal_cleanup_preserves_replaced_temp

make_fixture provenance-inputs
assert_release_provenance_inputs_and_helper_identity

echo "ok: CLI package paths, private stages, and cleanup identities are safe"
