#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

LOCK="$ROOT/scripts/cargo-build-lock.sh"
KB=("$LOCK" --lane cli -- cargo run -q -p kanban-cli --bin kanban -- --db "$TMPDIR/kb.db" --json)

"${KB[@]}" init >/dev/null
task_json="$("${KB[@]}" task create "v1 smoke task" --description "ready spec" --max-retries 2)"
task_id="$(printf '%s' "$task_json" | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["id"])')"

"${KB[@]}" dispatch --once --command "printf 'smoke log\n'" --log-dir "$TMPDIR/logs" >/dev/null
"${KB[@]}" run logs "$("${KB[@]}" runs "$task_id" | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"][0]["id"])')" >/dev/null
"${KB[@]}" stats >/dev/null
"${KB[@]}" doctor >/dev/null
"${KB[@]}" checkpoint >/dev/null
"${KB[@]}" vacuum >/dev/null
"${KB[@]}" backup --out "$TMPDIR/backup.sqlite" >/dev/null
"${KB[@]}" export --out "$TMPDIR/board.jsonl" >/dev/null

"$LOCK" --lane cli -- cargo run -q -p kanban-cli --bin kanban -- --db "$TMPDIR/imported.db" --json import --input "$TMPDIR/board.jsonl" --replace >/dev/null
"$LOCK" --lane cli -- cargo run -q -p kanban-cli --bin kanban -- --db "$TMPDIR/imported.db" --json task show "$task_id" >/dev/null

echo "v1 local smoke passed in $ROOT"
