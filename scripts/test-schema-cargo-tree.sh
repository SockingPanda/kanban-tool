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
  if grep -Eiq 'xtask v|schemars v1\.|jsonschema v|kanban-protocol feature "schema(-tool)?"' <<<"$tree"; then
    grep -Ein 'xtask v|schemars v1\.|jsonschema v|kanban-protocol feature "schema(-tool)?"' <<<"$tree" >&2 || true
    fail "$package runtime graph 包含 schema tooling"
  fi
}

assert_mcp_tree_uses_runtime_schema_only() {
  local tree
  tree="$(scripts/cargo-build-lock.sh -- cargo tree -p kanban-mcp --all-features --target all --edges normal,features --locked)"
  if grep -Eiq 'xtask v|jsonschema v' <<<"$tree"; then
    grep -Ein 'xtask v|jsonschema v' <<<"$tree" >&2 || true
    fail "kanban-mcp runtime graph 包含离线 schema tooling"
  fi
  grep -Eq 'kanban-protocol feature "schema"' <<<"$tree" \
    || fail "kanban-mcp runtime graph 缺少 contract schema derives"
  grep -Eq 'schemars v1\.' <<<"$tree" \
    || fail "kanban-mcp runtime graph 缺少 MCP tool schema support"
}

# `kanban-protocol` default graph 是产品采用基线；leaf tool 单独做正向验证。
# 产品 adopter 必须扫描所有 feature 与 target，且不得依赖 tooling owner。
assert_default_tree_excludes_schema_tooling kanban-protocol
for package in kanban-core kanban-service kanban-client \
  kanban-cli; do
  assert_default_tree_excludes_schema_tooling "$package" --all-features
done
# MCP 的 tool input 会在运行时生成 JSON Schema，因此允许 `schemars` 与
# `kanban-protocol/schema`，但仍禁止依赖离线 `xtask`/`jsonschema` 工具链。
assert_mcp_tree_uses_runtime_schema_only
for package in kanban-server kanban-desktop; do
  assert_default_tree_excludes_schema_tooling "$package" --all-features
done

schema_tree="$(scripts/cargo-build-lock.sh -- cargo tree -p xtask --all-features --target all --edges normal,features --locked)"
grep -Eq '^xtask v' <<<"$schema_tree" \
  || fail "tool graph 缺少 xtask root"
grep -Eq 'kanban-protocol feature "schema"' <<<"$schema_tree" \
  || fail "xtask graph 缺少 kanban-protocol/schema"
grep -Eq 'schemars v1\.' <<<"$schema_tree" \
  || fail "xtask graph 缺少 schemars"
grep -Eq 'jsonschema v' <<<"$schema_tree" \
  || fail "xtask graph 缺少 jsonschema"
grep -Eq 'sha2 v' <<<"$schema_tree" \
  || fail "xtask graph 缺少 sha2"

echo "ok: 产品 runtime graph 与独立 xtask tooling graph 已隔离"
