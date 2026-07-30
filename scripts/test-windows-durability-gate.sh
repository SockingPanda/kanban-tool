#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

grep -Fqx '    just check-windows-p kanban-local' "$ROOT/justfile" ||
  {
    echo "error: release must cross-check kanban-local for the Windows target" >&2
    exit 1
  }

for workflow in \
  "$ROOT/.github/workflows/pr.yml" \
  "$ROOT/.github/workflows/full-ci.yml"
do
  grep -Fq '  rust-windows-durability:' "$workflow" ||
    {
      echo "error: missing rust-windows-durability job in $workflow" >&2
      exit 1
    }
  grep -Fq 'runs-on: windows-latest' "$workflow" ||
    {
      echo "error: Windows durability job is not native in $workflow" >&2
      exit 1
    }
  grep -Fq 'cargo test --locked -p kanban-local durable_' "$workflow" ||
    {
      echo "error: missing native filesystem durability tests in $workflow" >&2
      exit 1
    }
  grep -Fq "cargo test --locked -p kanban-sqlite --features 'tantivy-backend,oxigraph-backend' --lib" "$workflow" ||
    {
      echo "error: missing native projection backend durability tests in $workflow" >&2
      exit 1
    }
  grep -Fq "suite::maintenance_runtime::" "$workflow" ||
    {
      echo "error: missing native maintenance recovery tests in $workflow" >&2
      exit 1
    }
  grep -Fq "suite::projection_v2::" "$workflow" ||
    {
      echo "error: missing native generation fencing tests in $workflow" >&2
      exit 1
    }
  grep -Fq '      - rust-windows-durability' "$workflow" ||
    {
      echo "error: Windows durability job is not required by the result gate in $workflow" >&2
      exit 1
    }
done

echo "ok: Windows projection durability is cross-compiled and natively tested"
