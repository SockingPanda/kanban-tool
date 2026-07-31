#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_GATE="$ROOT/scripts/release-source-gate.sh"
COHORT_WRAPPER="$ROOT/scripts/release-cohort.sh"
ARTIFACT_MANIFEST="$ROOT/scripts/release-artifact-manifest.sh"
EMBED_DEB="$ROOT/scripts/embed-release-provenance-deb.sh"
PACKAGE_CLI="$ROOT/scripts/package-cli-linux.sh"
SAFE_PATH="$ROOT/scripts/release-safe-path.py"
TMPROOT="$(mktemp -d)"
trap 'rm -rf "$TMPROOT"' EXIT

fail() {
  echo "error: $*" >&2
  exit 1
}

assert_fails() {
  local label="$1"
  shift
  local output status
  set +e
  output="$("$@" 2>&1)"
  status=$?
  set -e
  [[ "$status" -ne 0 ]] || fail "$label unexpectedly succeeded"
  printf '%s\n' "$output"
}

make_fake_repo() {
  local repo="$1"
  mkdir -p "$repo/scripts" "$repo/docs/release" "$repo/fake-bin" \
    "$repo/allow-bin" "$repo/output"
  cp "$SOURCE_GATE" "$repo/scripts/release-source-gate.sh"
  cp "$COHORT_WRAPPER" "$repo/scripts/release-cohort.sh"
  cp "$ARTIFACT_MANIFEST" "$repo/scripts/release-artifact-manifest.sh"
  cp "$EMBED_DEB" "$repo/scripts/embed-release-provenance-deb.sh"
  if [[ -f "$ROOT/scripts/release-safe-path.py" ]]; then
    cp "$ROOT/scripts/release-safe-path.py" "$repo/scripts/release-safe-path.py"
  fi
  cp "$ROOT/docs/release/derived-projection-v2-source-map.json" \
    "$repo/docs/release/derived-projection-v2-source-map.json"
  cp "$ROOT/justfile" "$repo/justfile"

  cat > "$repo/scripts/trace-release-exec.py" <<'PY'
#!/usr/bin/python3
import json
import os
import sys

record = {
    "argv": sys.argv[2:],
    "kind": sys.argv[1],
}
line = json.dumps(record, separators=(",", ":"), sort_keys=True) + "\n"
descriptor = os.open(
    os.environ["FAKE_RELEASE_EXEC_LOG"],
    os.O_WRONLY | os.O_CREAT | os.O_APPEND,
    0o600,
)
try:
    os.write(descriptor, line.encode())
finally:
    os.close(descriptor)
PY
  chmod +x "$repo/scripts/trace-release-exec.py"
  local allowed_tool resolved_tool
  for allowed_tool in \
    awk bash basename cat chmod cmp cp dirname env find grep install mkdir \
    mktemp python3 rm sha256sum stat tar gzip xz
  do
    resolved_tool="$(command -v "$allowed_tool")"
    [[ "$resolved_tool" == /* ]] ||
      fail "release test allowlist tool is unavailable: $allowed_tool"
    ln -s "$resolved_tool" "$repo/allow-bin/$allowed_tool"
  done

  cat > "$repo/Cargo.toml" <<'EOF'
[workspace]
[workspace.package]
version = "2.1.3"
EOF

  printf 'fixture lock for release identity\n' > "$repo/Cargo.lock"
  mkdir -p "$repo/policy"
  cp "$ROOT/policy/schema-tool-registry-closure.json" \
    "$repo/policy/schema-tool-registry-closure.json"

  cat > "$repo/fake-bin/rustc" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ "${1:-}" == "-vV" ]] || { echo "unexpected fake rustc invocation: $*" >&2; exit 98; }
printf '%s\n' "${FAKE_RUSTC_VV:-rustc 1.0.0
binary: rustc
commit-hash: fixture
commit-date: 1970-01-01
host: x86_64-unknown-linux-gnu
release: 1.0.0
LLVM version: 1.0.0}"
EOF
  cat > "$repo/fake-bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ "${1:-}" == "--version" ]] || { echo "unexpected fake cargo invocation: $*" >&2; exit 98; }
printf '%s\n' "${FAKE_CARGO_VERSION:-cargo 1.0.0 (fixture)}"
EOF
  cat > "$repo/fake-bin/uname" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  -s) printf '%s\n' "${FAKE_UNAME_S:-Linux}" ;;
  -m) printf '%s\n' "${FAKE_UNAME_M:-x86_64}" ;;
  *) echo "unexpected fake uname invocation: $*" >&2; exit 98 ;;
esac
EOF
  chmod +x "$repo/fake-bin/rustc" "$repo/fake-bin/cargo" "$repo/fake-bin/uname"

cat > "$repo/fake-bin/git" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ -n "${FAKE_TRACE_WRITER:-}" && -n "${FAKE_RELEASE_EXEC_LOG:-}" ]]; then
  /usr/bin/python3 "$FAKE_TRACE_WRITER" git "$@"
fi

if [[ "${1:-}" == "-C" ]]; then
  [[ $# -ge 3 ]] || exit 99
  [[ "$2" == "${FAKE_GIT_ROOT:?}" ]] || {
    echo "unexpected fake git root: $2" >&2
    exit 99
  }
  shift 2
fi

case "$*" in
  "rev-parse --show-toplevel")
    printf '%s\n' "${FAKE_GIT_ROOT:?}"
    ;;
  "symbolic-ref --quiet HEAD")
    [[ "${FAKE_GIT_DETACHED:-0}" != "1" ]] || exit 1
    if [[ -n "${FAKE_GIT_STATE_DIR:-}" && -f "$FAKE_GIT_STATE_DIR/branch" ]]; then
      printf 'refs/heads/%s\n' "$(cat "$FAKE_GIT_STATE_DIR/branch")"
    else
      printf 'refs/heads/%s\n' "${FAKE_GIT_BRANCH:-main}"
    fi
    ;;
  "status --porcelain=v1 --untracked-files=all")
    if [[ "${FAKE_MUTATE_STAGED_AFTER_HASH:-0}" == "1" && -n "${FAKE_TARGET_ROOT:-}" ]]; then
      staged="$(
        find "$FAKE_TARGET_ROOT/release/bundle/cohort" -type f \
          -path '*/artifacts/bin/kanban' -print -quit 2>/dev/null || true
      )"
      if [[ -n "$staged" && ! -f "${FAKE_GIT_STATE_DIR:?}/artifact-mutated" ]]; then
        chmod u+w "$staged"
        printf 'mutation after hash\n' >> "$staged"
        : > "$FAKE_GIT_STATE_DIR/artifact-mutated"
      fi
    fi
    status="${FAKE_GIT_STATUS:-clean}"
    if [[ -n "${FAKE_GIT_STATE_DIR:-}" && -f "$FAKE_GIT_STATE_DIR/status" ]]; then
      status="$(cat "$FAKE_GIT_STATE_DIR/status")"
    fi
    case "$status" in
      clean) ;;
      dirty) printf ' M tracked-file\n' ;;
      untracked) printf '?? untracked-file\n' ;;
      *) exit 97 ;;
    esac
    ;;
  "rev-parse --verify HEAD^{commit}")
    if [[ -n "${FAKE_GIT_STATE_DIR:-}" && -f "$FAKE_GIT_STATE_DIR/commit" ]]; then
      cat "$FAKE_GIT_STATE_DIR/commit"
    else
      printf '%s\n' "${FAKE_GIT_COMMIT:?}"
    fi
    ;;
  "rev-parse --verify HEAD^{tree}")
    if [[ -n "${FAKE_GIT_STATE_DIR:-}" && -f "$FAKE_GIT_STATE_DIR/tree" ]]; then
      cat "$FAKE_GIT_STATE_DIR/tree"
    else
      printf '%s\n' "${FAKE_GIT_TREE:?}"
    fi
    ;;
  archive\ --format=tar\ --prefix=source/\ *)
    # The real wrapper archives the pinned commit.  This hermetic fixture has
    # no Git object database, so archive only the fixture's versioned release
    # inputs into the same `source/` tar prefix.
    archive_fixture="${FAKE_GIT_STATE_DIR:?}/fixture-source.tar"
    tar -cf "$archive_fixture" --format=ustar --transform='s,^,source/,' \
      -C "${FAKE_GIT_ROOT:?}" Cargo.toml Cargo.lock justfile policy scripts docs
    if [[ "${FAKE_MUTATE_LIVE_SOURCE_GATE:-0}" == "1" ]]; then
      cp "${FAKE_GIT_ROOT:?}/scripts/release-source-gate.sh" \
        "${FAKE_GIT_STATE_DIR:?}/live-source-gate-original"
      printf '%s\n' \
        '#!/usr/bin/env bash' \
        'set -euo pipefail' \
        ': > "${FAKE_GIT_STATE_DIR:?}/live-source-gate-invoked"' \
        'cp "${FAKE_GIT_STATE_DIR:?}/live-source-gate-original" "${FAKE_GIT_ROOT:?}/scripts/release-source-gate.sh"' \
        ': > "${FAKE_GIT_STATE_DIR:?}/live-source-gate-restored"' \
        'exit 0' \
        >"${FAKE_GIT_ROOT:?}/scripts/release-source-gate.sh"
      chmod 0755 "${FAKE_GIT_ROOT:?}/scripts/release-source-gate.sh"
      : > "${FAKE_GIT_STATE_DIR:?}/live-source-gate-mutated"
    fi
    cat "$archive_fixture"
    rm -f "$archive_fixture"
    ;;
  "rev-parse --verify refs/remotes/origin/derived-projection-v2^{commit}")
    if [[ -n "${FAKE_GIT_STATE_DIR:-}" && -f "$FAKE_GIT_STATE_DIR/saved-source" ]]; then
      cat "$FAKE_GIT_STATE_DIR/saved-source"
    else
      printf '%s\n' "${FAKE_GIT_SAVED_SOURCE_TIP:?}"
    fi
    ;;
  rev-parse\ --verify\ *^\{commit\})
    value="$*"
    value="${value#rev-parse --verify }"
    printf '%s\n' "${value%\^\{commit\}}"
    ;;
  merge-base\ --is-ancestor\ *\ HEAD)
    value="$*"
    value="${value#merge-base --is-ancestor }"
    value="${value% HEAD}"
    [[ "${FAKE_GIT_REJECT_ANCESTOR:-}" != "$value" ]] || exit 1
    ;;
  merge-base\ --is-ancestor\ *\ c764706fcae214f58a3a65f5dc565135522bbd81)
    value="$*"
    value="${value#merge-base --is-ancestor }"
    value="${value% c764706fcae214f58a3a65f5dc565135522bbd81}"
    [[ "${FAKE_GIT_REJECT_SOURCE_ANCESTOR:-}" != "$value" ]] || exit 1
    ;;
  "merge-base HEAD c764706fcae214f58a3a65f5dc565135522bbd81")
    if [[ "${FAKE_GIT_HAS_MERGE_BASE:-0}" == "1" ]]; then
      printf '%s\n' "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
      exit 0
    fi
    exit 1
    ;;
  "ls-remote --exit-code origin refs/heads/main")
    [[ "${FAKE_GIT_REMOTE_FAIL:-0}" != "1" ]] || exit 2
    if [[ -n "${FAKE_GIT_STATE_DIR:-}" && -f "$FAKE_GIT_STATE_DIR/remote" ]]; then
      printf '%s\trefs/heads/main\n' "$(cat "$FAKE_GIT_STATE_DIR/remote")"
    else
      printf '%s\trefs/heads/main\n' "${FAKE_GIT_REMOTE_COMMIT:?}"
    fi
    ;;
  "ls-remote --exit-code origin refs/heads/derived-projection-v2")
    [[ "${FAKE_GIT_SOURCE_REMOTE_FAIL:-0}" != "1" ]] || exit 2
    if [[ -n "${FAKE_GIT_STATE_DIR:-}" && -f "$FAKE_GIT_STATE_DIR/remote-source" ]]; then
      printf '%s\trefs/heads/derived-projection-v2\n' \
        "$(cat "$FAKE_GIT_STATE_DIR/remote-source")"
    else
      printf '%s\trefs/heads/derived-projection-v2\n' \
        "${FAKE_GIT_REMOTE_SOURCE_TIP:?}"
    fi
    ;;
  *)
    echo "unexpected fake git invocation: $*" >&2
    exit 98
    ;;
esac
EOF
  cat > "$repo/scripts/cargo-build-lock.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ -n "${FAKE_TRACE_WRITER:-}" && -n "${FAKE_RELEASE_EXEC_LOG:-}" ]]; then
  /usr/bin/python3 "$FAKE_TRACE_WRITER" build-lock "$@"
fi
if [[ "$*" == "--print-target-dir" ]]; then
  printf 'target\t%s\n' "${KANBAN_CARGO_BUILD_LOCK_HELD:-0}" \
    >> "${FAKE_LOCK_LOG:?}"
  printf '%s\n' "${FAKE_TARGET_ROOT:?}"
  exit 0
fi
[[ "${1:-}" == "--" && $# -ge 2 ]] || {
  echo "unexpected fake build lock invocation: $*" >&2
  exit 98
}
shift
printf 'exclusive\t%s\n' "${KANBAN_CARGO_BUILD_LOCK_HELD:-0}" \
  >> "${FAKE_LOCK_LOG:?}"
KANBAN_CARGO_BUILD_LOCK_HELD=1 exec "$@"
EOF

  cat > "$repo/fake-bin/just" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ -n "${FAKE_TRACE_WRITER:-}" && -n "${FAKE_RELEASE_EXEC_LOG:-}" ]]; then
  /usr/bin/python3 "$FAKE_TRACE_WRITER" just "$@"
fi

args=("$@")
recipe_index=0
while [[ "$recipe_index" -lt "${#args[@]}" ]]; do
  if [[ "${args[$recipe_index]}" == "--justfile" ]]; then
    recipe_index=$((recipe_index + 2))
  else
    break
  fi
done
recipe="${args[*]:$recipe_index}"
[[ -n "$recipe" ]] || { echo "missing fake just recipe" >&2; exit 98; }

printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
  "$recipe" "${KANBAN_BUILD_ID:-}" "${KANBAN_RELEASE_SOURCE_MANIFEST:-}" \
  "${KANBAN_RELEASE_SOURCE_MAP:-}" "${KANBAN_CARGO_BUILD_LOCK_HELD:-0}" \
  "${KANBAN_RELEASE_FEATURES:-}" "${KANBAN_RELEASE_NO_DEFAULT_FEATURES:-}" \
  "${CARGO_BUILD_TARGET:-}" \
  >> "${FAKE_JUST_LOG:?}"

release="${FAKE_TARGET_ROOT:?}/release"
build_id="${KANBAN_BUILD_ID:?}"
case "$recipe" in
  cli-package)
    mkdir -p "$release/bundle/cli/deb"
    for binary in kanban kanban-vector-lancedb kanban-graph-oxigraph; do
      if [[ "$binary" == kanban-vector-lancedb || "$binary" == kanban-graph-oxigraph ]]; then
        runtime_id="$build_id"
        if [[ "${FAKE_HELPER_RUNTIME_MISMATCH:-}" == "$binary" ]]; then
          runtime_id="wrong-runtime-build-id"
        fi
        cat > "$release/$binary" <<EOF_HELPER
#!/usr/bin/env bash
# embedded cohort: $build_id
if [[ "\${1:-}" == "__build-identity" ]]; then
  if [[ -n "\${FAKE_TRACE_WRITER:-}" && -n "\${FAKE_RELEASE_EXEC_LOG:-}" ]]; then
    /usr/bin/python3 "\$FAKE_TRACE_WRITER" helper "$binary" "\$@"
  fi
  printf '%s' '$runtime_id'
  exit 0
