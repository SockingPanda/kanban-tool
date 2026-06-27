#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

assert_tree_excludes_heavy_helpers() {
  local package="$1"
  local tree
  tree="$(cargo tree -p "$package")"
  if grep -Eiq '(^|[[:space:]])(lancedb|oxigraph|arrow|kanban-vector-lancedb|kanban-graph-oxigraph)($|[[:space:]])' <<<"$tree"; then
    echo "error: $package depends on a helper/heavy derived-store crate" >&2
    grep -Ein 'lancedb|oxigraph|arrow|kanban-vector-lancedb|kanban-graph-oxigraph' <<<"$tree" >&2 || true
    exit 1
  fi
}

assert_tree_includes() {
  local package="$1"
  local pattern="$2"
  local tree
  tree="$(cargo tree -p "$package")"
  grep -Eiq "$pattern" <<<"$tree" || {
    echo "error: $package cargo tree did not include expected pattern: $pattern" >&2
    exit 1
  }
}

assert_tree_excludes_heavy_helpers kanban-cli
assert_tree_excludes_heavy_helpers kanban-server
assert_tree_includes kanban-vector-lancedb '(^|[[:space:]])lancedb($|[[:space:]])'
assert_tree_includes kanban-vector-lancedb '(^|[[:space:]])arrow($|[[:space:]])'
assert_tree_includes kanban-graph-oxigraph '(^|[[:space:]])oxigraph($|[[:space:]])'

echo "ok: helper dependencies stay behind helper binaries"
