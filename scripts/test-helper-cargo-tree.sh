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

grep -Fqx '    just feature-p kanban-cli "tantivy-backend,oxigraph-backend"' "$ROOT/justfile" ||
  fail "projection-release-cohort must test the CLI with Tantivy and Oxigraph"
grep -Fqx '    just feature-p kanban-server "tantivy-backend,oxigraph-backend"' "$ROOT/justfile" ||
  fail "projection-release-cohort must test the server with Tantivy and Oxigraph"
grep -Fqx '    scripts/package-cli-linux.sh --format deb --no-default-features --features "tantivy-backend,oxigraph-backend"' "$ROOT/justfile" ||
  fail "cli-package must build the explicit Tantivy/Oxigraph release cohort"
grep -Fqx '    just projection-release-cohort' "$ROOT/justfile" ||
  fail "release must invoke projection-release-cohort"

echo "ok: default helper isolation and the explicit projection release cohort are verified"