fi
exit 64
EOF_HELPER
      elif [[ "${FAKE_MISSING_HELPER_BUILD_ID:-}" == "$binary" ]]; then
        printf '#!/usr/bin/env bash\nexit 0\n' > "$release/$binary"
      else
        printf '#!/usr/bin/env bash\n# fake binary %s %s\nexit 0\n' \
          "$binary" "$build_id" > "$release/$binary"
      fi
      chmod +x "$release/$binary"
    done
    deb="$release/bundle/cli/deb/kanban-tool-cli_2.1.3-1_amd64.deb"
    root="${FAKE_CLI_PACKAGE_ROOT:?}"
    rm -rf "$root"
    mkdir -p "$root/usr/bin" "$root/usr/lib/kanban" \
      "$root/usr/share/doc/kanban-tool-cli" "$root/DEBIAN"
    chmod 0755 "$root" "$root/DEBIAN"
    cp "$release/kanban" "$root/usr/bin/kanban"
    cp "$release/kanban-vector-lancedb" "$root/usr/lib/kanban/kanban-vector-lancedb"
    cp "$release/kanban-graph-oxigraph" "$root/usr/lib/kanban/kanban-graph-oxigraph"
    cp "$KANBAN_RELEASE_SOURCE_MANIFEST" \
      "$root/usr/share/doc/kanban-tool-cli/source-provenance.json"
    cp "$KANBAN_RELEASE_SOURCE_MAP" \
      "$root/usr/share/doc/kanban-tool-cli/derived-projection-v2-source-map.json"
    cat > "$root/DEBIAN/control" <<'EOF_CONTROL'
Package: kanban-tool-cli
Version: 2.1.3-1
Architecture: amd64
Maintainer: release-test <release-test@example.invalid>
Description: fake CLI release package
EOF_CONTROL
    dpkg-deb --root-owner-group --build "$root" "$deb" >/dev/null
    if [[ "${FAKE_CORRUPT_CLI_DEB:-0}" == "1" ]]; then
      printf 'corrupt cohort deb bytes\n' > "$deb"
    fi
    ;;
  desktop-package)
    printf '#!/usr/bin/env bash\n# fake binary kanban-desktop %s\nexit 0\n' \
      "$build_id" > "$release/kanban-desktop"
    chmod +x "$release/kanban-desktop"
    mkdir -p "$release/bundle/deb"
    deb="$release/bundle/deb/Kanban Tool_2.1.3_amd64.deb"
    root="${FAKE_DESKTOP_PACKAGE_ROOT:?}"
    rm -rf "$root"
    mkdir -p "$root/usr/bin" "$root/DEBIAN"
    chmod 0755 "$root" "$root/DEBIAN"
    cp "$release/kanban-desktop" "$root/usr/bin/kanban-desktop"
    cp "$release/kanban-vector-lancedb" "$root/usr/bin/kanban-vector-lancedb"
    cp "$release/kanban-graph-oxigraph" "$root/usr/bin/kanban-graph-oxigraph"
    cat > "$root/DEBIAN/control" <<'EOF_CONTROL'
Package: kanban-tool
Version: 2.1.3
Architecture: amd64
Maintainer: release-test <release-test@example.invalid>
Description: fake desktop release package
EOF_CONTROL
    dpkg-deb --root-owner-group --build "$root" "$deb" >/dev/null
    ;;
  desktop-package-layout)
    deb="${FAKE_TARGET_ROOT:?}/release/bundle/deb/Kanban Tool_2.1.3_amd64.deb"
    extracted="$(mktemp -d)"
    trap 'rm -rf "$extracted"' EXIT
    dpkg-deb -x "$deb" "$extracted"
    [[ -f "$extracted/usr/share/doc/kanban-tool-desktop/source-provenance.json" ]]
    [[ -f "$extracted/usr/share/doc/kanban-tool-desktop/derived-projection-v2-source-map.json" ]]
    ;;
esac

if [[ "$recipe" == "desktop-package" && "${FAKE_MUTATE_LIVE_EMBED:-0}" == "1" ]]; then
  cp "${FAKE_GIT_ROOT:?}/scripts/embed-release-provenance-deb.sh" \
    "${FAKE_GIT_STATE_DIR:?}/live-embed-original"
  printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    ': > "${FAKE_GIT_STATE_DIR:?}/live-embed-invoked"' \
    'exit 91' \
    >"${FAKE_GIT_ROOT:?}/scripts/embed-release-provenance-deb.sh"
  chmod 0755 "${FAKE_GIT_ROOT:?}/scripts/embed-release-provenance-deb.sh"
  : > "${FAKE_GIT_STATE_DIR:?}/live-embed-mutated"
fi

if [[ -n "${FAKE_DRIFT_AFTER:-}" && "$recipe" == "$FAKE_DRIFT_AFTER" ]]; then
  printf '%s\n' "4444444444444444444444444444444444444444" \
    > "${FAKE_GIT_STATE_DIR:?}/tree"
fi
EOF

  cat > "$repo/fake-bin/dpkg-deb" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ -n "${FAKE_TRACE_WRITER:-}" && -n "${FAKE_RELEASE_EXEC_LOG:-}" ]]; then
  /usr/bin/python3 "$FAKE_TRACE_WRITER" dpkg-deb "$@"
fi
exec /usr/bin/dpkg-deb "$@"
EOF
  chmod +x "$repo/fake-bin/git" "$repo/fake-bin/just" \
    "$repo/fake-bin/dpkg-deb" "$repo/scripts/cargo-build-lock.sh" \
    "$repo/scripts/release-source-gate.sh" "$repo/scripts/release-cohort.sh" \
    "$repo/scripts/release-artifact-manifest.sh" \
    "$repo/scripts/embed-release-provenance-deb.sh"
}

run_gate() {
  local repo="$1"
  shift
  env \
    PATH="$repo/fake-bin:$repo/allow-bin" \
    FAKE_RELEASE_EXEC_LOG="$repo/output/source-exec.jsonl" \
    FAKE_TRACE_WRITER="$repo/scripts/trace-release-exec.py" \
    FAKE_GIT_ROOT="$repo" \
    FAKE_GIT_COMMIT="${FAKE_GIT_COMMIT:-1111111111111111111111111111111111111111}" \
    FAKE_GIT_TREE="${FAKE_GIT_TREE:-2222222222222222222222222222222222222222}" \
    FAKE_GIT_REMOTE_COMMIT="${FAKE_GIT_REMOTE_COMMIT:-1111111111111111111111111111111111111111}" \
    FAKE_GIT_BRANCH="${FAKE_GIT_BRANCH:-main}" \
    FAKE_GIT_STATUS="${FAKE_GIT_STATUS:-clean}" \
    FAKE_GIT_REMOTE_FAIL="${FAKE_GIT_REMOTE_FAIL:-0}" \
    FAKE_GIT_SOURCE_REMOTE_FAIL="${FAKE_GIT_SOURCE_REMOTE_FAIL:-0}" \
    FAKE_GIT_REMOTE_SOURCE_TIP="${FAKE_GIT_REMOTE_SOURCE_TIP:-c764706fcae214f58a3a65f5dc565135522bbd81}" \
    FAKE_GIT_SAVED_SOURCE_TIP="${FAKE_GIT_SAVED_SOURCE_TIP:-c764706fcae214f58a3a65f5dc565135522bbd81}" \
    FAKE_GIT_REJECT_ANCESTOR="${FAKE_GIT_REJECT_ANCESTOR:-}" \
    FAKE_GIT_REJECT_SOURCE_ANCESTOR="${FAKE_GIT_REJECT_SOURCE_ANCESTOR:-}" \
    FAKE_GIT_HAS_MERGE_BASE="${FAKE_GIT_HAS_MERGE_BASE:-0}" \
    "$repo/scripts/release-source-gate.sh" "$@"
}

assert_source_gate() {
  local repo="$TMPROOT/source-gate"
  local output="$TMPROOT/source-gate-output"
  local manifest="$output/source-provenance.json"
  make_fake_repo "$repo"
  mkdir -p "$output"

  run_gate "$repo" prepare --output "$manifest"
  python3 - "$manifest" <<'PY'
import json
import pathlib
import sys

manifest = json.loads(pathlib.Path(sys.argv[1]).read_text())
assert set(manifest) == {
    "branch", "build_id", "commit", "generation_key", "identity",
    "identity_sha256", "project", "remote", "schema_version",
    "semantic_source", "source_map", "tree", "version",
}
assert manifest["schema_version"] == 3
assert manifest["commit"] == "1111111111111111111111111111111111111111"
assert manifest["tree"] == "2222222222222222222222222222222222222222"
assert manifest["identity_sha256"] == manifest["identity"]["identity_sha256"]
assert manifest["generation_key"] == (
    manifest["commit"] + "-" + manifest["tree"] + "-" + manifest["identity_sha256"]
)
assert manifest["build_id"].endswith(";identity=" + manifest["identity_sha256"])
assert manifest["identity"]["cargo_lock"]["path"] == "Cargo.lock"
assert manifest["identity"]["registry_closure"]["path"] == "policy/schema-tool-registry-closure.json"
assert manifest["identity"]["features"] == {
    "effective": ["oxigraph-backend", "tantivy-backend"],
    "no_default_features": True,
}
assert manifest["identity"]["target"] == {
    "deb_arch": "amd64",
    "machine_arch": "x86_64",
    "platform": "Linux",
    "triple": "x86_64-unknown-linux-gnu",
}
assert manifest["identity"]["toolchain"]["cargo_version"] == "cargo 1.0.0 (fixture)"
assert "rustc 1.0.0" in manifest["identity"]["toolchain"]["rustc_vv"]
assert len(manifest["source_map"]["sha256"]) == 64
assert manifest["source_map"]["sha256"].isalnum()
PY

  cp "$manifest" "$output/first.json"
  run_gate "$repo" prepare --output "$manifest"
  cmp -s "$output/first.json" "$manifest" ||
    fail "source manifest is not deterministic"
  run_gate "$repo" verify --manifest "$manifest"

  FAKE_GIT_STATUS=dirty assert_fails "tracked dirty source" \
    run_gate "$repo" prepare --output "$output/dirty.json" >/dev/null
  FAKE_GIT_STATUS=untracked assert_fails "untracked source" \
    run_gate "$repo" prepare --output "$output/untracked.json" >/dev/null
  FAKE_GIT_BRANCH=feature assert_fails "wrong branch" \
    run_gate "$repo" prepare --output "$output/branch.json" >/dev/null
  FAKE_GIT_REMOTE_COMMIT=3333333333333333333333333333333333333333 \
    assert_fails "remote mismatch" \
    run_gate "$repo" prepare --output "$output/remote.json" >/dev/null
  FAKE_GIT_REMOTE_FAIL=1 assert_fails "ls-remote failure" \
    run_gate "$repo" prepare --output "$output/remote-fail.json" >/dev/null
  FAKE_GIT_REMOTE_SOURCE_TIP=3333333333333333333333333333333333333333 \
    assert_fails "derived source remote mismatch" \
    run_gate "$repo" prepare --output "$output/source-remote.json" >/dev/null
  FAKE_GIT_SAVED_SOURCE_TIP=3333333333333333333333333333333333333333 \
    assert_fails "saved derived source mismatch" \
    run_gate "$repo" prepare --output "$output/source-saved.json" >/dev/null
  FAKE_GIT_SOURCE_REMOTE_FAIL=1 assert_fails "derived source ls-remote failure" \
    run_gate "$repo" prepare --output "$output/source-remote-fail.json" >/dev/null
  FAKE_GIT_REJECT_SOURCE_ANCESTOR=85e1f797c2d5c2b089d1a2f29827f42e25cd595c \
    assert_fails "source commit ancestry on saved tip" \
    run_gate "$repo" prepare --output "$output/source-ancestor.json" >/dev/null
  FAKE_GIT_HAS_MERGE_BASE=1 assert_fails "main/source unexpectedly have merge base" \
    run_gate "$repo" prepare --output "$output/merge-base.json" >/dev/null
  FAKE_GIT_REJECT_ANCESTOR=095a5c2ee88434976ae7f8c8bf8310c8227eec70 \
    assert_fails "source-map integrated commit ancestry" \
    run_gate "$repo" prepare --output "$output/ancestor.json" >/dev/null
  RUSTC_WRAPPER=/tmp/unrecorded-wrapper assert_fails "rustc wrapper override" \
    run_gate "$repo" prepare --output "$output/rustc-wrapper.json" >/dev/null
  RUSTC_WORKSPACE_WRAPPER=/tmp/unrecorded-workspace-wrapper \
    assert_fails "rustc workspace wrapper override" \
    run_gate "$repo" prepare --output "$output/rustc-workspace-wrapper.json" >/dev/null
  CARGO_BUILD_TARGET=aarch64-unknown-linux-gnu assert_fails "cargo target override" \
    run_gate "$repo" prepare --output "$output/cargo-target.json" >/dev/null
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/tmp/linker \
    assert_fails "cargo target linker override" \
    run_gate "$repo" prepare --output "$output/cargo-target-linker.json" >/dev/null
  KANBAN_RELEASE_NO_DEFAULT_FEATURES=0 assert_fails "default feature mode override" \
    run_gate "$repo" prepare --output "$output/default-features.json" >/dev/null
  KANBAN_RELEASE_FEATURES=tantivy-backend assert_fails "feature set override" \
    run_gate "$repo" prepare --output "$output/features.json" >/dev/null

  FAKE_GIT_TREE=4444444444444444444444444444444444444444 \
    assert_fails "tree drift" run_gate "$repo" verify --manifest "$manifest" >/dev/null
  assert_fails "source-tree output" \
    run_gate "$repo" prepare --output "$repo/output/forbidden.json" >/dev/null
}

initialize_git_state() {
  local state="$1"
  mkdir -p "$state"
  printf '%s\n' main > "$state/branch"
  printf '%s\n' clean > "$state/status"
  printf '%s\n' "1111111111111111111111111111111111111111" > "$state/commit"
  printf '%s\n' "2222222222222222222222222222222222222222" > "$state/tree"
  printf '%s\n' "1111111111111111111111111111111111111111" > "$state/remote"
  printf '%s\n' "c764706fcae214f58a3a65f5dc565135522bbd81" > "$state/remote-source"
  printf '%s\n' "c764706fcae214f58a3a65f5dc565135522bbd81" > "$state/saved-source"
}

run_wrapper() {
  local repo="$1" target="$2" state="$3" log="$4"
  shift 4
  env -u KANBAN_BUILD_ID -u KANBAN_RELEASE_SOURCE_MANIFEST \
    -u KANBAN_RELEASE_SOURCE_MAP \
    PATH="$repo/fake-bin:$repo/allow-bin" \
    FAKE_TRACE_WRITER="$repo/scripts/trace-release-exec.py" \
    FAKE_GIT_ROOT="$repo" \
    FAKE_GIT_STATE_DIR="$state" \
    FAKE_GIT_COMMIT="1111111111111111111111111111111111111111" \
    FAKE_GIT_TREE="2222222222222222222222222222222222222222" \
    FAKE_GIT_REMOTE_COMMIT="1111111111111111111111111111111111111111" \
    FAKE_GIT_REMOTE_SOURCE_TIP="c764706fcae214f58a3a65f5dc565135522bbd81" \
    FAKE_GIT_SAVED_SOURCE_TIP="c764706fcae214f58a3a65f5dc565135522bbd81" \
    FAKE_TARGET_ROOT="$target" \
    FAKE_LOCK_LOG="$target/release-lock.log" \
    FAKE_JUST_LOG="$log" \
    FAKE_RELEASE_EXEC_LOG="$target/release-exec.jsonl" \
    FAKE_CLI_PACKAGE_ROOT="$target/fake-cli-root" \
    FAKE_DESKTOP_PACKAGE_ROOT="$target/fake-desktop-root" \
    "$@" \
    "$repo/scripts/release-cohort.sh"
}

assert_live_embed_mutation_is_ignored() {
  local repo="$TMPROOT/cohort-live-embed-race-repo"
  local target="$TMPROOT/cohort-live-embed-race-target"
  local state="$TMPROOT/cohort-live-embed-race-state"
  local log="$TMPROOT/cohort-live-embed-race.log"
  local output="$TMPROOT/cohort-live-embed-race.output"
  make_fake_repo "$repo"
  initialize_git_state "$state"
  mkdir -p "$target"
  set +e
  run_wrapper "$repo" "$target" "$state" "$log" env \
    FAKE_MUTATE_LIVE_EMBED=1 >"$output" 2>&1
  local status=$?
  set -e
  if [[ -f "$state/live-embed-original" ]]; then
    cp "$state/live-embed-original" "$repo/scripts/embed-release-provenance-deb.sh"
  fi
  [[ "$status" -eq 0 ]] || {
    cat "$output" >&2
    fail "live embed replacement affected sealed release execution"
  }
  [[ ! -e "$state/live-embed-invoked" ]] ||
    fail "release cohort executed a mutable live embed script"
  [[ -e "$state/live-embed-mutated" ]] ||
    fail "live embed mutation fixture did not run"
}

