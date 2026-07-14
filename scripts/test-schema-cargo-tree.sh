#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail() {
  echo "error: $*" >&2
  exit 1
}

assert_default_tree_excludes_schema_tooling() {
  local package="$1"
  shift
  local tree
  tree="$(scripts/cargo-build-lock.sh -- cargo tree -p "$package" "$@" --target all --edges normal,features --locked)"
  if grep -Eiq 'kanban-schema-tool v|schemars v1\.|jsonschema v|kanban-contract feature "schema(-tool)?"' <<<"$tree"; then
    grep -Ein 'kanban-schema-tool v|schemars v1\.|jsonschema v|kanban-contract feature "schema(-tool)?"' <<<"$tree" >&2 || true
    fail "$package runtime graph contains schema tooling"
  fi
}

# kanban-contract default graph 是产品采用基线；leaf tool 独立正向验证。
# 产品 adopter 必须扫描所有 feature 与 target，且不得依赖 tooling owner。
assert_default_tree_excludes_schema_tooling kanban-contract
for package in kanban-cli kanban-server kanban-sqlite kanban-desktop \
  kanban-vector-lancedb kanban-graph-oxigraph; do
  assert_default_tree_excludes_schema_tooling "$package" --all-features
done

schema_tree="$(scripts/cargo-build-lock.sh -- cargo tree -p kanban-schema-tool --all-features --target all --edges normal,features --locked)"
grep -Eq '^kanban-schema-tool v' <<<"$schema_tree" \
  || fail "tool graph is missing kanban-schema-tool root"
grep -Eq 'kanban-contract feature "schema"' <<<"$schema_tree" \
  || fail "kanban-schema-tool graph is missing kanban-contract/schema"
grep -Eq 'schemars v1\.' <<<"$schema_tree" \
  || fail "kanban-schema-tool graph is missing schemars"
grep -Eq 'jsonschema v' <<<"$schema_tree" \
  || fail "kanban-schema-tool graph is missing jsonschema"
grep -Eq 'sha2 v' <<<"$schema_tree" \
  || fail "kanban-schema-tool graph is missing sha2"

echo "ok: 产品 runtime graph 与独立 kanban-schema-tool tooling graph 已隔离"
