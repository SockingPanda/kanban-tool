#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
RELEASE_FEATURES="tantivy-backend,oxigraph-backend"
cd "$ROOT"

fail() {
  echo "error: $*" >&2
  exit 1
}

cargo_tree() {
  local package="$1"
  shift
  "$LOCK" -- cargo tree --locked -p "$package" --target all --edges normal,features "$@"
}

assert_tree_excludes_heavy_helpers() {
  local package="$1"
  shift
  local tree
  tree="$(cargo_tree "$package" "$@")"
  if grep -Eiq 'lancedb|oxigraph|arrow|kanban-vector-lancedb|kanban-graph-oxigraph' <<<"$tree"; then
    echo "error: $package depends on a helper/heavy derived-store crate" >&2
    grep -Ein 'lancedb|oxigraph|arrow|kanban-vector-lancedb|kanban-graph-oxigraph' <<<"$tree" >&2 || true
    exit 1
  fi
}

assert_tree_includes() {
  local package="$1"
  local pattern="$2"
  shift 2
  local tree
  tree="$(cargo_tree "$package" "$@")"
  grep -Eiq "$pattern" <<<"$tree" || {
    echo "error: $package cargo tree did not include expected pattern: $pattern" >&2
    exit 1
  }
}

assert_release_backend_cohort() {
  local package="$1"
  local tree
  tree="$(cargo_tree "$package" --no-default-features --features "$RELEASE_FEATURES")"

  grep -Eiq '(^|[[:space:]])tantivy($|[[:space:]])' <<<"$tree" ||
    fail "$package release cohort is missing Tantivy"
  grep -Eiq 'kanban-graph-oxigraph' <<<"$tree" ||
    fail "$package release cohort is missing the Oxigraph backend adapter"
  grep -Eiq '(^|[[:space:]])oxigraph($|[[:space:]])' <<<"$tree" ||
    fail "$package release cohort is missing Oxigraph"

  if grep -Eiq 'lancedb|arrow|kanban-vector-lancedb' <<<"$tree"; then
    grep -Ein 'lancedb|arrow|kanban-vector-lancedb' <<<"$tree" >&2 || true
    fail "$package release cohort unexpectedly links the LanceDB helper graph"
  fi
}

# Default product graphs stay helper-light. Release binaries opt into the
# currently supported in-process maintenance backends explicitly; that
# intentional cohort is verified separately below.
assert_tree_excludes_heavy_helpers kanban-cli
assert_tree_excludes_heavy_helpers kanban-server
assert_tree_excludes_heavy_helpers kanban-sqlite

assert_release_backend_cohort kanban-cli
assert_release_backend_cohort kanban-server

assert_tree_includes kanban-vector-lancedb '(^|[[:space:]])lancedb($|[[:space:]])'
assert_tree_includes kanban-vector-lancedb 'arrow'
assert_tree_includes kanban-graph-oxigraph '(^|[[:space:]])oxigraph($|[[:space:]])'

python3 - "$ROOT/justfile" "$ROOT/scripts/release-cohort.sh" <<'PY'
import pathlib
import sys

justfile = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
wrapper = pathlib.Path(sys.argv[2]).read_text(encoding="utf-8").splitlines()

def recipe(name):
    try:
        start = justfile.index(f"{name}:")
    except ValueError as error:
        raise SystemExit(f"error: missing just recipe: {name}") from error
    body = []
    for line in justfile[start + 1 :]:
        if line.startswith("    "):
            body.append(line.strip())
            continue
        if line == "":
            break
        break
    return body

if recipe("projection-release-cohort") != [
    'just feature-p kanban-cli "tantivy-backend,oxigraph-backend"',
    'just feature-p kanban-server "tantivy-backend,oxigraph-backend"',
]:
    raise SystemExit(
        "error: projection-release-cohort must execute the exact CLI/server release feature pair"
    )
if recipe("cli-package") != [
    'scripts/package-cli-linux.sh --format deb --no-default-features --features "tantivy-backend,oxigraph-backend"',
]:
    raise SystemExit(
        "error: cli-package must build the explicit Tantivy/Oxigraph release cohort"
    )
if recipe("release") != ["scripts/release-cohort.sh"]:
    raise SystemExit("error: release must enter the single-process release cohort wrapper")

expected_wrapper_steps = [
    "just affected-self-test",
    "just schema-contract",
    "just audit",
    "just rust-full",
    "just check-windows-p kanban-local",
    "just projection-release-cohort",
    "just bench-check",
    "just target-tools",
    "just cli-package",
    "just cli-package-layout",
    "just desktop-package-config",
    "just desktop-package",
    "just desktop-package-layout",
    "just smoke",
    "just diff-check",
]
wrapper_steps = [line.strip() for line in wrapper if line.startswith("just ")]
if wrapper_steps != expected_wrapper_steps:
    raise SystemExit(
        "error: release cohort wrapper must execute the exact canonical recipe sequence"
    )
PY

echo "ok: default helper isolation and the explicit projection release cohort are verified"