assert_live_source_gate_mutation_is_ignored() {
  local repo="$TMPROOT/cohort-live-source-gate-race-repo"
  local target="$TMPROOT/cohort-live-source-gate-race-target"
  local state="$TMPROOT/cohort-live-source-gate-race-state"
  local log="$TMPROOT/cohort-live-source-gate-race.log"
  local output="$TMPROOT/cohort-live-source-gate-race.output"
  make_fake_repo "$repo"
  initialize_git_state "$state"
  mkdir -p "$target"
  set +e
  run_wrapper "$repo" "$target" "$state" "$log" env \
    FAKE_MUTATE_LIVE_SOURCE_GATE=1 >"$output" 2>&1
  local status=$?
  set -e
  if [[ -f "$state/live-source-gate-original" ]]; then
    cp "$state/live-source-gate-original" "$repo/scripts/release-source-gate.sh"
  fi
  [[ "$status" -eq 0 ]] || {
    cat "$output" >&2
    fail "live source-gate replacement affected sealed release execution"
  }
  [[ ! -e "$state/live-source-gate-invoked" ]] ||
    fail "release cohort executed a mutable live source-gate script"
  [[ -e "$state/live-source-gate-mutated" ]] ||
    fail "live source-gate mutation fixture did not run"
  # A vulnerable live-gate invocation self-restores after recording the
  # bypass.  The sealed path must leave no invocation marker; restore the
  # fixture manually only as cleanup for that safe case.
}

assert_cohort_wrapper() {
  local repo="$TMPROOT/cohort-wrapper"
  local target="$TMPROOT/cohort-target"
  local state="$TMPROOT/cohort-state"
  local log="$TMPROOT/cohort-just.log"
  make_fake_repo "$repo"
  initialize_git_state "$state"
  mkdir -p "$target"

  run_wrapper "$repo" "$target" "$state" "$log" env

  [[ -s "$target/release-exec.jsonl" ]] ||
    fail "release wrapper did not produce a unified executable trace"

  python3 - "$log" "$target" "$repo" "$target/release-lock.log" \
    "$target/release-exec.jsonl" <<'PY'
import hashlib
import json
import pathlib
import re
import sys

log = pathlib.Path(sys.argv[1])
target = pathlib.Path(sys.argv[2])
repo = pathlib.Path(sys.argv[3])
lock_log = pathlib.Path(sys.argv[4])
exec_log = pathlib.Path(sys.argv[5])
expected = [
    "affected-self-test",
    "schema-contract",
    "audit",
    "rust-full",
    "check-windows-p kanban-local",
    "projection-release-cohort",
    "bench-check",
    "target-tools",
    "cli-package",
    "cli-package-layout",
    "desktop-package-config",
    "desktop-package",
    "desktop-package-layout",
    "smoke",
    "diff-check",
]
rows = [line.split("\t") for line in log.read_text().splitlines()]
assert [row[0] for row in rows] == expected
cohort_root = target / "release/bundle/cohort"
published_candidates = [
    path for path in cohort_root.iterdir()
    if path.is_dir() and not path.name.startswith(".cohort-stage.")
]
assert len(published_candidates) == 1
published = published_candidates[0]
source_document = json.loads((published / "source-provenance.json").read_text())
build_id = source_document["build_id"]
assert all(row[1] == build_id for row in rows)
assert all(pathlib.Path(row[2]).name == "source-provenance.json" for row in rows)
assert len({row[2] for row in rows}) == 1
assert all(pathlib.Path(row[3]).name == "derived-projection-v2-source-map.json" for row in rows)
assert all(row[4] == "1" for row in rows)
assert all(row[5] == "tantivy-backend,oxigraph-backend" for row in rows)
assert all(row[6] == "1" for row in rows)
assert all(row[7] == "x86_64-unknown-linux-gnu" for row in rows)
lock_rows = lock_log.read_text().splitlines()
assert lock_rows[0] == "exclusive\t0"
assert lock_rows.count("exclusive\t0") == 1
assert all(row == "exclusive\t0" or row == "target\t1" for row in lock_rows)

exec_rows = []
for line in exec_log.read_text(encoding="utf-8").splitlines():
    row = json.loads(line)
    assert set(row) == {"argv", "kind"}
    assert isinstance(row["argv"], list)
    normalized_argv = []
    for value in row["argv"]:
        assert isinstance(value, str)
        value = value.replace(str(repo), "<REPO>").replace(str(target), "<TARGET>")
        value = re.sub(r"/tmp/tmp\.[^/]+", "<TMP>", value)
        value = re.sub(
            r"\.embed-release-provenance\.[0-9a-f]+",
            ".embed-release-provenance.<ID>",
            value,
        )
        value = re.sub(r"/proc/self/fd/[0-9]+", "<PINNED_FD>", value)
        normalized_argv.append(value)
    exec_rows.append({"argv": normalized_argv, "kind": row["kind"]})
assert len(exec_rows) == 181
assert {row["kind"] for row in exec_rows} == {
    "build-lock",
    "dpkg-deb",
    "git",
    "helper",
    "just",
}

exec_payload = json.dumps(
    exec_rows,
    sort_keys=True,
    separators=(",", ":"),
).encode()
assert hashlib.sha256(exec_payload).hexdigest() == (
    "2375833bab73d2ec939835286700047f85020d7768770dd4de79d206b7edf04a"
)

generations = [path for path in cohort_root.iterdir() if path.is_dir()]
assert len(generations) == 1
published = generations[0]
marker = published.with_name(published.name + ".published")
assert marker.is_file()
assert marker.read_text(encoding="utf-8").strip().split("\t")[0] == "kanban-release-v1"
source_manifest = published / "source-provenance.json"
artifacts = json.loads((published / "release-artifacts.json").read_text())
assert artifacts["build_id"] == build_id
assert artifacts["generation_path"] == (
    "release/bundle/cohort/" + artifacts["generation_key"]
)
assert {item["role"] for item in artifacts["artifacts"]} == {
    "cli_binary",
    "lancedb_helper",
    "oxigraph_helper",
    "desktop_binary",
    "cli_deb",
    "desktop_deb",
}
assert all(item["build_id"] == build_id for item in artifacts["artifacts"])
for item in artifacts["artifacts"]:
    path = target / item["path"]
    assert path.is_file()
    assert hashlib.sha256(path.read_bytes()).hexdigest() == item["sha256"]
    assert path.stat().st_size == item["size"]
PY

  local published
  published="$(
    find "$target/release/bundle/cohort" -mindepth 1 -maxdepth 1 -type d \
      -print -quit
  )"
  [[ -n "$published" ]] || fail "release cohort did not publish one generation"
  local bad_path_stage="$TMPROOT/cohort-noncanonical-path-stage"
  cp -a "$published" "$bad_path_stage"
  chmod -R u+w "$bad_path_stage"
  python3 - "$bad_path_stage/release-artifacts.json" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
document = json.loads(path.read_text(encoding="utf-8"))
document["generation_path"] = "release/bundle/other/" + document["generation_key"]
path.write_text(json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n")
PY
  local bad_path_manifest="$bad_path_stage/release-artifacts.json"
  local bad_path_build_id
  bad_path_build_id="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["build_id"])' "$published/release-artifacts.json")"
  assert_fails "noncanonical generation path" \
    env PATH="$repo/fake-bin:$repo/allow-bin" \
      FAKE_TARGET_ROOT="$target" \
      FAKE_LOCK_LOG="$target/noncanonical-path-lock.log" \
      FAKE_GIT_ROOT="$repo" \
      FAKE_GIT_STATE_DIR="$state" \
      KANBAN_BUILD_ID="$bad_path_build_id" \
      KANBAN_RELEASE_GENERATION_KEY="$(basename "$published")" \
      "$repo/scripts/release-artifact-manifest.sh" verify \
        --manifest "$bad_path_manifest" --stage-dir "$bad_path_stage" >/dev/null
  for redundant_path in \
    "release//bundle/cohort/$(basename "$published")" \
    "release/./bundle/cohort/$(basename "$published")"; do
    python3 - "$bad_path_manifest" "$redundant_path" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
document = json.loads(path.read_text(encoding="utf-8"))
document["generation_path"] = sys.argv[2]
path.write_text(json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n")
PY
    assert_fails "redundant-separator generation path" \
      env PATH="$repo/fake-bin:$repo/allow-bin" \
        FAKE_TARGET_ROOT="$target" \
        FAKE_LOCK_LOG="$target/redundant-path-lock.log" \
        FAKE_GIT_ROOT="$repo" \
        FAKE_GIT_STATE_DIR="$state" \
        KANBAN_BUILD_ID="$bad_path_build_id" \
        KANBAN_RELEASE_GENERATION_KEY="$(basename "$published")" \
        "$repo/scripts/release-artifact-manifest.sh" verify \
          --manifest "$bad_path_manifest" --stage-dir "$bad_path_stage" >/dev/null
  done
  local desktop_extract desktop_deb
  desktop_extract="$TMPROOT/cohort-desktop-extract"
  desktop_deb="$(
    find "$target/release/bundle/deb" -maxdepth 1 -type f \
      -name 'Kanban Tool_*.deb' -print -quit
  )"
  mkdir -p "$desktop_extract"
  /usr/bin/dpkg-deb -x "$desktop_deb" "$desktop_extract"
  cmp -s "$desktop_extract/usr/share/doc/kanban-tool-desktop/source-provenance.json" \
    "$published/source-provenance.json" ||
    fail "desktop package did not receive source provenance"
  cmp -s \
    "$desktop_extract/usr/share/doc/kanban-tool-desktop/derived-projection-v2-source-map.json" \
    "$repo/docs/release/derived-projection-v2-source-map.json" ||
    fail "desktop package did not receive the semantic source map"

  local drift_repo="$TMPROOT/cohort-drift-repo"
  local drift_target="$TMPROOT/cohort-drift-target"
  local drift_state="$TMPROOT/cohort-drift-state"
  local drift_log="$TMPROOT/cohort-drift.log"
  make_fake_repo "$drift_repo"
  initialize_git_state "$drift_state"
  mkdir -p "$drift_target"
  assert_fails "cohort tree drift" \
    run_wrapper "$drift_repo" "$drift_target" "$drift_state" "$drift_log" \
    env FAKE_DRIFT_AFTER=diff-check >/dev/null
  [[ -z "$(find "$drift_target/release/bundle/cohort" -mindepth 1 -maxdepth 1 \
    -type d ! -name '.cohort-stage.*' ! -name '.cohort-source.*' -print -quit 2>/dev/null || true)" ]] ||
    fail "tree drift published a final release cohort"

  local helper_repo="$TMPROOT/cohort-helper-repo"
  local helper_target="$TMPROOT/cohort-helper-target"
  local helper_state="$TMPROOT/cohort-helper-state"
  local helper_log="$TMPROOT/cohort-helper.log"
  make_fake_repo "$helper_repo"
  initialize_git_state "$helper_state"
  mkdir -p "$helper_target"
  assert_fails "helper without cohort build id" \
    run_wrapper "$helper_repo" "$helper_target" "$helper_state" "$helper_log" \
    env FAKE_HELPER_RUNTIME_MISMATCH=kanban-vector-lancedb >/dev/null
  [[ -z "$(find "$helper_target/release/bundle/cohort" -mindepth 1 -maxdepth 1 \
    -type d ! -name '.cohort-stage.*' ! -name '.cohort-source.*' -print -quit 2>/dev/null || true)" ]] ||
    fail "runtime-unbound helper published a final release cohort"

  local mutation_repo="$TMPROOT/cohort-mutation-repo"
  local mutation_target="$TMPROOT/cohort-mutation-target"
  local mutation_state="$TMPROOT/cohort-mutation-state"
  local mutation_log="$TMPROOT/cohort-mutation.log"
  make_fake_repo "$mutation_repo"
  initialize_git_state "$mutation_state"
  mkdir -p "$mutation_target"
  assert_fails "artifact mutation after hashing" \
    run_wrapper "$mutation_repo" "$mutation_target" "$mutation_state" "$mutation_log" \
    env FAKE_MUTATE_STAGED_AFTER_HASH=1 >/dev/null
  [[ -f "$mutation_state/artifact-mutated" ]] ||
    fail "after-hash mutation fixture did not mutate a staged artifact"
  [[ -z "$(find "$mutation_target/release/bundle/cohort" -mindepth 1 -maxdepth 1 \
    -type d ! -name '.cohort-stage.*' ! -name '.cohort-source.*' -print -quit 2>/dev/null || true)" ]] ||
    fail "after-hash mutation published a final release cohort"

  local corrupt_repo="$TMPROOT/cohort-corrupt-deb-repo"
  local corrupt_target="$TMPROOT/cohort-corrupt-deb-target"
  local corrupt_state="$TMPROOT/cohort-corrupt-deb-state"
  local corrupt_log="$TMPROOT/cohort-corrupt-deb.log"
  make_fake_repo "$corrupt_repo"
  initialize_git_state "$corrupt_state"
  mkdir -p "$corrupt_target"
  assert_fails "artifact gate must read the actual Debian bytes" \
    run_wrapper "$corrupt_repo" "$corrupt_target" "$corrupt_state" "$corrupt_log" \
    env FAKE_CORRUPT_CLI_DEB=1 >/dev/null
  [[ -z "$(find "$corrupt_target/release/bundle/cohort" -mindepth 1 -maxdepth 1 \
    -type d ! -name '.cohort-stage.*' ! -name '.cohort-source.*' -print -quit 2>/dev/null || true)" ]] ||
    fail "corrupt Debian bytes published a final release cohort"

  local final_gap_repo="$TMPROOT/cohort-final-gap-repo"
  local final_gap_target="$TMPROOT/cohort-final-gap-target"
  local final_gap_state="$TMPROOT/cohort-final-gap-state"
  local final_gap_log="$TMPROOT/cohort-final-gap.log"
  local final_gap_output="$TMPROOT/cohort-final-gap.output"
  local pause_marker="$TMPROOT/cohort-final-gap.pause"
  local continue_marker="$TMPROOT/cohort-final-gap.continue"
  local writer_started="$TMPROOT/cohort-final-gap.writer-started"
  local writer_done="$TMPROOT/cohort-final-gap.writer-done"
  local wrapper_pid wrapper_status writer_pid staged
  make_fake_repo "$final_gap_repo"
  initialize_git_state "$final_gap_state"
  mkdir -p "$final_gap_target"
  run_wrapper \
    "$final_gap_repo" "$final_gap_target" "$final_gap_state" "$final_gap_log" \
    env \
      KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_AT=publish-dir-after-final-digest \
      KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_MARKER="$pause_marker" \
      KANBAN_RELEASE_SAFE_PATH_TEST_CONTINUE="$continue_marker" \
      >"$final_gap_output" 2>&1 &
  wrapper_pid=$!
  for _ in {1..3000}; do
    [[ -f "$pause_marker" ]] && break
    if ! kill -0 "$wrapper_pid" 2>/dev/null; then
      wait "$wrapper_pid" || true
      sed -n '1,240p' "$final_gap_output" >&2
      fail "final verify/publish fixture exited before reaching its mutation hook"
    fi
    sleep 0.01
  done
  if [[ ! -f "$pause_marker" ]]; then
    kill -TERM "$wrapper_pid" 2>/dev/null || true
    wait "$wrapper_pid" || true
    sed -n '1,240p' "$final_gap_output" >&2
    fail "final verify/publish fixture did not reach its mutation hook"
  fi
  staged="$(
    find "$final_gap_target/release/bundle/cohort" -type f \
      -path '*/artifacts/bin/kanban' -print -quit
  )"
  [[ -n "$staged" ]] || fail "final verify/publish fixture has no staged CLI"
  (
    : > "$writer_started"
    chmod u+w "$staged"
    printf 'mutation after final semantic verify\n' >> "$staged"
    : > "$writer_done"
  ) &
  writer_pid=$!
  for _ in {1..3000}; do
    [[ -f "$writer_started" ]] && break
    kill -0 "$writer_pid" 2>/dev/null ||
      fail "post-digest writer exited before attempting the mutation"
    sleep 0.01
  done
  [[ -f "$writer_started" ]] || fail "post-digest writer never started"
  sleep 0.1
  [[ ! -f "$writer_done" ]] ||
    fail "kernel lease did not exclude a post-digest content writer"
  : > "$continue_marker"
  set +e
  wait "$wrapper_pid"
  wrapper_status=$?
  wait "$writer_pid"
  set -e
  [[ -f "$writer_done" ]] ||
    fail "blocked post-digest writer did not resume after lease rollback"
  [[ "$wrapper_status" -ne 0 ]] ||
    fail "mutation after final semantic verify unexpectedly published"
  grep -Fq \
    'release snapshot write lease break requested (SIGIO); publication aborted' \
    "$final_gap_output" ||
    fail "post-digest writer failure did not prove SIGIO/F_GETLEASE abort"
  [[ -z "$(find "$final_gap_target/release/bundle/cohort" -mindepth 1 -maxdepth 1 \
    -type d ! -name '.cohort-stage.*' ! -name '.cohort-source.*' -print -quit 2>/dev/null || true)" ]] ||
    fail "final verify/publish mutation left a published generation"

  local fsync_repo="$TMPROOT/cohort-fsync-repo"
  local fsync_target="$TMPROOT/cohort-fsync-target"
  local fsync_state="$TMPROOT/cohort-fsync-state"
  local fsync_log="$TMPROOT/cohort-fsync.log"
  make_fake_repo "$fsync_repo"
  initialize_git_state "$fsync_state"
  mkdir -p "$fsync_target"
  assert_fails "late nested fsync failure" \
    run_wrapper "$fsync_repo" "$fsync_target" "$fsync_state" "$fsync_log" \
    env KANBAN_RELEASE_SAFE_PATH_TEST_FAIL_AT=durable-tree-directory >/dev/null
  [[ -z "$(find "$fsync_target/release/bundle/cohort" -mindepth 1 -maxdepth 1 \
    -type d ! -name '.cohort-stage.*' ! -name '.cohort-source.*' -print -quit 2>/dev/null || true)" ]] ||
    fail "late nested fsync failure published a release generation"

  local publish_fsync_repo="$TMPROOT/cohort-publish-fsync-repo"
  local publish_fsync_target="$TMPROOT/cohort-publish-fsync-target"
  local publish_fsync_state="$TMPROOT/cohort-publish-fsync-state"
  local publish_fsync_log="$TMPROOT/cohort-publish-fsync.log"
  make_fake_repo "$publish_fsync_repo"
  initialize_git_state "$publish_fsync_state"
  mkdir -p "$publish_fsync_target"
  assert_fails "post-rename generation parent fsync failure" \
    run_wrapper \
      "$publish_fsync_repo" "$publish_fsync_target" \
      "$publish_fsync_state" "$publish_fsync_log" \
      env KANBAN_RELEASE_SAFE_PATH_TEST_FAIL_AT=publish-dir-parent >/dev/null
  [[ -z "$(find "$publish_fsync_target/release/bundle/cohort" \
    -mindepth 1 -maxdepth 1 -type d ! -name '.cohort-stage.*' \
      ! -name '.cohort-source.*' \
    -print -quit 2>/dev/null || true)" ]] ||
    fail "generation parent fsync failure escaped atomic rollback"

  local crash_repo="$TMPROOT/cohort-crash-resume-repo"
  local crash_target="$TMPROOT/cohort-crash-resume-target"
  local crash_state="$TMPROOT/cohort-crash-resume-state"
  local crash_log="$TMPROOT/cohort-crash-resume.log"
  local crash_output="$TMPROOT/cohort-crash-resume.output"
  make_fake_repo "$crash_repo"
  initialize_git_state "$crash_state"
  mkdir -p "$crash_target"
  assert_fails "real wrapper crash after generation rename" \
    run_wrapper "$crash_repo" "$crash_target" "$crash_state" "$crash_log" \
      env KANBAN_RELEASE_SAFE_PATH_TEST_EXIT_AT=publish-dir-after-rename \
      >"$crash_output"
  published="$(find "$crash_target/release/bundle/cohort" -mindepth 1 -maxdepth 1 \
    -type d -name '1111111111111111111111111111111111111111-2222222222222222222222222222222222222222-*' \
    -print -quit)"
  [[ -d "$published" && ! -e "$published.published" &&
    -f "$published.publishing" ]] ||
    fail "wrapper crash did not leave a durable non-authoritative intent"
  run_wrapper "$crash_repo" "$crash_target" "$crash_state" "$crash_log" env
  [[ -d "$published" && -f "$published.published" &&
    ! -e "$published.publishing" ]] ||
    fail "wrapper retry did not safely resume the durable release intent"

  local committed_crash_repo="$TMPROOT/cohort-committed-crash-resume-repo"
  local committed_crash_target="$TMPROOT/cohort-committed-crash-resume-target"
  local committed_crash_state="$TMPROOT/cohort-committed-crash-resume-state"
  local committed_crash_log="$TMPROOT/cohort-committed-crash-resume.log"
  local committed_crash_output="$TMPROOT/cohort-committed-crash-resume.output"
  local committed_gate_count
  make_fake_repo "$committed_crash_repo"
  initialize_git_state "$committed_crash_state"
  mkdir -p "$committed_crash_target"
  assert_fails "real wrapper crash after authoritative marker fsync" \
    run_wrapper \
      "$committed_crash_repo" "$committed_crash_target" \
      "$committed_crash_state" "$committed_crash_log" \
      env \
        KANBAN_RELEASE_SAFE_PATH_TEST_EXIT_AT=publish-dir-after-marker-commit-parent \
      >"$committed_crash_output"
  published="$(find "$committed_crash_target/release/bundle/cohort" -mindepth 1 -maxdepth 1 \
    -type d -name '1111111111111111111111111111111111111111-2222222222222222222222222222222222222222-*' \
    -print -quit)"
  [[ -d "$published" && -f "$published.published" &&
    ! -e "$published.publishing" ]] ||
    fail "post-marker-fsync crash did not retain one authoritative generation"
  committed_gate_count="$(wc -l < "$committed_crash_log")"
  run_wrapper \
    "$committed_crash_repo" "$committed_crash_target" \
    "$committed_crash_state" "$committed_crash_log" env
  [[ -d "$published" && -f "$published.published" &&
    ! -e "$published.publishing" ]] ||
    fail "wrapper retry did not finish the fsynced authoritative generation"
  [[ "$(wc -l < "$committed_crash_log")" -eq "$committed_gate_count" ]] ||
    fail "authoritative crash resume reran the release build gates"

  local bypass_repo="$TMPROOT/cohort-path-bypass-repo"
  local bypass_target="$TMPROOT/cohort-path-bypass-target"
  local bypass_state="$TMPROOT/cohort-path-bypass-state"
  local bypass_log="$TMPROOT/cohort-path-bypass.log"
  make_fake_repo "$bypass_repo"
  initialize_git_state "$bypass_state"
  mkdir -p "$bypass_target"
  python3 - "$bypass_repo/scripts/release-cohort.sh" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
