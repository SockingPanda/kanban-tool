#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
TARGET_DIR="$($LOCK --print-target-dir)"
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RUN_DIR="${RUN_DIR:-$ROOT/.omx/ultragoal/evidence/G004-ontology-bootstrap-verify-e2e-$TIMESTAMP}"
BOARD="default"
ACTOR="ontology-bootstrap-e2e"
MODEL="mock-bootstrap-e2e-v1"
DIMENSIONS=4
VECTOR_CONFIG="$RUN_DIR/vector-config.toml"
PORT_FILE="$RUN_DIR/mock-ollama.port"
MOCK_LOG="$RUN_DIR/mock-ollama.log"
COMMAND_LOG="$RUN_DIR/commands.log"
SUMMARY="$RUN_DIR/summary.json"
MOCK_PID=""

command -v jq >/dev/null 2>&1 || {
  echo "error: jq is required" >&2
  exit 1
}
command -v python3 >/dev/null 2>&1 || {
  echo "error: python3 is required" >&2
  exit 1
}

mkdir -p "$RUN_DIR"

cleanup() {
  if [[ -n "$MOCK_PID" ]] && kill -0 "$MOCK_PID" >/dev/null 2>&1; then
    kill "$MOCK_PID" >/dev/null 2>&1 || true
    wait "$MOCK_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

log() {
  printf '%s\n' "$*" | tee -a "$COMMAND_LOG" >&2
}

fail() {
  echo "error: $*" >&2
  exit 1
}

ensure_kanban_bin() {
  if [[ -n "${KANBAN_BIN:-}" ]]; then
    [[ -x "$KANBAN_BIN" || "$(command -v "$KANBAN_BIN" 2>/dev/null)" ]] || {
      fail "KANBAN_BIN is not executable or on PATH: $KANBAN_BIN"
    }
    return 0
  fi
  log "+ $LOCK -- cargo build -q -p kanban-cli --bin kanban"
  "$LOCK" -- cargo build -q -p kanban-cli --bin kanban
  KANBAN_BIN="$TARGET_DIR/debug/kanban"
  [[ -x "$KANBAN_BIN" ]] || fail "expected debug kanban binary at $KANBAN_BIN"
}

start_mock_ollama() {
  python3 -u - "$PORT_FILE" >"$MOCK_LOG" 2>&1 <<'PY' &
import json
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

port_file = sys.argv[1]

def vector_for(text, dimensions):
    lower = str(text).lower()
    if "matching-bootstrap" in lower or "crash-safe" in lower:
        base = [1.0, 0.0, 0.0, 0.0]
    elif "unrelated-bootstrap" in lower:
        base = [0.2, 1.0, 0.0, 0.0]
    else:
        base = [0.0, 0.0, 1.0, 0.0]
    if dimensions <= len(base):
        return base[:dimensions]
    return base + [0.0] * (dimensions - len(base))

class Handler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        return

    def do_POST(self):
        if self.path != "/api/embed":
            self.send_response(404)
            self.end_headers()
            return
        length = int(self.headers.get("Content-Length", "0"))
        payload = json.loads(self.rfile.read(length) or b"{}")
        dimensions = int(payload.get("dimensions") or 4)
        value = payload.get("input", "")
        inputs = value if isinstance(value, list) else [value]
        response = {"embeddings": [vector_for(item, dimensions) for item in inputs]}
        body = json.dumps(response).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
with open(port_file, "w", encoding="utf-8") as handle:
    handle.write(str(server.server_address[1]))
server.serve_forever()
PY
  MOCK_PID=$!
  for _ in $(seq 1 100); do
    [[ -s "$PORT_FILE" ]] && return 0
    sleep 0.05
  done
  fail "mock Ollama server did not start"
}

kb() {
  local db="$1"
  shift
  "$KANBAN_BIN" --db "$db" --board "$BOARD" --actor "$ACTOR" --json "$@"
}

run_json() {
  local output="$1"
  shift
  log "+ $* > $output"
  "$@" >"$output"
}

assert_jq() {
  local file="$1"
  local filter="$2"
  local message="$3"
  if ! jq -e "$filter" "$file" >/dev/null; then
    echo "assertion failed: $message" >&2
    echo "file: $file" >&2
    exit 1
  fi
}

counts_json() {
  local db="$1"
  local output="$2"
  python3 - "$db" >"$output" <<'PY'
import json
import sqlite3
import sys

db = sys.argv[1]
conn = sqlite3.connect(db)

def one(sql, args=()):
    return conn.execute(sql, args).fetchone()[0]

data = {
    "labels": one("SELECT COUNT(*) FROM labels"),
    "target_label_count": one("SELECT COUNT(*) FROM labels WHERE name='database'"),
    "label_semantics": one("SELECT COUNT(*) FROM label_semantics"),
    "label_atoms": one("SELECT COUNT(*) FROM label_atoms"),
    "task_labels": one("SELECT COUNT(*) FROM task_labels"),
    "task_events": one("SELECT COUNT(*) FROM task_events"),
    "label_created_events": one("SELECT COUNT(*) FROM task_events WHERE kind='label.created'"),
    "task_label_added_events": one("SELECT COUNT(*) FROM task_events WHERE kind='task.label.added'"),
    "compensation_events": one("SELECT COUNT(*) FROM task_events WHERE payload_json LIKE '%bootstrap verification compensation%'"),
    "actions": one("SELECT COUNT(*) FROM label_ontology_actions"),
    "bootstrap_actions": one("SELECT COUNT(*) FROM label_ontology_actions WHERE action_type='bootstrap_label'"),
    "effects": one("SELECT COUNT(*) FROM label_ontology_action_atom_effects"),
}
print(json.dumps(data, sort_keys=True))
PY
}

init_case() {
  local db="$1"
  local task_file="$2"
  run_json "$task_file.init.json" kb "$db" init
  run_json "$task_file.task.json" kb "$db" task create \
    "matching-bootstrap crash-safe target" \
    --description "matching-bootstrap crash-safe task description for staged verification."
  jq -r '.data.id' "$task_file.task.json"
}

bootstrap_args_matching() {
  local task_id="$1"
  printf '%s\0' label bootstrap "$task_id" database \
    --description "matching-bootstrap database persistence work" \
    --applies-when "matching-bootstrap SQLite migration evidence" \
    --positive-example "matching-bootstrap new table migration" \
    --verify --min-verify-score 0.50 --vector-config "$VECTOR_CONFIG"
}

bootstrap_args_low_score() {
  local task_id="$1"
  printf '%s\0' label bootstrap "$task_id" database \
    --description "unrelated-bootstrap database persistence work" \
    --applies-when "unrelated-bootstrap evidence" \
    --positive-example "unrelated-bootstrap migration" \
    --verify --min-verify-score 0.50 --vector-config "$VECTOR_CONFIG"
}

read_null_args() {
  local -n out_ref="$1"
  mapfile -d '' -t out_ref
}

wait_for_marker() {
  local pid="$1"
  local marker="$2"
  local stderr="$3"
  for _ in $(seq 1 200); do
    [[ -s "$marker" ]] && return 0
    if ! kill -0 "$pid" >/dev/null 2>&1; then
      cat "$stderr" >&2 || true
      fail "process exited before marker $marker"
    fi
    sleep 0.05
  done
  cat "$stderr" >&2 || true
  fail "timed out waiting for marker $marker"
}

assert_zero_bootstrap_state() {
  local counts="$1"
  assert_jq "$counts" '.target_label_count == 0' "target label must not exist"
  assert_jq "$counts" '.label_semantics == 0' "label_semantics must be empty"
  assert_jq "$counts" '.label_atoms == 0' "label_atoms must be empty"
  assert_jq "$counts" '.task_labels == 0' "task_labels must be empty"
  assert_jq "$counts" '.bootstrap_actions == 0 and .effects == 0' "ontology bootstrap actions/effects must be empty"
  assert_jq "$counts" '.compensation_events == 0' "compensation events must not exist"
}

run_kill_case() {
  local stage="$1"
  local db="$RUN_DIR/kill-$stage.db"
  local prefix="$RUN_DIR/kill-$stage"
  local task_id
  task_id="$(init_case "$db" "$prefix")"
  local marker="$prefix.marker"
  local out="$prefix.bootstrap.out.json"
  local err="$prefix.bootstrap.err"
  local args=()
  read_null_args args < <(bootstrap_args_matching "$task_id")

  log "+ failpoint $stage kill bootstrap"
  env \
    KANBAN_BOOTSTRAP_VERIFY_TEST_FAILPOINT="$stage" \
    KANBAN_BOOTSTRAP_VERIFY_TEST_MARKER="$marker" \
    KANBAN_BOOTSTRAP_VERIFY_TEST_SLEEP_MS=60000 \
    "$KANBAN_BIN" --db "$db" --board "$BOARD" --actor "$ACTOR" --json "${args[@]}" \
    >"$out" 2>"$err" &
  local pid=$!
  wait_for_marker "$pid" "$marker" "$err"
  kill "$pid" >/dev/null 2>&1 || true
  wait "$pid" >/dev/null 2>&1 || true
  counts_json "$db" "$prefix.counts.json"

  if [[ "$stage" == "after_commit" ]]; then
    assert_jq "$prefix.counts.json" '.target_label_count == 1 and .label_semantics == 1 and .label_atoms > 0 and .task_labels == 1' "after_commit kill must preserve complete canonical state"
    assert_jq "$prefix.counts.json" '.bootstrap_actions == 1 and .actions == 1 and .effects == .label_atoms' "after_commit kill must preserve one root action and effects"
    assert_jq "$prefix.counts.json" '.task_label_added_events == 1 and .compensation_events == 0' "after_commit kill must preserve binding event without compensation"
  else
    assert_zero_bootstrap_state "$prefix.counts.json"
    assert_jq "$prefix.counts.json" '.label_created_events == 0 and .task_label_added_events == 0' "$stage kill must not write bootstrap events"
  fi
}

run_conflict_case() {
  local kind="$1"
  local db="$RUN_DIR/conflict-$kind.db"
  local prefix="$RUN_DIR/conflict-$kind"
  local task_id
  task_id="$(init_case "$db" "$prefix")"
  local marker="$prefix.marker"
  local out="$prefix.bootstrap.out.json"
  local err="$prefix.bootstrap.err"
  local args=()
  read_null_args args < <(bootstrap_args_matching "$task_id")

  log "+ failpoint before_commit conflict $kind"
  env \
    KANBAN_BOOTSTRAP_VERIFY_TEST_FAILPOINT="before_commit" \
    KANBAN_BOOTSTRAP_VERIFY_TEST_MARKER="$marker" \
    KANBAN_BOOTSTRAP_VERIFY_TEST_SLEEP_MS=1500 \
    "$KANBAN_BIN" --db "$db" --board "$BOARD" --actor "$ACTOR" --json "${args[@]}" \
    >"$out" 2>"$err" &
  local pid=$!
  wait_for_marker "$pid" "$marker" "$err"

  if [[ "$kind" == "task" ]]; then
    run_json "$prefix.concurrent-task-update.json" kb "$db" task update "$task_id" \
      --title "matching-bootstrap crash-safe target changed"
  else
    run_json "$prefix.concurrent-label-create.json" kb "$db" label create concurrent-ontology
  fi

  set +e
  wait "$pid"
  local status=$?
  set -e
  [[ "$status" -ne 0 ]] || fail "bootstrap conflict case $kind unexpectedly succeeded"
  grep -q "label bootstrap verification conflict" "$err" || {
    cat "$err" >&2 || true
    fail "bootstrap conflict case $kind did not report expected conflict"
  }
  counts_json "$db" "$prefix.counts.json"
  assert_zero_bootstrap_state "$prefix.counts.json"
}

run_threshold_failure_case() {
  local db="$RUN_DIR/threshold-fail.db"
  local prefix="$RUN_DIR/threshold-fail"
  local task_id
  task_id="$(init_case "$db" "$prefix")"
  run_json "$prefix.status-before.json" kb "$db" label atom-index status --vector-config "$VECTOR_CONFIG"
  local args=()
  read_null_args args < <(bootstrap_args_low_score "$task_id")
  set +e
  "$KANBAN_BIN" --db "$db" --board "$BOARD" --actor "$ACTOR" --json "${args[@]}" \
    >"$prefix.bootstrap.out.json" 2>"$prefix.bootstrap.err"
  local status=$?
  set -e
  [[ "$status" -ne 0 ]] || fail "threshold failure unexpectedly succeeded"
  grep -q "below min_verify_score" "$prefix.bootstrap.err" || {
    cat "$prefix.bootstrap.err" >&2 || true
    fail "threshold failure did not report score gate"
  }
  run_json "$prefix.status-after.json" kb "$db" label atom-index status --vector-config "$VECTOR_CONFIG"
  jq -S '.data | {dirty, board_dirty, generation, diagnostics}' "$prefix.status-before.json" >"$prefix.status-before.canon.json"
  jq -S '.data | {dirty, board_dirty, generation, diagnostics}' "$prefix.status-after.json" >"$prefix.status-after.canon.json"
  cmp "$prefix.status-before.canon.json" "$prefix.status-after.canon.json"
  counts_json "$db" "$prefix.counts.json"
  assert_zero_bootstrap_state "$prefix.counts.json"
}

run_success_case() {
  local db="$RUN_DIR/success.db"
  local prefix="$RUN_DIR/success"
  local task_id
  task_id="$(init_case "$db" "$prefix")"
  local args=()
  read_null_args args < <(bootstrap_args_matching "$task_id")
  run_json "$prefix.bootstrap.json" "$KANBAN_BIN" --db "$db" --board "$BOARD" --actor "$ACTOR" --json "${args[@]}"
  assert_jq "$prefix.bootstrap.json" '.data.verification.score >= 0.50 and .data.task.labels[0].name == "database"' "success bootstrap should verify and bind label"
  counts_json "$db" "$prefix.counts.json"
  assert_jq "$prefix.counts.json" '.target_label_count == 1 and .label_semantics == 1 and .label_atoms > 0 and .task_labels == 1' "success must write canonical state"
  assert_jq "$prefix.counts.json" '.bootstrap_actions == 1 and .actions == 1 and .effects == .label_atoms' "success must write one root action and matching effects"
  assert_jq "$prefix.counts.json" '.compensation_events == 0' "success must not write compensation event"
  run_json "$prefix.status.json" kb "$db" label atom-index status --vector-config "$VECTOR_CONFIG"
  assert_jq "$prefix.status.json" '(.data.board_dirty == true) or (.data.dirty == true) or ((.data.diagnostics // []) | index("label_atom_index_dirty"))' "success must mark label atom index dirty"
}

ensure_kanban_bin
start_mock_ollama
PORT="$(cat "$PORT_FILE")"
cat >"$VECTOR_CONFIG" <<EOF
[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:$PORT"
model = "$MODEL"
dimensions = $DIMENSIONS
EOF

run_kill_case before_verify
run_kill_case before_commit
run_kill_case during_commit
run_kill_case after_commit
run_conflict_case task
run_conflict_case ontology
run_threshold_failure_case
run_success_case

jq -n \
  --arg run_dir "$RUN_DIR" \
  '{
    status: "passed",
    run_dir: $run_dir,
    evidence: [
      "before_verify kill left zero bootstrap canonical/action/event state",
      "before_commit kill left zero bootstrap canonical/action/event state",
      "during_commit kill rolled back uncommitted canonical/action/event state",
      "after_commit kill preserved complete canonical state with one root action and effects",
      "task change after verification returned conflict without bootstrap writes",
      "ontology change after verification returned conflict without bootstrap writes",
      "threshold failure left canonical state and index dirty state unchanged",
      "success wrote canonical state, one root action, matching effects, binding, and dirty marker",
      "no scenario wrote bootstrap verification compensation events"
    ]
  }' >"$SUMMARY"

log "PASS ontology bootstrap verify E2E"
log "summary: $SUMMARY"
log "raw JSON artifacts: $RUN_DIR"