document = path.read_text(encoding="utf-8")
needle = "just affected-self-test\n"
assert needle in document
path.write_text(
    document.replace(
        needle,
        "cargo build --locked >/dev/null\n" + needle,
        1,
    ),
    encoding="utf-8",
)
PY
  assert_fails "undeclared direct cargo through host PATH" \
    run_wrapper "$bypass_repo" "$bypass_target" "$bypass_state" \
      "$bypass_log" env >/dev/null
}

assert_release_identity_resume_binding() {
  local repo="$TMPROOT/release-identity-repo"
  local target="$TMPROOT/release-identity-target"
  local state="$TMPROOT/release-identity-state"
  local log="$TMPROOT/release-identity.log"
  local output="$TMPROOT/release-identity-output"
  local cohort_root="$target/release/bundle/cohort"
  local base="1111111111111111111111111111111111111111-2222222222222222222222222222222222222222"

  make_fake_repo "$repo"
  initialize_git_state "$state"
  mkdir -p "$cohort_root/$base"

  # A v1/v2 commit-tree-only generation is never a valid resume candidate.
  assert_fails "legacy commit-tree generation resume" \
    run_wrapper "$repo" "$target" "$state" "$log" env >"$output" 2>&1
  grep -Fq "different or legacy identity" "$output" ||
    fail "legacy generation rejection lacked the canonical identity diagnostic"

  rm -rf "$cohort_root/$base"
  mkdir -p "$cohort_root/$base-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
  assert_fails "different full identity sibling collision" \
    run_wrapper "$repo" "$target" "$state" "$log" env >"$output" 2>&1
  grep -Fq "different or legacy identity" "$output" ||
    fail "different full identity sibling lacked the canonical collision diagnostic"
  rm -rf "$cohort_root/$base-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
  run_gate "$repo" prepare --output "$TMPROOT/release-identity-manifest.json"
  python3 - "$TMPROOT/release-identity-manifest.json" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
document = json.loads(path.read_text())
document.pop("identity")
path.write_text(json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n")
PY
  assert_fails "missing v3 identity field" \
    run_gate "$repo" verify --manifest "$TMPROOT/release-identity-manifest.json" >/dev/null
  run_gate "$repo" prepare --output "$TMPROOT/release-identity-old.json"
  python3 - "$TMPROOT/release-identity-old.json" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
document = json.loads(path.read_text())
document["schema_version"] = 2
path.write_text(json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n")
PY
  assert_fails "old v2 source manifest resume" \
    run_gate "$repo" verify --manifest "$TMPROOT/release-identity-old.json" >/dev/null

  mkdir -p "$cohort_root/$base"
  printf 'changed Cargo.lock\n' > "$repo/Cargo.lock"
  assert_fails "Cargo.lock identity resume" \
    run_wrapper "$repo" "$target" "$state" "$log" env >"$output" 2>&1
  rm -rf "$cohort_root/$base"
  printf 'fixture lock for release identity\n' > "$repo/Cargo.lock"

  mkdir -p "$cohort_root/$base"
  assert_fails "toolchain identity resume" \
    run_wrapper "$repo" "$target" "$state" "$log" env \
      FAKE_CARGO_VERSION='cargo 9.9.9 (different)' >"$output" 2>&1
  rm -rf "$cohort_root/$base"

  mkdir -p "$cohort_root/$base"
  assert_fails "feature identity resume" \
    run_wrapper "$repo" "$target" "$state" "$log" env \
      KANBAN_RELEASE_FEATURES='tantivy-backend' >"$output" 2>&1
  rm -rf "$cohort_root/$base"

  mkdir -p "$cohort_root/$base"
  assert_fails "target triple identity resume" \
    run_wrapper "$repo" "$target" "$state" "$log" env \
      FAKE_RUSTC_VV=$'rustc 1.0.0\nhost: aarch64-unknown-linux-gnu' >"$output" 2>&1
  rm -rf "$cohort_root/$base"

  mkdir -p "$cohort_root/$base"
  assert_fails "platform identity resume" \
    run_wrapper "$repo" "$target" "$state" "$log" env \
      FAKE_UNAME_S=Darwin >"$output" 2>&1
  rm -rf "$cohort_root/$base"

  mkdir -p "$cohort_root/$base"
  assert_fails "machine/debian arch identity resume" \
    run_wrapper "$repo" "$target" "$state" "$log" env \
      FAKE_UNAME_M=aarch64 >"$output" 2>&1
}

assert_cli_package_embeds_cohort() {
  local repo="$TMPROOT/cli-package-repo"
  local target="$TMPROOT/cli-package-target"
  local output="$TMPROOT/cli-package-output"
  local manifest="$output/source-provenance.json"
  local packaged_root="$TMPROOT/cli-package-root"
  local build_id
  make_fake_repo "$repo"
  cp "$PACKAGE_CLI" "$repo/scripts/package-cli-linux.sh"
  printf 'fixture lock\n' > "$repo/Cargo.lock"
  printf '# fixture\n' > "$repo/README.md"
  mkdir -p "$target" "$output"
  run_gate "$repo" prepare --output "$manifest"
  build_id="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["build_id"])' "$manifest")"

  cat > "$repo/scripts/cargo-build-lock.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "$*" == "--print-target-dir" ]]; then
  printf '%s\n' "${FAKE_TARGET_ROOT:?}"
  exit 0
fi
[[ "${1:-}" == "--" && $# -ge 2 ]] || exit 98
shift
KANBAN_CARGO_BUILD_LOCK_HELD=1 exec "$@"
EOF
  cat > "$repo/scripts/package-source-provenance.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  --invalidate-packages|--verify-dep-info) exit 0 ;;
  *) exit 98 ;;
esac
EOF
  cat > "$repo/fake-bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  --version)
    printf 'cargo 1.0.0 (fixture)\n'
    ;;
  pkgid)
    printf 'path+file:///fixture#2.1.3\n'
    ;;
  metadata)
    printf '%s\n' '{"packages":[{"name":"kanban-cli"},{"name":"kanban-vector-lancedb"},{"name":"kanban-graph-oxigraph"}]}'
    ;;
  build)
    release="${FAKE_TARGET_ROOT:?}/release"
    mkdir -p "$release"
    if [[ " $* " == *' -p kanban-cli '* ]]; then
      binaries=(kanban)
    else
      binaries=(kanban-vector-lancedb kanban-graph-oxigraph)
    fi
    for binary in "${binaries[@]}"; do
      cat > "$release/$binary" <<EOF_BINARY
#!/usr/bin/env bash
if [[ "\${1:-}" == "__build-identity" ]]; then
  printf '%s' '${KANBAN_BUILD_ID:?}'
  exit 0
fi
printf 'fake %s %s\n' '$binary' '${KANBAN_BUILD_ID:?}'
EOF_BINARY
      printf 'fake dep info\n' > "$release/$binary.d"
      chmod +x "$release/$binary"
    done
    ;;
  *)
    echo "unexpected fake cargo invocation: $*" >&2
    exit 98
    ;;
esac
EOF
cat > "$repo/fake-bin/rustc" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' 'rustc 1.0.0
binary: rustc
commit-hash: fixture
commit-date: 1970-01-01
host: x86_64-unknown-linux-gnu
release: 1.0.0
LLVM version: 1.0.0'
EOF
  cat > "$repo/fake-bin/dpkg-shlibdeps" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf 'shlibs:Depends=libc6\n'
EOF
  cat > "$repo/fake-bin/dpkg-deb" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
exec /usr/bin/dpkg-deb "$@"
EOF
  chmod +x "$repo/scripts/package-cli-linux.sh" \
    "$repo/scripts/cargo-build-lock.sh" \
    "$repo/scripts/package-source-provenance.sh" \
    "$repo/fake-bin/cargo" "$repo/fake-bin/rustc" \
    "$repo/fake-bin/dpkg-shlibdeps" "$repo/fake-bin/dpkg-deb"

  env \
    PATH="$repo/fake-bin:$PATH" \
    FAKE_TARGET_ROOT="$target" \
    FAKE_PACKAGED_ROOT="$packaged_root" \
    KANBAN_BUILD_ID="$build_id" \
    KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
    KANBAN_RELEASE_SOURCE_MAP="$repo/docs/release/derived-projection-v2-source-map.json" \
    "$repo/scripts/package-cli-linux.sh" --format deb \
      --no-default-features --features "tantivy-backend,oxigraph-backend" >/dev/null

  rm -rf "$packaged_root"
  mkdir -p "$packaged_root"
  /usr/bin/dpkg-deb -R "$target/release/bundle/cli/deb/"kanban-tool-cli_*.deb \
    "$packaged_root"
  cmp -s "$packaged_root/usr/share/doc/kanban-tool-cli/source-provenance.json" \
    "$manifest" || fail "CLI Debian package did not embed source provenance"
  cmp -s \
    "$packaged_root/usr/share/doc/kanban-tool-cli/derived-projection-v2-source-map.json" \
    "$repo/docs/release/derived-projection-v2-source-map.json" ||
    fail "CLI Debian package did not embed the semantic source map"
  grep -Fqx "X-Kanban-Build-Id: $build_id" "$packaged_root/DEBIAN/control" ||
    fail "CLI Debian control does not bind the release build identity"

  local deb_path secret escaped
  deb_path="$(
    find "$target/release/bundle/cli/deb" -maxdepth 1 -type f \
      -name 'kanban-tool-cli_*.deb' -print -quit
  )"
  [[ -n "$deb_path" ]] || fail "CLI package fixture produced no Debian artifact"

  secret="$TMPROOT/hardlink-sentinel"
  printf 'hardlink sentinel\n' > "$secret"
  unlink "$deb_path"
  ln "$secret" "$deb_path"
  assert_fails "CLI package hostile hardlink output" \
    env \
      PATH="$repo/fake-bin:$PATH" \
      FAKE_TARGET_ROOT="$target" \
      FAKE_PACKAGED_ROOT="$packaged_root" \
      KANBAN_BUILD_ID="$build_id" \
      KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
      KANBAN_RELEASE_SOURCE_MAP="$repo/docs/release/derived-projection-v2-source-map.json" \
      "$repo/scripts/package-cli-linux.sh" --format deb \
        --no-default-features --features "tantivy-backend,oxigraph-backend" >/dev/null
  [[ "$(cat "$secret")" == "hardlink sentinel" ]] ||
    fail "CLI package rewrote a hostile hardlink target"

  unlink "$deb_path"
  ln -s "$secret" "$deb_path"
  assert_fails "CLI package hostile symlink output" \
    env \
      PATH="$repo/fake-bin:$PATH" \
      FAKE_TARGET_ROOT="$target" \
      FAKE_PACKAGED_ROOT="$packaged_root" \
      KANBAN_BUILD_ID="$build_id" \
      KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
      KANBAN_RELEASE_SOURCE_MAP="$repo/docs/release/derived-projection-v2-source-map.json" \
      "$repo/scripts/package-cli-linux.sh" --format deb \
        --no-default-features --features "tantivy-backend,oxigraph-backend" >/dev/null
  [[ "$(cat "$secret")" == "hardlink sentinel" ]] ||
    fail "CLI package rewrote a hostile symlink target"

  unlink "$deb_path"
  rmdir "$target/release/bundle/cli/deb"
  rmdir "$target/release/bundle/cli"
  escaped="$TMPROOT/escaped-package-output"
  mkdir -p "$escaped"
  ln -s "$escaped" "$target/release/bundle/cli"
  assert_fails "CLI package symlinked output parent" \
    env \
      PATH="$repo/fake-bin:$PATH" \
      FAKE_TARGET_ROOT="$target" \
      FAKE_PACKAGED_ROOT="$packaged_root" \
      KANBAN_BUILD_ID="$build_id" \
      KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
      KANBAN_RELEASE_SOURCE_MAP="$repo/docs/release/derived-projection-v2-source-map.json" \
      "$repo/scripts/package-cli-linux.sh" --format deb \
        --no-default-features --features "tantivy-backend,oxigraph-backend" >/dev/null
  [[ -z "$(find "$escaped" -mindepth 1 -print -quit)" ]] ||
    fail "CLI package escaped through a symlinked output parent"
}

assert_desktop_embed_rejects_hostile_paths() {
  local repo="$TMPROOT/embed-repo"
  local target="$TMPROOT/embed-target"
  local output="$TMPROOT/embed-output"
  local manifest="$output/source-provenance.json"
  local desktop_root="$target/fake-desktop-root"
  local deb="$target/release/bundle/deb/Kanban Tool_2.1.3_amd64.deb"
  local build_id sentinel escaped baseline_deb forged_manifest no_op_gate
  make_fake_repo "$repo"
  mkdir -p "$target/release/bundle/deb" "$desktop_root/DEBIAN" "$output"
  chmod 0755 "$desktop_root" "$desktop_root/DEBIAN"
  cat > "$desktop_root/DEBIAN/control" <<'EOF_CONTROL'
Package: kanban-tool
Version: 2.1.3
Architecture: amd64
Maintainer: release-test <release-test@example.invalid>
Description: fixture desktop package
EOF_CONTROL
  /usr/bin/dpkg-deb --root-owner-group --build "$desktop_root" "$deb" >/dev/null
  baseline_deb="$TMPROOT/embed-original.deb"
  cp "$deb" "$baseline_deb"
  run_gate "$repo" prepare --output "$manifest"
  build_id="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["build_id"])' "$manifest")"

  env \
    PATH="$repo/fake-bin:$PATH" \
    FAKE_TARGET_ROOT="$target" \
    FAKE_LOCK_LOG="$target/embed-lock.log" \
    FAKE_CLI_PACKAGE_ROOT="$target/fake-cli-root" \
    FAKE_DESKTOP_PACKAGE_ROOT="$desktop_root" \
    KANBAN_BUILD_ID="$build_id" \
    KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
    KANBAN_RELEASE_SOURCE_MAP="$repo/docs/release/derived-projection-v2-source-map.json" \
    "$repo/scripts/embed-release-provenance-deb.sh" \
      --deb "$deb" --doc-dir kanban-tool-desktop

  forged_manifest="$output/forged-source-provenance.json"
  no_op_gate="$repo/scripts/no-op-source-gate.sh"
  cp "$manifest" "$forged_manifest"
  cat > "$no_op_gate" <<'EOF_NOOP_GATE'
#!/usr/bin/env bash
set -euo pipefail
exit 0
EOF_NOOP_GATE
  chmod 0755 "$no_op_gate"
  python3 - "$forged_manifest" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
document = json.loads(path.read_text(encoding="utf-8"))
document["remote"]["name"] = "attacker"
path.write_text(json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n")
PY
  cp "$baseline_deb" "$deb"
  assert_fails "desktop provenance rejects forged manifest with custom source gate" \
    env \
      PATH="$repo/fake-bin:$PATH" \
      FAKE_TARGET_ROOT="$target" \
      FAKE_LOCK_LOG="$target/embed-lock.log" \
      KANBAN_BUILD_ID="$build_id" \
      KANBAN_RELEASE_SOURCE_MANIFEST="$forged_manifest" \
      KANBAN_RELEASE_SOURCE_MAP="$repo/docs/release/derived-projection-v2-source-map.json" \
      KANBAN_RELEASE_SOURCE_GATE="$no_op_gate" \
      "$repo/scripts/embed-release-provenance-deb.sh" \
        --deb "$deb" --doc-dir kanban-tool-desktop >/dev/null

  sentinel="$TMPROOT/embed-hardlink-sentinel"
  printf 'embed hardlink sentinel\n' > "$sentinel"
  unlink "$deb"
  ln "$sentinel" "$deb"
  assert_fails "desktop provenance hostile hardlink" \
    env \
      PATH="$repo/fake-bin:$PATH" \
      FAKE_TARGET_ROOT="$target" \
      FAKE_LOCK_LOG="$target/embed-lock.log" \
      KANBAN_BUILD_ID="$build_id" \
      KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
      KANBAN_RELEASE_SOURCE_MAP="$repo/docs/release/derived-projection-v2-source-map.json" \
      "$repo/scripts/embed-release-provenance-deb.sh" \
        --deb "$deb" --doc-dir kanban-tool-desktop >/dev/null
  [[ "$(cat "$sentinel")" == "embed hardlink sentinel" ]] ||
    fail "desktop provenance injector rewrote a hostile hardlink"

  unlink "$deb"
  ln -s "$sentinel" "$deb"
  assert_fails "desktop provenance hostile symlink" \
    env \
      PATH="$repo/fake-bin:$PATH" \
      FAKE_TARGET_ROOT="$target" \
      FAKE_LOCK_LOG="$target/embed-lock.log" \
      KANBAN_BUILD_ID="$build_id" \
      KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
      KANBAN_RELEASE_SOURCE_MAP="$repo/docs/release/derived-projection-v2-source-map.json" \
      "$repo/scripts/embed-release-provenance-deb.sh" \
        --deb "$deb" --doc-dir kanban-tool-desktop >/dev/null
  [[ "$(cat "$sentinel")" == "embed hardlink sentinel" ]] ||
    fail "desktop provenance injector rewrote a hostile symlink"

  unlink "$deb"
  rmdir "$target/release/bundle/deb"
  escaped="$TMPROOT/embed-escaped-output"
  mkdir -p "$escaped"
  ln -s "$escaped" "$target/release/bundle/deb"
  printf 'escaped deb fixture\n' > "$escaped/$(basename "$deb")"
  assert_fails "desktop provenance symlinked parent" \
    env \
      PATH="$repo/fake-bin:$PATH" \
      FAKE_TARGET_ROOT="$target" \
      FAKE_LOCK_LOG="$target/embed-lock.log" \
      KANBAN_BUILD_ID="$build_id" \
      KANBAN_RELEASE_SOURCE_MANIFEST="$manifest" \
      KANBAN_RELEASE_SOURCE_MAP="$repo/docs/release/derived-projection-v2-source-map.json" \
      "$repo/scripts/embed-release-provenance-deb.sh" \
        --deb "$deb" --doc-dir kanban-tool-desktop >/dev/null
  [[ "$(cat "$escaped/$(basename "$deb")")" == "escaped deb fixture" ]] ||
    fail "desktop provenance injector escaped through a symlinked parent"
}

assert_publish_source_parent_identity() {
  local root="$TMPROOT/publish-source-parent-identity"
  local stage detached source destination output status
  local -a identity
  mkdir -p "$root/output"

  mapfile -t identity < <(
    python3 "$SAFE_PATH" private-dir --root "$root" \
      --parent "$root" --prefix ".stage." --print-identity
  )
  [[ "${#identity[@]}" -eq 3 ]] ||
    fail "private-dir did not return one complete source-parent identity token"
  stage="${identity[0]}"
  detached="$root/original-stage"
  source="$stage/artifact"
  destination="$root/output/artifact"
  printf 'original artifact\n' > "$source"

  python3 "$SAFE_PATH" dir-identity --root "$root" --path "$stage" \
    --expected-dev "${identity[1]}" --expected-ino "${identity[2]}" >/dev/null
  mv "$stage" "$detached"
  mkdir -m 0700 "$stage"
  printf 'replacement artifact\n' > "$source"
  output="$TMPROOT/publish-source-parent-drift.output"
  assert_fails "publish source-parent identity drift" \
    python3 "$SAFE_PATH" publish-file --root "$root" \
      --source "$source" --destination "$destination" \
      --expected-source-parent-dev "${identity[1]}" \
      --expected-source-parent-ino "${identity[2]}" >"$output"
  grep -Fq "publish source parent identity does not match expected identity observation token" \
    "$output" ||
    fail "source-parent identity drift lacked the canonical diagnostic"
  [[ "$(cat "$source")" == "replacement artifact" &&
    "$(cat "$detached/artifact")" == "original artifact" &&
    ! -e "$destination" ]] ||
    fail "source-parent identity drift published or consumed an unbound artifact"

  mapfile -t identity < <(
    python3 "$SAFE_PATH" private-dir --root "$root" \
      --parent "$root" --prefix ".valid-stage." --print-identity
  )
  stage="${identity[0]}"
  source="$stage/artifact"
  destination="$root/output/valid-artifact"
  printf 'valid bound artifact\n' > "$source"
  python3 "$SAFE_PATH" publish-file --root "$root" \
    --source "$source" --destination "$destination" \
    --expected-source-parent-dev "${identity[1]}" \
    --expected-source-parent-ino "${identity[2]}"
  [[ ! -e "$source" && "$(cat "$destination")" == "valid bound artifact" ]] ||
    fail "valid source-parent identity token did not publish normally"

  mkdir -m 0700 "$root/unbound-stage"
  source="$root/unbound-stage/artifact"
  destination="$root/output/unbound-artifact"
  printf 'unbound compatibility artifact\n' > "$source"
  python3 "$SAFE_PATH" publish-file --root "$root" \
    --source "$source" --destination "$destination"
  [[ ! -e "$source" &&
    "$(cat "$destination")" == "unbound compatibility artifact" ]] ||
    fail "publish-file without an optional source-parent token regressed"

  mkdir -m 0700 "$root/invalid-stage"
  source="$root/invalid-stage/artifact"
  printf 'invalid token artifact\n' > "$source"
  assert_fails "partial source-parent identity token" \
    python3 "$SAFE_PATH" publish-file --root "$root" \
      --source "$source" --destination "$root/output/partial-artifact" \
      --expected-source-parent-dev 1 >/dev/null
  assert_fails "empty source-parent identity token" \
    python3 "$SAFE_PATH" publish-file --root "$root" \
      --source "$source" --destination "$root/output/empty-artifact" \
      --expected-source-parent-dev "" --expected-source-parent-ino 1 >/dev/null
  assert_fails "negative source-parent identity token" \
    python3 "$SAFE_PATH" publish-file --root "$root" \
      --source "$source" --destination "$root/output/negative-artifact" \
      --expected-source-parent-dev -1 --expected-source-parent-ino 1 >/dev/null
  [[ "$(cat "$source")" == "invalid token artifact" &&
    ! -e "$root/output/partial-artifact" &&
    ! -e "$root/output/empty-artifact" &&
    ! -e "$root/output/negative-artifact" ]] ||
    fail "invalid source-parent identity arguments consumed the source"
}

assert_safe_path_atomic_durability() {
  local root="$TMPROOT/safe-path-root"
  local trace="$TMPROOT/safe-path.trace"
  local missing_primitive_fixture="$TMPROOT/missing-release-primitive.py"
  local source destination continue_file status private_dir
  local pause_marker continue_marker output helper_pid detached escaped
  local generation published digest writer_fd expected unknown
  local failure_trace retry_trace marker
  mkdir -p "$root"
  cat > "$missing_primitive_fixture" <<'PY'
#!/usr/bin/env python3
import fcntl
import runpy
import signal
import sys

safe_path, module_name, symbol, *arguments = sys.argv[1:]
modules = {"fcntl": fcntl, "signal": signal}
module = modules[module_name]
if hasattr(module, symbol):
    delattr(module, symbol)
sys.argv = [safe_path, *arguments]
runpy.run_path(safe_path, run_name="__main__")
PY
  chmod 0755 "$missing_primitive_fixture"

  private_dir="$(
    KANBAN_RELEASE_SAFE_PATH_TEST_TRACE="$trace" \
      python3 "$SAFE_PATH" private-dir --root "$root" \
        --parent "$root" --prefix ".private-phase."
  )"
  [[ "$(stat -c '%a' "$private_dir")" == "700" ]] ||
    fail "private-dir did not create a mode 0700 directory"
  KANBAN_RELEASE_SAFE_PATH_TEST_TRACE="$trace" \
    python3 "$SAFE_PATH" ensure-dir --root "$root" \
      --path "$root/one/two/three" --mode 0755
  printf 'stable artifact\n' > "$root/source"
  KANBAN_RELEASE_SAFE_PATH_TEST_TRACE="$trace" \
    python3 "$SAFE_PATH" copy-file --root "$root" \
      --source "$root/source" --destination "$root/one/two/three/copied" --mode 0444
  KANBAN_RELEASE_SAFE_PATH_TEST_TRACE="$trace" \
    python3 "$SAFE_PATH" seal-tree --root "$root" --path "$root/one"
  python3 - "$trace" "$root" "$private_dir" <<'PY'
import pathlib
import sys

trace = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
root = pathlib.Path(sys.argv[2])
required = {
    f"private-dir-new\t{pathlib.Path(sys.argv[2]) / pathlib.Path(sys.argv[3]).name}",
    f"private-dir-parent\t{root}",
    f"copy-file-data\t{root / 'one/two/three/copied'}",
    f"copy-file-parent\t{root / 'one/two/three'}",
    f"seal-file\t{root / 'one/two/three/copied'}",
    f"seal-directory\t{root / 'one/two/three'}",
    f"seal-directory\t{root / 'one/two'}",
    f"seal-directory\t{root / 'one'}",
    f"fsync-dir\t{root / 'one'}",
    f"fsync-dir\t{root / 'one/two'}",
    f"fsync-dir\t{root / 'one/two/three'}",
    f"fsync-file\t{root / 'one/two/three/copied'}",
}
missing = required.difference(trace)
assert not missing, f"missing durable sync evidence: {sorted(missing)}"
positions = {
    value: max(index for index, row in enumerate(trace) if row == value)
    for value in required
}
assert positions[f"fsync-dir\t{root / 'one/two/three'}"] < positions[f"fsync-dir\t{root / 'one/two'}"]
assert positions[f"fsync-dir\t{root / 'one/two'}"] < positions[f"fsync-dir\t{root / 'one'}"]
PY

  failure_trace="$TMPROOT/private-dir-fsync-failure.trace"
  assert_fails "private-dir object fsync failure" \
    env KANBAN_RELEASE_SAFE_PATH_TEST_TRACE="$failure_trace" \
      KANBAN_RELEASE_SAFE_PATH_TEST_FAIL_AT=private-dir-new \
    python3 "$SAFE_PATH" private-dir --root "$root" \
      --parent "$root" --prefix ".private-fsync-failure." >/dev/null
  grep -Fq $'private-dir-new\t'"$root/.private-fsync-failure." "$failure_trace" ||
    fail "private-dir fsync failure trace lacks the injected checkpoint"
  [[ -z "$(find "$root" -maxdepth 1 -type d \
    -name '.private-fsync-failure.*' -print -quit)" ]] ||
    fail "failed private-dir durability left an unreported staging directory"

  mkdir -p "$root/xattr-user"
  printf 'xattr user\n' > "$root/xattr-user/artifact"
  python3 "$SAFE_PATH" seal-tree --root "$root" --path "$root/xattr-user"
  python3 - "$root/xattr-user/artifact" <<'PY'
import os
import sys

os.setxattr(sys.argv[1], "user.kanban_release_test", b"forbidden")
PY
  assert_fails "sealed generation user xattr" \
    python3 "$SAFE_PATH" tree-digest --root "$root" \
      --path "$root/xattr-user" --require-sealed >/dev/null

  mkdir -p "$root/xattr-acl"
  printf 'xattr acl\n' > "$root/xattr-acl/artifact"
  python3 "$SAFE_PATH" seal-tree --root "$root" --path "$root/xattr-acl"
  if python3 - "$root/xattr-acl/artifact" <<'PY'
import os
import struct
import sys

header = struct.pack("<I", 2)
entry = lambda tag, permissions, identifier: struct.pack(
    "<HHI", tag, permissions, identifier
)
acl = b"".join(
    (
        header,
        entry(0x01, 0x04, 0xFFFFFFFF),
        entry(0x02, 0x04, 0),
        entry(0x04, 0x04, 0xFFFFFFFF),
        entry(0x10, 0x04, 0xFFFFFFFF),
        entry(0x20, 0x04, 0xFFFFFFFF),
    )
)
try:
    os.setxattr(sys.argv[1], "system.posix_acl_access", acl)
except OSError as error:
    print(f"note: POSIX ACL fixture unsupported: {error}", file=sys.stderr)
    raise SystemExit(77)
PY
  then
    assert_fails "sealed generation POSIX ACL" \
      python3 "$SAFE_PATH" tree-digest --root "$root" \
        --path "$root/xattr-acl" --require-sealed >/dev/null
  fi

  mkdir -p "$root/xattr-capability"
  printf 'xattr capability\n' > "$root/xattr-capability/artifact"
  python3 "$SAFE_PATH" seal-tree --root "$root" --path "$root/xattr-capability"
  if python3 - "$root/xattr-capability/artifact" <<'PY'
import os
import sys

try:
    os.setxattr(
        sys.argv[1],
        "security.capability",
        bytes.fromhex("0100000200040000000000000000000000000000"),
    )
except OSError as error:
    print(f"note: file capability fixture unsupported: {error}", file=sys.stderr)
    raise SystemExit(77)
PY
  then
    assert_fails "sealed generation file capability" \
      python3 "$SAFE_PATH" tree-digest --root "$root" \
        --path "$root/xattr-capability" --require-sealed >/dev/null
  fi

  mkdir -p "$root/aba-parent"
  printf 'dirfd anchored source\n' > "$root/aba-source"
  pause_marker="$TMPROOT/safe-path-aba.pause"
  continue_marker="$TMPROOT/safe-path-aba.continue"
  output="$TMPROOT/safe-path-aba.output"
  KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_AT=publish-file-before-rename \
    KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_MARKER="$pause_marker" \
    KANBAN_RELEASE_SAFE_PATH_TEST_CONTINUE="$continue_marker" \
    python3 "$SAFE_PATH" publish-file --root "$root" \
      --source "$root/aba-source" \
      --destination "$root/aba-parent/published" >"$output" 2>&1 &
  helper_pid=$!
  for _ in {1..3000}; do
    [[ -f "$pause_marker" ]] && break
    kill -0 "$helper_pid" 2>/dev/null ||
      fail "safe-path ABA fixture exited before the rename checkpoint"
    sleep 0.01
  done
  [[ -f "$pause_marker" ]] || fail "safe-path ABA fixture did not pause"
  detached="$root/aba-parent-detached"
  escaped="$TMPROOT/safe-path-escaped-parent"
  mv "$root/aba-parent" "$detached"
  mkdir -p "$escaped"
  ln -s "$escaped" "$root/aba-parent"
  : > "$continue_marker"
  set +e
  wait "$helper_pid"
  status=$?
  set -e
  [[ "$status" -ne 0 ]] || fail "parent ABA publish unexpectedly succeeded"
  [[ ! -e "$escaped/published" ]] ||
    fail "dirfd publish escaped through a swapped symlink parent"
  [[ -f "$root/aba-source" ]] ||
    fail "failed parent identity check consumed the publish source"
  unlink "$root/aba-parent"
  mv "$detached" "$root/aba-parent"

  chmod u+w "$root/one" "$root/one/two" "$root/one/two/three"
  source="$root/one/two/three/no-replace-source"
  destination="$root/one/two/no-replace-destination"
  printf 'before rename crash\n' > "$source"
  set +e
  KANBAN_RELEASE_SAFE_PATH_TEST_EXIT_AT=publish-file-before-rename \
    python3 "$SAFE_PATH" publish-file --root "$root" \
      --source "$source" --destination "$destination"
  status=$?
  set -e
  [[ "$status" -eq 86 && -f "$source" && ! -e "$destination" ]] ||
    fail "pre-rename crash did not leave one retryable source entry"
  [[ "$(stat -c '%h' "$source")" == "1" ]] ||
    fail "pre-rename crash left a multiply linked source"
  python3 "$SAFE_PATH" publish-file --root "$root" \
    --source "$source" --destination "$destination"
  [[ ! -e "$source" && -f "$destination" &&
    "$(stat -c '%h' "$destination")" == "1" ]] ||
    fail "NOREPLACE retry did not converge to one destination entry"

  source="$root/one/two/three/after-rename-source"
  destination="$root/one/two/after-rename-destination"
  printf 'after rename crash\n' > "$source"
  set +e
  KANBAN_RELEASE_SAFE_PATH_TEST_EXIT_AT=publish-file-after-rename \
    python3 "$SAFE_PATH" publish-file --root "$root" \
      --source "$source" --destination "$destination"
  status=$?
  set -e
  [[ "$status" -eq 86 && ! -e "$source" && -f "$destination" ]] ||
    fail "post-rename crash did not leave one complete destination entry"
  [[ "$(stat -c '%h' "$destination")" == "1" ]] ||
    fail "post-rename crash left the historical hardlink residue"

  source="$root/one/two/three/collision-source"
  destination="$root/one/two/collision-destination"
  printf 'collision source\n' > "$source"
  printf 'existing destination\n' > "$destination"
  assert_fails "NOREPLACE collision" \
    python3 "$SAFE_PATH" publish-file --root "$root" \
      --source "$source" --destination "$destination" >/dev/null
  [[ "$(cat "$source")" == "collision source" &&
    "$(cat "$destination")" == "existing destination" &&
    "$(stat -c '%h' "$source")" == "1" &&
    "$(stat -c '%h' "$destination")" == "1" ]] ||
    fail "NOREPLACE collision mutated source/destination or created hardlinks"

  source="$root/one/two/three/replace-aba-source"
  destination="$root/one/two/replace-aba-destination"
  expected="$root/one/two/replace-aba-expected"
  printf 'replace ABA source\n' > "$source"
  printf 'expected old destination\n' > "$destination"
  pause_marker="$TMPROOT/safe-path-replace-aba.pause"
  continue_marker="$TMPROOT/safe-path-replace-aba.continue"
  output="$TMPROOT/safe-path-replace-aba.output"
  KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_AT=publish-file-before-rename \
    KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_MARKER="$pause_marker" \
    KANBAN_RELEASE_SAFE_PATH_TEST_CONTINUE="$continue_marker" \
    python3 "$SAFE_PATH" publish-file --root "$root" \
      --source "$source" --destination "$destination" --replace \
      >"$output" 2>&1 &
  helper_pid=$!
  for _ in {1..3000}; do
    [[ -f "$pause_marker" ]] && break
    kill -0 "$helper_pid" 2>/dev/null ||
      fail "replace ABA fixture exited before its checkpoint"
    sleep 0.01
  done
  [[ -f "$pause_marker" ]] || fail "replace ABA fixture did not pause"
  mv "$destination" "$expected"
  printf 'unknown replacement\n' > "$destination"
  : > "$continue_marker"
  set +e
  wait "$helper_pid"
  status=$?
  set -e
  [[ "$status" -ne 0 ]] || fail "replace destination ABA unexpectedly succeeded"
  [[ "$(cat "$source")" == "replace ABA source" &&
    "$(cat "$destination")" == "unknown replacement" &&
    "$(cat "$expected")" == "expected old destination" ]] ||
    fail "replace ABA overwrote or consumed an unknown replacement"

  local post_parent="$root/post-rename-parent"
  local post_detached="$root/post-rename-parent-detached"
  mkdir -p "$post_parent"
  source="$post_parent/source"
  destination="$post_parent/destination"
  printf 'post-rename parent ABA release\n' > "$source"
  pause_marker="$TMPROOT/post-rename-parent-aba.pause"
  continue_marker="$TMPROOT/post-rename-parent-aba.continue"
  output="$TMPROOT/post-rename-parent-aba.output"
  KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_AT=publish-file-before-final-check \
    KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_MARKER="$pause_marker" \
    KANBAN_RELEASE_SAFE_PATH_TEST_CONTINUE="$continue_marker" \
    python3 "$SAFE_PATH" publish-file --root "$root" \
      --source "$source" --destination "$destination" >"$output" 2>&1 &
  helper_pid=$!
  for _ in {1..3000}; do
    [[ -f "$pause_marker" ]] && break
    kill -0 "$helper_pid" 2>/dev/null ||
      fail "post-rename parent ABA fixture exited before its checkpoint"
    sleep 0.01
  done
  [[ -f "$pause_marker" ]] ||
    fail "post-rename parent ABA fixture did not pause"
  mv "$post_parent" "$post_detached"
  mkdir -p "$post_parent"
  printf 'unknown public destination\n' > "$post_parent/destination"
  : > "$continue_marker"
  set +e
  wait "$helper_pid"
  status=$?
  set -e
  [[ "$status" -ne 0 ]] ||
    fail "post-rename parent ABA incorrectly crossed the commit boundary"
  [[ "$(cat "$post_parent/destination")" == "unknown public destination" &&
    "$(cat "$post_detached/source")" == "post-rename parent ABA release" &&
    ! -e "$post_detached/destination" ]] ||
    fail "post-rename parent ABA did not safely roll back through retained dirfds"

  local rollback_root="$root/replace-rollback-drift"
  local rollback_dev rollback_ino
  mkdir -p "$rollback_root"
  read -r rollback_dev rollback_ino < <(
    python3 "$SAFE_PATH" dir-identity --root "$root" --path "$rollback_root"
  )
  source="$rollback_root/source"
  destination="$rollback_root/destination"
  expected="$rollback_root/old-detached"
  printf 'replacement new release\n' > "$source"
  printf 'replacement old target\n' > "$destination"
  pause_marker="$TMPROOT/replace-rollback-drift.pause"
  continue_marker="$TMPROOT/replace-rollback-drift.continue"
  output="$TMPROOT/replace-rollback-drift.output"
  KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_AT=publish-file-before-final-check \
    KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_MARKER="$pause_marker" \
    KANBAN_RELEASE_SAFE_PATH_TEST_CONTINUE="$continue_marker" \
    python3 "$SAFE_PATH" publish-file --root "$root" \
      --source "$source" --destination "$destination" --replace \
      >"$output" 2>&1 &
  helper_pid=$!
  for _ in {1..3000}; do
    [[ -f "$pause_marker" ]] && break
    kill -0 "$helper_pid" 2>/dev/null ||
      fail "replacement rollback drift fixture exited before its checkpoint"
    sleep 0.01
  done
  [[ -f "$pause_marker" ]] ||
    fail "replacement rollback drift fixture did not pause"
  mv "$source" "$expected"
  printf 'unknown private-stage entry\n' > "$source"
  : > "$continue_marker"
  set +e
  wait "$helper_pid"
  status=$?
  set -e
  [[ "$status" -ne 0 ]] ||
    fail "replacement rollback drift unexpectedly committed"
  assert_fails "unsafe replacement rollback cleanup" \
    python3 "$SAFE_PATH" remove-tree --root "$root" --path "$rollback_root" \
      --expected-dev "$rollback_dev" --expected-ino "$rollback_ino" >/dev/null
  [[ "$(cat "$source")" == "unknown private-stage entry" &&
    "$(cat "$destination")" == "replacement new release" &&
    "$(cat "$expected")" == "replacement old target" ]] ||
    fail "unsafe replacement rollback exchanged or cleaned an unknown entry"

  source="$root/one/two/three/no-replace-fsync-source"
  destination="$root/one/two/no-replace-fsync-destination"
  failure_trace="$TMPROOT/no-replace-fsync-failure.trace"
  printf 'no-replace fsync rollback\n' > "$source"
  assert_fails "no-replace parent fsync rollback" \
    env KANBAN_RELEASE_SAFE_PATH_TEST_TRACE="$failure_trace" \
      KANBAN_RELEASE_SAFE_PATH_TEST_FAIL_AT=publish-file-parent \
    python3 "$SAFE_PATH" publish-file --root "$root" \
      --source "$source" --destination "$destination" >/dev/null
  grep -Fqx "publish-file-parent	$root/one/two" "$failure_trace" ||
    fail "no-replace fsync failure trace lacks the injected checkpoint"
  grep -Fqx "publish-file-rollback-parent	$root/one/two" "$failure_trace" ||
    fail "no-replace fsync failure trace lacks rollback durability"
  [[ "$(cat "$source")" == "no-replace fsync rollback" &&
    ! -e "$destination" ]] ||
    fail "no-replace fsync failure did not restore the source"

  source="$root/one/two/three/replace-fsync-source"
  destination="$root/one/two/replace-fsync-destination"
  failure_trace="$TMPROOT/replace-fsync-failure.trace"
  printf 'replace fsync new\n' > "$source"
  printf 'replace fsync old\n' > "$destination"
  assert_fails "replace parent fsync rollback" \
    env KANBAN_RELEASE_SAFE_PATH_TEST_TRACE="$failure_trace" \
      KANBAN_RELEASE_SAFE_PATH_TEST_FAIL_AT=publish-file-parent \
    python3 "$SAFE_PATH" publish-file --root "$root" \
      --source "$source" --destination "$destination" --replace >/dev/null
  grep -Fqx "publish-file-parent	$root/one/two" "$failure_trace" ||
    fail "replace fsync failure trace lacks the injected checkpoint"
  grep -Fqx "publish-file-rollback-parent	$root/one/two" "$failure_trace" ||
    fail "replace fsync failure trace lacks rollback durability"
  [[ "$(cat "$source")" == "replace fsync new" &&
    "$(cat "$destination")" == "replace fsync old" ]] ||
    fail "replace fsync failure lost the old target or retryable source"

  source="$root/one/two/three/enosys-source"
  destination="$root/one/two/enosys-destination"
  output="$TMPROOT/renameat2-enosys.output"
  printf 'renameat2 unavailable\n' > "$source"
  assert_fails "renameat2 ENOSYS must fail closed" \
    env KANBAN_RELEASE_SAFE_PATH_TEST_RENAMEAT2_ERRNO=ENOSYS \
    python3 "$SAFE_PATH" publish-file --root "$root" \
      --source "$source" --destination "$destination" >"$output"
  grep -Fqx \
    'error: required Linux primitive unavailable: renameat2(RENAME_NOREPLACE): ENOSYS (Function not implemented); no fallback while release file' \
    "$output" ||
    fail "renameat2 ENOSYS did not use the canonical primitive diagnostic"
  [[ -f "$source" && ! -e "$destination" ]] ||
    fail "renameat2 ENOSYS did not preserve the retryable source"

  source="$root/one/two/three/missing-renameat2-source"
  destination="$root/one/two/missing-renameat2-destination"
  output="$TMPROOT/renameat2-missing.output"
  printf 'renameat2 symbol missing\n' > "$source"
  assert_fails "missing renameat2 primitive must fail closed" \
    env KANBAN_RELEASE_SAFE_PATH_TEST_RENAMEAT2_MISSING=1 \
    python3 "$SAFE_PATH" publish-file --root "$root" \
      --source "$source" --destination "$destination" >"$output"
  grep -Fqx \
    'error: required Linux primitive unavailable: renameat2(RENAME_NOREPLACE): symbol missing; no fallback' \
    "$output" ||
    fail "missing renameat2 primitive lacked the canonical diagnostic"
  [[ -f "$source" && ! -e "$destination" ]] ||
    fail "missing renameat2 primitive consumed the retryable source"

  chmod u+w "$root/one/two/three"
  printf 'fsync failure\n' > "$root/one/two/three/fsync-source"
  failure_trace="$TMPROOT/copy-file-parent-failure.trace"
  assert_fails "copy parent fsync failure" \
    env KANBAN_RELEASE_SAFE_PATH_TEST_TRACE="$failure_trace" \
      KANBAN_RELEASE_SAFE_PATH_TEST_FAIL_AT=copy-file-parent \
    python3 "$SAFE_PATH" copy-file --root "$root" \
      --source "$root/one/two/three/fsync-source" \
      --destination "$root/one/two/three/fsync-destination" --mode 0444 >/dev/null
  grep -Fqx "copy-file-parent	$root/one/two/three" "$failure_trace" ||
    fail "copy parent fsync failure trace lacks the injected checkpoint"
  grep -Fqx "copy-file-cleanup-parent	$root/one/two/three" "$failure_trace" ||
    fail "copy parent fsync failure trace lacks cleanup durability"
  [[ ! -e "$root/one/two/three/fsync-destination" ]] ||
    fail "failed copy fsync left a publishable destination"

  generation="$root/generation-existing-writer"
  published="$root/published-existing-writer"
  mkdir -p "$generation/nested"
  printf 'existing writer lease conflict\n' > "$generation/nested/artifact"
  exec {writer_fd}>> "$generation/nested/artifact"
  python3 "$SAFE_PATH" seal-tree --root "$root" --path "$generation"
  digest="$(
    python3 "$SAFE_PATH" tree-digest --root "$root" \
      --path "$generation" --require-sealed
  )"
  assert_fails "existing writer must prevent snapshot lease acquisition" \
    python3 "$SAFE_PATH" publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command /bin/true >/dev/null
  [[ -d "$generation" && ! -e "$published" ]] ||
    fail "lease acquisition conflict consumed the retryable generation"
  exec {writer_fd}>&-

  generation="$root/generation-post-digest-xattr"
  published="$root/published-post-digest-xattr"
  mkdir -p "$generation/nested"
  printf 'post-digest xattr\n' > "$generation/nested/artifact"
  python3 "$SAFE_PATH" seal-tree --root "$root" --path "$generation"
  digest="$(
    python3 "$SAFE_PATH" tree-digest --root "$root" \
      --path "$generation" --require-sealed
  )"
  pause_marker="$TMPROOT/safe-path-post-digest-xattr.pause"
  continue_marker="$TMPROOT/safe-path-post-digest-xattr.continue"
  output="$TMPROOT/safe-path-post-digest-xattr.output"
  KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_AT=publish-dir-after-final-digest \
    KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_MARKER="$pause_marker" \
    KANBAN_RELEASE_SAFE_PATH_TEST_CONTINUE="$continue_marker" \
    python3 "$SAFE_PATH" publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command /bin/true \
      >"$output" 2>&1 &
  helper_pid=$!
  for _ in {1..3000}; do
    [[ -f "$pause_marker" ]] && break
    kill -0 "$helper_pid" 2>/dev/null ||
      fail "post-digest xattr fixture exited before its checkpoint"
    sleep 0.01
  done
  [[ -f "$pause_marker" ]] || fail "post-digest xattr fixture did not pause"
  chmod u+w "$generation/nested/artifact"
  python3 - "$generation/nested/artifact" <<'PY'
import os
import sys

os.setxattr(sys.argv[1], "user.kanban_release_race", b"forbidden")
PY
  chmod 0444 "$generation/nested/artifact"
  : > "$continue_marker"
  set +e
  wait "$helper_pid"
  status=$?
  set -e
  [[ "$status" -ne 0 && -d "$generation" && ! -e "$published" ]] ||
    fail "post-digest xattr mutation escaped snapshot rollback"

  generation="$root/generation-final-boundary"
  published="$root/published-final-boundary"
  marker="$root/published-final-boundary.published"
  mkdir -p "$generation"
  printf 'final boundary\n' > "$generation/artifact"
  python3 "$SAFE_PATH" seal-tree --root "$root" --path "$generation"
  digest="$(
    python3 "$SAFE_PATH" tree-digest --root "$root" \
      --path "$generation" --require-sealed
  )"
  pause_marker="$TMPROOT/final-boundary.pause"
  continue_marker="$TMPROOT/final-boundary.continue"
  output="$TMPROOT/final-boundary.output"
  KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_AT=publish-dir-post-publish-verified \
    KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_MARKER="$pause_marker" \
    KANBAN_RELEASE_SAFE_PATH_TEST_CONTINUE="$continue_marker" \
    python3 "$SAFE_PATH" publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command /bin/true \
      >"$output" 2>&1 &
  helper_pid=$!
  for _ in {1..3000}; do
    [[ -f "$pause_marker" ]] && break
    kill -0 "$helper_pid" 2>/dev/null ||
      fail "final-boundary fixture exited before its checkpoint"
    sleep 0.01
  done
  [[ -f "$pause_marker" ]] || fail "final-boundary fixture did not pause"
  chmod u+w "$published/artifact"
  python3 - "$published/artifact" <<'PY'
import os
import sys

os.setxattr(sys.argv[1], "user.kanban_release_final_gap", b"forbidden")
PY
  chmod 0444 "$published/artifact"
  : > "$continue_marker"
  set +e
  wait "$helper_pid"
  status=$?
  set -e
  [[ "$status" -ne 0 && -d "$generation" && ! -e "$published" &&
    ! -e "$marker" ]] ||
    fail "mutation at the final commit boundary became authoritative"

  local public_parent="$root/publish-parent-aba"
  local detached_public_parent="$root/publish-parent-aba-detached"
  generation="$root/generation-public-parent-aba"
  published="$public_parent/published"
  marker="$public_parent/published.published"
  mkdir -p "$generation" "$public_parent"
  printf 'public parent ABA\n' > "$generation/artifact"
  python3 "$SAFE_PATH" seal-tree --root "$root" --path "$generation"
  digest="$(
    python3 "$SAFE_PATH" tree-digest --root "$root" \
      --path "$generation" --require-sealed
  )"
  pause_marker="$TMPROOT/public-parent-aba.pause"
  continue_marker="$TMPROOT/public-parent-aba.continue"
  output="$TMPROOT/public-parent-aba.output"
  KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_AT=publish-dir-before-final-public-check \
    KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_MARKER="$pause_marker" \
    KANBAN_RELEASE_SAFE_PATH_TEST_CONTINUE="$continue_marker" \
    python3 "$SAFE_PATH" publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command /bin/true \
      >"$output" 2>&1 &
  helper_pid=$!
  for _ in {1..3000}; do
    [[ -f "$pause_marker" ]] && break
    kill -0 "$helper_pid" 2>/dev/null ||
      fail "public parent ABA fixture exited before its final identity gate"
    sleep 0.01
  done
  [[ -f "$pause_marker" ]] ||
    fail "public parent ABA fixture did not reach its final identity gate"
  mv "$public_parent" "$detached_public_parent"
  mkdir -p "$public_parent"
  printf 'unknown public parent\n' > "$public_parent/sentinel"
  : > "$continue_marker"
  set +e
  wait "$helper_pid"
  status=$?
  set -e
  [[ "$status" -ne 0 &&
    "$(cat "$public_parent/sentinel")" == "unknown public parent" &&
    -d "$generation" && ! -e "$published" && ! -e "$marker" ]] ||
    fail "post-marker-fsync public parent ABA crossed the commit boundary"

  local retained_marker="$root/published-marker-drift.original"
  generation="$root/generation-marker-drift"
  published="$root/published-marker-drift"
  marker="$root/published-marker-drift.published"
  mkdir -p "$generation"
  printf 'marker identity drift\n' > "$generation/artifact"
  python3 "$SAFE_PATH" seal-tree --root "$root" --path "$generation"
  digest="$(
    python3 "$SAFE_PATH" tree-digest --root "$root" \
      --path "$generation" --require-sealed
  )"
  pause_marker="$TMPROOT/marker-identity-drift.pause"
  continue_marker="$TMPROOT/marker-identity-drift.continue"
  output="$TMPROOT/marker-identity-drift.output"
  KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_AT=publish-dir-before-final-public-check \
    KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_MARKER="$pause_marker" \
    KANBAN_RELEASE_SAFE_PATH_TEST_CONTINUE="$continue_marker" \
    python3 "$SAFE_PATH" publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command /bin/true \
      >"$output" 2>&1 &
  helper_pid=$!
  for _ in {1..3000}; do
    [[ -f "$pause_marker" ]] && break
    kill -0 "$helper_pid" 2>/dev/null ||
      fail "marker identity drift fixture exited before rollback"
    sleep 0.01
  done
  [[ -f "$pause_marker" ]] ||
    fail "marker identity drift fixture did not reach rollback boundary"
  mv "$marker" "$retained_marker"
  printf 'unknown marker replacement\n' > "$marker"
  : > "$continue_marker"
  set +e
  wait "$helper_pid"
  status=$?
  set -e
  [[ "$status" -ne 0 &&
    "$(cat "$marker")" == "unknown marker replacement" &&
    -f "$retained_marker" && -d "$published" &&
    ! -e "$published.publishing" ]] ||
    fail "unsafe marker rollback moved or removed an unknown public entry"

  local pinned_valid="$root/generation-pinned-valid"
  local pinned_detached="$root/generation-pinned-detached"
  local verifier="$TMPROOT/pinned-semantic-verifier.py"
  local verifier_ready="$TMPROOT/pinned-semantic.ready"
  local verifier_read="$TMPROOT/pinned-semantic.read"
  local verifier_go="$TMPROOT/pinned-semantic.go"
  local verifier_restore="$TMPROOT/pinned-semantic.restore"
  generation="$root/generation-pinned-invalid"
  published="$root/published-pinned-invalid"
  mkdir -p "$generation" "$pinned_valid"
  printf 'invalid\n' > "$generation/semantic"
  printf 'valid\n' > "$pinned_valid/semantic"
  python3 "$SAFE_PATH" seal-tree --root "$root" --path "$generation"
  python3 "$SAFE_PATH" seal-tree --root "$root" --path "$pinned_valid"
  digest="$(
    python3 "$SAFE_PATH" tree-digest --root "$root" \
      --path "$generation" --require-sealed
  )"
  cat > "$verifier" <<'PY'
#!/usr/bin/env python3
import os
import pathlib
import sys
import time

public = pathlib.Path(sys.argv[1])
ready = pathlib.Path(sys.argv[2])
go = pathlib.Path(sys.argv[3])
read = pathlib.Path(sys.argv[4])
restore = pathlib.Path(sys.argv[5])
ready.write_text("ready\n", encoding="utf-8")
deadline = time.monotonic() + 30
while not go.exists():
    if time.monotonic() >= deadline:
        raise SystemExit("timed out waiting for swapped verifier tree")
    time.sleep(0.01)
pinned_fd = os.environ.get("KANBAN_RELEASE_PINNED_STAGE_FD")
if pinned_fd is None:
    stage = public
else:
    descriptor = int(pinned_fd)
    metadata = os.fstat(descriptor)
    expected = (
        int(os.environ["KANBAN_RELEASE_PINNED_STAGE_DEV"]),
        int(os.environ["KANBAN_RELEASE_PINNED_STAGE_INO"]),
    )
    if (metadata.st_dev, metadata.st_ino) != expected:
        raise SystemExit("pinned verifier fd identity drifted")
    stage = pathlib.Path(f"/proc/self/fd/{descriptor}")
content = (stage / "semantic").read_text(encoding="utf-8")
read.write_text(content, encoding="utf-8")
deadline = time.monotonic() + 30
while not restore.exists():
    if time.monotonic() >= deadline:
        raise SystemExit("timed out waiting for public-path restoration")
    time.sleep(0.01)
raise SystemExit(0 if content == "valid\n" else 42)
PY
  chmod 0755 "$verifier"
  python3 "$SAFE_PATH" publish-dir --root "$root" \
    --source "$generation" --destination "$published" \
    --expected-tree-sha256 "$digest" \
    --verify-command "$verifier" "$generation" "$verifier_ready" \
      "$verifier_go" "$verifier_read" "$verifier_restore" \
    >"$TMPROOT/pinned-semantic.output" 2>&1 &
  helper_pid=$!
  for _ in {1..3000}; do
    [[ -f "$verifier_ready" ]] && break
    kill -0 "$helper_pid" 2>/dev/null ||
      fail "pinned semantic verifier exited before the swap"
    sleep 0.01
  done
  [[ -f "$verifier_ready" ]] || fail "pinned semantic verifier never started"
  mv "$generation" "$pinned_detached"
  mv "$pinned_valid" "$generation"
  : > "$verifier_go"
  for _ in {1..3000}; do
    [[ -f "$verifier_read" ]] && break
    kill -0 "$helper_pid" 2>/dev/null ||
      fail "pinned semantic verifier exited before reading its tree"
    sleep 0.01
  done
  [[ -f "$verifier_read" ]] || fail "pinned semantic verifier never read a tree"
  mv "$generation" "$pinned_valid"
  mv "$pinned_detached" "$generation"
  : > "$verifier_restore"
  set +e
  wait "$helper_pid"
  status=$?
  set -e
  [[ "$status" -ne 0 && -d "$generation" && ! -e "$published" ]] ||
    fail "public-path semantic verifier approved a different tree than the pinned snapshot"
  [[ "$(cat "$verifier_read")" == "invalid" ]] ||
    fail "semantic verifier did not read the pinned invalid snapshot"

  generation="$root/generation-lease-unsupported"
  published="$root/published-lease-unsupported"
  mkdir -p "$generation"
  printf 'lease unsupported\n' > "$generation/artifact"
  python3 "$SAFE_PATH" seal-tree --root "$root" --path "$generation"
  digest="$(
    python3 "$SAFE_PATH" tree-digest --root "$root" \
      --path "$generation" --require-sealed
  )"
  output="$TMPROOT/lease-unsupported.output"
  assert_fails "F_SETLEASE unsupported must fail closed" \
    env KANBAN_RELEASE_SAFE_PATH_TEST_SETLEASE_ERRNO=ENOSYS \
    python3 "$SAFE_PATH" publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command /bin/true \
      >"$output"
  grep -Fqx \
    "error: required Linux primitive unavailable: fcntl(F_SETLEASE): ENOSYS (Function not implemented); no fallback while leasing $generation/artifact" \
    "$output" ||
    fail "unsupported lease failure lacked the canonical primitive diagnostic"
  [[ -d "$generation" && ! -e "$published" ]] ||
    fail "unsupported lease primitive consumed the generation"

  output="$TMPROOT/sigio-symbol-missing.output"
  assert_fails "missing SIGIO symbol must fail closed" \
    python3 "$missing_primitive_fixture" "$SAFE_PATH" signal SIGIO \
      publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command /bin/true \
      >"$output"
  grep -Fqx \
    'error: required Linux primitive unavailable: signal(SIGIO): ENOSYS (Function not implemented); no fallback' \
    "$output" ||
    fail "missing SIGIO symbol lacked the canonical primitive diagnostic"

  output="$TMPROOT/setlease-symbol-missing.output"
  assert_fails "missing F_SETLEASE symbol must fail closed" \
    python3 "$missing_primitive_fixture" "$SAFE_PATH" fcntl F_SETLEASE \
      publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command /bin/true \
      >"$output"
  grep -Fqx \
    "error: required Linux primitive unavailable: fcntl(F_SETLEASE): ENOSYS (Function not implemented); no fallback while leasing $generation/artifact" \
    "$output" ||
    fail "missing F_SETLEASE symbol lacked the canonical primitive diagnostic"

  output="$TMPROOT/getlease-symbol-missing.output"
  assert_fails "missing F_GETLEASE symbol must fail closed" \
    python3 "$missing_primitive_fixture" "$SAFE_PATH" fcntl F_GETLEASE \
      publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command /bin/true \
      >"$output"
  grep -Fqx \
    "error: required Linux primitive unavailable: fcntl(F_GETLEASE): ENOSYS (Function not implemented); no fallback while inspecting lease for $generation/artifact" \
    "$output" ||
    fail "missing F_GETLEASE symbol lacked the canonical primitive diagnostic"

  output="$TMPROOT/getlease-enosys.output"
  assert_fails "F_GETLEASE ENOSYS must fail closed" \
    env KANBAN_RELEASE_SAFE_PATH_TEST_GETLEASE_ERRNO=ENOSYS \
    python3 "$SAFE_PATH" publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command /bin/true \
      >"$output"
  grep -Fqx \
    "error: required Linux primitive unavailable: fcntl(F_GETLEASE): ENOSYS (Function not implemented); no fallback while inspecting lease for $generation/artifact" \
    "$output" ||
    fail "F_GETLEASE ENOSYS lacked the canonical primitive diagnostic"
  [[ -d "$generation" && ! -e "$published" ]] ||
    fail "missing lease inspection primitive consumed the generation"

  verifier="$TMPROOT/pinned-fd-required.py"
  cat > "$verifier" <<'PY'
#!/usr/bin/env python3
import os
import pathlib

descriptor = int(os.environ["KANBAN_RELEASE_PINNED_STAGE_FD"])
metadata = os.fstat(descriptor)
expected = (
    int(os.environ["KANBAN_RELEASE_PINNED_STAGE_DEV"]),
    int(os.environ["KANBAN_RELEASE_PINNED_STAGE_INO"]),
)
if (metadata.st_dev, metadata.st_ino) != expected:
    raise SystemExit("pinned fd identity mismatch")
if (pathlib.Path(f"/proc/self/fd/{descriptor}") / "artifact").read_text() != "fd\n":
    raise SystemExit("pinned fd content mismatch")
PY
  chmod 0755 "$verifier"
  generation="$root/generation-dropped-verifier-fd"
  published="$root/published-dropped-verifier-fd"
  mkdir -p "$generation"
  printf 'fd\n' > "$generation/artifact"
  python3 "$SAFE_PATH" seal-tree --root "$root" --path "$generation"
  digest="$(
    python3 "$SAFE_PATH" tree-digest --root "$root" \
      --path "$generation" --require-sealed
  )"
  output="$TMPROOT/dropped-verifier-fd.output"
  assert_fails "semantic verifier without inherited pinned fd" \
    env KANBAN_RELEASE_SAFE_PATH_TEST_DROP_PINNED_FD=1 \
    python3 "$SAFE_PATH" publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command "$verifier" \
      >"$output"
  [[ -d "$generation" && ! -e "$published" &&
    ! -e "$published.published" ]] ||
    fail "verifier fd inheritance failure became authoritative"

  generation="$root/generation-before-crash"
  published="$root/published-before-crash"
  mkdir -p "$generation/nested"
  printf 'directory crash before rename\n' > "$generation/nested/artifact"
  python3 "$SAFE_PATH" seal-tree --root "$root" --path "$generation"
  digest="$(
    python3 "$SAFE_PATH" tree-digest --root "$root" \
      --path "$generation" --require-sealed
  )"
  set +e
  KANBAN_RELEASE_SAFE_PATH_TEST_EXIT_AT=publish-dir-before-rename \
    python3 "$SAFE_PATH" publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command /bin/true
  status=$?
  set -e
  [[ "$status" -eq 86 && -d "$generation" && ! -e "$published" ]] ||
    fail "directory pre-rename crash did not leave a retryable source"
  KANBAN_RELEASE_SAFE_PATH_TEST_TRACE="$trace" \
    python3 "$SAFE_PATH" publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command /bin/true
  [[ ! -e "$generation" && -d "$published" &&
    -f "$published.published" ]] ||
    fail "directory publish retry did not converge"

  generation="$root/generation-unknown-recovery"
  published="$root/published-unknown-recovery"
  mkdir -p "$published/nested"
  printf 'unknown unjournaled generation\n' > "$published/nested/artifact"
  python3 "$SAFE_PATH" seal-tree --root "$root" --path "$published"
  assert_fails "unjournaled destination recovery" \
    python3 "$SAFE_PATH" publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 \
      aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
      --verify-command /bin/true >/dev/null
  [[ ! -e "$generation" && -d "$published" &&
    "$(cat "$published/nested/artifact")" == "unknown unjournaled generation" ]] ||
    fail "unjournaled destination was moved or rewritten during recovery"

  generation="$root/generation-after-crash"
  published="$root/published-after-crash"
  mkdir -p "$generation/nested"
  printf 'directory crash after rename\n' > "$generation/nested/artifact"
  python3 "$SAFE_PATH" seal-tree --root "$root" --path "$generation"
  digest="$(
    python3 "$SAFE_PATH" tree-digest --root "$root" \
      --path "$generation" --require-sealed
  )"
  set +e
  KANBAN_RELEASE_SAFE_PATH_TEST_EXIT_AT=publish-dir-after-rename \
    python3 "$SAFE_PATH" publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command /bin/true
  status=$?
  set -e
  [[ "$status" -eq 86 && ! -e "$generation" && -d "$published" &&
    ! -e "$published.published" ]] ||
    fail "directory post-rename crash was incorrectly authoritative"
  python3 "$SAFE_PATH" publish-dir --root "$root" \
    --source "$generation" --destination "$published" \
    --expected-tree-sha256 "$digest" --verify-command /bin/true
  [[ ! -e "$generation" && -d "$published" &&
    -f "$published.published" ]] ||
    fail "unmarked directory crash recovery did not converge"

  generation="$root/generation-rollback"
  published="$root/published-rollback"
  mkdir -p "$generation/nested"
  printf 'directory rollback retry\n' > "$generation/nested/artifact"
  python3 "$SAFE_PATH" seal-tree --root "$root" --path "$generation"
  digest="$(
    python3 "$SAFE_PATH" tree-digest --root "$root" \
      --path "$generation" --require-sealed
  )"
  failure_trace="$TMPROOT/directory-parent-fsync-failure.trace"
  assert_fails "directory parent fsync rollback" \
    env KANBAN_RELEASE_SAFE_PATH_TEST_TRACE="$failure_trace" \
      KANBAN_RELEASE_SAFE_PATH_TEST_FAIL_AT=publish-dir-parent \
    python3 "$SAFE_PATH" publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command /bin/true >/dev/null
  grep -Fqx "publish-dir-parent	$root" "$failure_trace" ||
    fail "directory publish failure trace lacks the injected parent checkpoint"
  grep -Fqx "publish-dir-rollback-parent	$root" "$failure_trace" ||
    fail "directory publish failure trace lacks rollback durability"
  [[ -d "$generation" && ! -e "$published" ]] ||
    fail "directory fsync rollback did not restore a retryable source"
  retry_trace="$TMPROOT/directory-parent-fsync-retry.trace"
  KANBAN_RELEASE_SAFE_PATH_TEST_TRACE="$retry_trace" \
    python3 "$SAFE_PATH" publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command /bin/true
  grep -Fqx "publish-dir-parent	$root" "$retry_trace" ||
    fail "directory publish retry trace lacks durable parent evidence"
  [[ ! -e "$generation" && -d "$published" &&
    -f "$published.published" ]] ||
    fail "directory rollback retry did not converge"

  generation="$root/generation-marker-fsync-rollback"
  published="$root/published-marker-fsync-rollback"
  mkdir -p "$generation/nested"
  printf 'authoritative marker rollback retry\n' > "$generation/nested/artifact"
  python3 "$SAFE_PATH" seal-tree --root "$root" --path "$generation"
  digest="$(
    python3 "$SAFE_PATH" tree-digest --root "$root" \
      --path "$generation" --require-sealed
  )"
  failure_trace="$TMPROOT/directory-marker-fsync-failure.trace"
  assert_fails "authoritative marker parent fsync rollback" \
    env KANBAN_RELEASE_SAFE_PATH_TEST_TRACE="$failure_trace" \
      KANBAN_RELEASE_SAFE_PATH_TEST_FAIL_AT=publish-dir-marker-commit-parent \
    python3 "$SAFE_PATH" publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command /bin/true >/dev/null
  grep -Fqx "publish-dir-marker-commit-parent	$root" "$failure_trace" ||
    fail "marker publish failure trace lacks the injected parent checkpoint"
  grep -Fqx "publish-dir-rollback-parent	$root" "$failure_trace" ||
    fail "marker publish failure trace lacks rollback durability"
  [[ -d "$generation" && ! -e "$published" &&
    ! -e "$published.published" ]] ||
    fail "marker fsync rollback did not restore a non-authoritative source"
  retry_trace="$TMPROOT/directory-marker-fsync-retry.trace"
  KANBAN_RELEASE_SAFE_PATH_TEST_TRACE="$retry_trace" \
    python3 "$SAFE_PATH" publish-dir --root "$root" \
      --source "$generation" --destination "$published" \
      --expected-tree-sha256 "$digest" --verify-command /bin/true
  grep -Fqx "publish-dir-marker-commit-parent	$root" "$retry_trace" ||
    fail "marker publish retry trace lacks durable parent evidence"
  [[ ! -e "$generation" && -d "$published" &&
    -f "$published.published" ]] ||
    fail "marker publish retry did not converge"

  python3 - "$trace" "$root" <<'PY'
import pathlib
import sys

trace = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
root = pathlib.Path(sys.argv[2])
required = {
    f"durable-tree-file\t{root / 'generation-before-crash/nested/artifact'}",
    f"durable-tree-directory\t{root / 'generation-before-crash/nested'}",
    f"durable-tree-directory\t{root / 'generation-before-crash'}",
    f"publish-dir-parent\t{root}",
}
missing = required.difference(trace)
assert not missing, f"missing per-phase directory durability evidence: {sorted(missing)}"
PY

  local cleanup_tree="$root/cleanup-known"
  local cleanup_detached="$root/cleanup-detached"
  local cleanup_dev cleanup_ino
  local cleanup_inserted="$root/cleanup-inserted"
  mkdir -p "$cleanup_inserted/nested"
  printf 'known cleanup payload\n' > "$cleanup_inserted/nested/payload"
  read -r cleanup_dev cleanup_ino < <(stat -Lc '%d %i' "$cleanup_inserted")
  pause_marker="$TMPROOT/remove-tree-insert.pause"
  continue_marker="$TMPROOT/remove-tree-insert.continue"
  output="$TMPROOT/remove-tree-insert.output"
  KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_AT=remove-tree-before-delete \
    KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_MARKER="$pause_marker" \
    KANBAN_RELEASE_SAFE_PATH_TEST_CONTINUE="$continue_marker" \
    python3 "$SAFE_PATH" remove-tree --root "$root" --path "$cleanup_inserted" \
      --expected-dev "$cleanup_dev" --expected-ino "$cleanup_ino" \
      >"$output" 2>&1 &
  helper_pid=$!
  for _ in {1..3000}; do
    [[ -f "$pause_marker" ]] && break
    kill -0 "$helper_pid" 2>/dev/null ||
      fail "remove-tree insertion fixture exited before its authority checkpoint"
    sleep 0.01
  done
  [[ -f "$pause_marker" ]] ||
    fail "remove-tree insertion fixture did not pause"
  printf 'unknown cleanup insertion\n' > "$cleanup_inserted/unknown"
  : > "$continue_marker"
  set +e
  wait "$helper_pid"
  status=$?
  set -e
  [[ "$status" -ne 0 &&
    "$(cat "$cleanup_inserted/unknown")" == "unknown cleanup insertion" ]] ||
    fail "cleanup authority deleted an entry inserted after authorization"

  mkdir -p "$cleanup_tree/nested"
  printf 'known cleanup payload\n' > "$cleanup_tree/nested/payload"
  read -r cleanup_dev cleanup_ino < <(stat -Lc '%d %i' "$cleanup_tree")
  pause_marker="$TMPROOT/remove-tree-aba.pause"
  continue_marker="$TMPROOT/remove-tree-aba.continue"
  output="$TMPROOT/remove-tree-aba.output"
  KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_AT=remove-tree-before-delete \
    KANBAN_RELEASE_SAFE_PATH_TEST_PAUSE_MARKER="$pause_marker" \
    KANBAN_RELEASE_SAFE_PATH_TEST_CONTINUE="$continue_marker" \
    python3 "$SAFE_PATH" remove-tree --root "$root" --path "$cleanup_tree" \
      --expected-dev "$cleanup_dev" --expected-ino "$cleanup_ino" \
      >"$output" 2>&1 &
  helper_pid=$!
  for _ in {1..3000}; do
    [[ -f "$pause_marker" ]] && break
    kill -0 "$helper_pid" 2>/dev/null ||
      fail "remove-tree ABA fixture exited before its checkpoint"
    sleep 0.01
  done
  [[ -f "$pause_marker" ]] || fail "remove-tree ABA fixture did not pause"
  mv "$cleanup_tree" "$cleanup_detached"
  mkdir -p "$cleanup_tree"
  printf 'unknown cleanup sentinel\n' > "$cleanup_tree/sentinel"
  : > "$continue_marker"
  set +e
  wait "$helper_pid"
  status=$?
  set -e
  [[ "$status" -ne 0 &&
    "$(cat "$cleanup_tree/sentinel")" == "unknown cleanup sentinel" ]] ||
    fail "cleanup path ABA deleted an unknown replacement"
}

[[ -x "$SOURCE_GATE" ]] || fail "missing executable release source gate: $SOURCE_GATE"
[[ -x "$COHORT_WRAPPER" ]] || fail "missing executable release cohort wrapper: $COHORT_WRAPPER"
[[ -x "$ARTIFACT_MANIFEST" ]] || fail "missing executable release artifact manifest: $ARTIFACT_MANIFEST"
[[ -x "$EMBED_DEB" ]] || fail "missing executable Debian provenance injector: $EMBED_DEB"
[[ -x "$PACKAGE_CLI" ]] || fail "missing executable CLI package script: $PACKAGE_CLI"
[[ -x "$SAFE_PATH" ]] || fail "missing executable release safe-path helper: $SAFE_PATH"
[[ -f "$ROOT/docs/release/derived-projection-v2-source-map.json" ]] ||
  fail "missing committed Projection v2 source map"

assert_source_gate
assert_cohort_wrapper
assert_live_embed_mutation_is_ignored
assert_live_source_gate_mutation_is_ignored
assert_release_identity_resume_binding
assert_cli_package_embeds_cohort
assert_desktop_embed_rejects_hostile_paths
assert_publish_source_parent_identity
assert_safe_path_atomic_durability
echo "ok: release provenance source, package, artifact, and drift gates are verified"
