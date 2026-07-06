#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
TARGET_DIR="$($LOCK --print-target-dir)"
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RUN_DIR="${RUN_DIR:-$ROOT/.omx/ultragoal/evidence/G018-ontology-negative-atom-e2e-$TIMESTAMP}"
BOARD="default"
ACTOR="ontology-negative-e2e"
MODEL="mock-negative-atom-e2e-v1"
DIMENSIONS=4
POSITIVE_ATOM_TEXT="cli-positive-surface command behavior and help output"
NEGATIVE_ATOM_TEXT="cli-negative-suppressor non-command maintenance without user-visible cli behavior"

command -v jq >/dev/null 2>&1 || {
  echo "error: jq is required" >&2
  exit 1
}
command -v python3 >/dev/null 2>&1 || {
  echo "error: python3 is required" >&2
  exit 1
}

mkdir -p "$RUN_DIR"
COMMAND_LOG="$RUN_DIR/commands.log"
DB="$RUN_DIR/disposable.db"
WAIVER_DB="$RUN_DIR/waiver.db"
IMPORTED_DB="$RUN_DIR/imported.db"
EXPORT_JSONL="$RUN_DIR/export.jsonl"
VECTOR_CONFIG="$RUN_DIR/vector-config.toml"
PORT_FILE="$RUN_DIR/mock-ollama.port"
MOCK_LOG="$RUN_DIR/mock-ollama.log"
SUMMARY="$RUN_DIR/summary.json"
MOCK_PID=""

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
    if "cli-false-positive-source" in lower or "cli-regression-control" in lower:
        base = [1.0, 1.0, 0.0, 0.0]
    elif "cli-negative-suppressor" in lower:
        base = [0.0, 1.0, 0.0, 0.0]
    elif "cli-positive-control" in lower or "cli-positive-surface" in lower:
        base = [1.0, 0.0, 0.0, 0.0]
    elif "desktop-visual" in lower or "palette" in lower or "layout" in lower:
        base = [0.0, 0.0, 1.0, 0.0]
    else:
        base = [0.0, 0.0, 0.0, 1.0]
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
  "$KANBAN_BIN" --db "$DB" --board "$BOARD" --actor "$ACTOR" --json "$@"
}

kb_waiver() {
  "$KANBAN_BIN" --db "$WAIVER_DB" --board "$BOARD" --actor "$ACTOR" --json "$@"
}

kb_imported() {
  "$KANBAN_BIN" --db "$IMPORTED_DB" --board "$BOARD" --actor "$ACTOR" --json "$@"
}

run_json() {
  local output="$1"
  shift
  log "+ $* > $output"
  "$@" >"$output"
}

run_fail() {
  local stdout="$1"
  local stderr="$2"
  shift 2
  log "+ ! $* > $stdout 2> $stderr"
  if "$@" >"$stdout" 2>"$stderr"; then
    cat "$stdout" >&2 || true
    fail "command unexpectedly succeeded: $*"
  fi
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

assert_jq_with_args() {
  local file="$1"
  shift
  local message="${!#}"
  set -- "${@:1:$(($# - 1))}"
  if ! jq -e "$@" "$file" >/dev/null; then
    echo "assertion failed: $message" >&2
    echo "file: $file" >&2
    exit 1
  fi
}

assert_file_contains() {
  local file="$1"
  local expected="$2"
  local message="$3"
  if ! grep -F "$expected" "$file" >/dev/null; then
    echo "assertion failed: $message" >&2
    echo "file: $file" >&2
    cat "$file" >&2 || true
    exit 1
  fi
}

action_count() {
  local db="$1"
  local action_type="$2"
  python3 - "$db" "$action_type" <<'PY'
import sqlite3
import sys

db, action_type = sys.argv[1], sys.argv[2]
conn = sqlite3.connect(db)
print(conn.execute(
    "SELECT COUNT(*) FROM label_ontology_actions WHERE action_type=?",
    (action_type,),
).fetchone()[0])
PY
}

ensure_kanban_bin
start_mock_ollama
PORT="$(cat "$PORT_FILE")"

run_json "$RUN_DIR/init.json" kb init
run_json "$RUN_DIR/vector.configure.json" kb vector configure \
  --vector-config "$VECTOR_CONFIG" \
  --endpoint "http://127.0.0.1:$PORT" \
  --model "$MODEL" \
  --dimensions "$DIMENSIONS"

run_json "$RUN_DIR/label.cli.json" kb label create cli
run_json "$RUN_DIR/semantics.cli.json" kb label semantics upsert cli \
  --description "Command-line workflow label for existing command flags and help text." \
  --applies-when "$POSITIVE_ATOM_TEXT" \
  --positive-example "cli-positive-control shipped command behavior should stay covered."
run_json "$RUN_DIR/label.ui.json" kb label create ui
run_json "$RUN_DIR/semantics.ui.json" kb label semantics upsert ui \
  --description "Desktop visual presentation work." \
  --applies-when "desktop-visual layout palette polish."

run_json "$RUN_DIR/task.source.json" kb task create \
  "cli-false-positive-source release note cleanup" \
  --description "cli-false-positive-source maintenance note: it mentions CLI but is not a shipped command behavior change."
SOURCE_TASK_REF="$(jq -r '.data.ref' "$RUN_DIR/task.source.json")"

run_json "$RUN_DIR/task.positive-control.json" kb task create \
  "cli-positive-control command behavior fixture" \
  --description "cli-positive-control verifies command flags and help output stay selected."
POSITIVE_CONTROL_REF="$(jq -r '.data.ref' "$RUN_DIR/task.positive-control.json")"

run_json "$RUN_DIR/task.regression-control.json" kb task create \
  "cli-regression-control ambiguous maintenance fixture" \
  --description "cli-regression-control should reveal if the negative atom suppresses too much."
REGRESSION_CONTROL_REF="$(jq -r '.data.ref' "$RUN_DIR/task.regression-control.json")"

run_json "$RUN_DIR/atom-index.rebuild.before.json" kb label atom-index rebuild --vector-config "$VECTOR_CONFIG"
run_json "$RUN_DIR/atom-index.status.before.json" kb label atom-index status --vector-config "$VECTOR_CONFIG"
assert_jq "$RUN_DIR/atom-index.status.before.json" '.data.enabled == true and (.data.dirty == false or .data.dirty == null)' "initial atom index should be clean"

run_json "$RUN_DIR/before.source.suggest.json" kb label suggest "$SOURCE_TASK_REF" \
  --vector-config "$VECTOR_CONFIG" --min-score 0.50 --candidate-limit 8 --atom-limit 16 --max-selected-labels 1
run_json "$RUN_DIR/before.positive-control.suggest.json" kb label suggest "$POSITIVE_CONTROL_REF" \
  --vector-config "$VECTOR_CONFIG" --min-score 0.50 --candidate-limit 8 --atom-limit 16 --max-selected-labels 1
assert_jq "$RUN_DIR/before.source.suggest.json" '(.data.selected_labels // []) | any(.label_name == "cli")' "source should select cli before the negative atom"
assert_jq "$RUN_DIR/before.positive-control.suggest.json" '(.data.selected_labels // []) | any(.label_name == "cli")' "positive control should select cli before the negative atom"

SOURCE_SCORE="$(jq -r '((.data.selected_labels // []) + (.data.candidates // []) | map(select(.label_name == "cli")) | first.score) // 0' "$RUN_DIR/before.source.suggest.json")"
jq -n \
  --slurpfile suggest "$RUN_DIR/before.source.suggest.json" \
  --arg atom "$NEGATIVE_ATOM_TEXT" \
  --arg score "$SOURCE_SCORE" '
  {
    actor: {name: "ontology-negative-e2e-labeler", type: "agent", agent_type: "e2e-script"},
    agent_candidates_json: ([{label: "cli", confidence: 0.88, reason: "source selected cli but should be suppressed"}] | tojson),
    suggestion_snapshot_json: ($suggest[0].data | tojson),
    final_decision_json: ({accepted_labels: [], rejected_labels: ["cli"], rationale: "source task is a false-positive for cli"} | tojson),
    suggest_coverage: $suggest[0].data.coverage,
    suggest_coverage_cosine: $suggest[0].data.coverage_cosine,
    suggest_residual_norm: $suggest[0].data.residual_norm,
    suggest_needs_new_label: $suggest[0].data.needs_new_label,
    suggest_degraded: $suggest[0].data.degraded,
    diagnostics_json: ($suggest[0].data.diagnostics | tojson),
    capture_fingerprint: "ontology-e2e-negative-atom-source",
    signals: [{
      kind: "false_positive",
      target_label_ref: "cli",
      related_labels_json: "[]",
      proposed_action: "add_negative_atom",
      candidate_atom: {polarity: "negative", kind: "excludes_when", text: $atom},
      proposed_label_name: null,
      proposal_json: "{}",
      agent_selected: true,
      suggest_state: "selected",
      suggest_score: ($score | tonumber),
      suggest_rank: 1,
      final_selected: false,
      rationale: "The cli label was selected for a non-command maintenance task.",
      confidence: 0.9,
      signal_key: "ontology-e2e-cli-false-positive-negative-atom"
    }]
  }' >"$RUN_DIR/record.input.json"

run_json "$RUN_DIR/record.output.json" kb label ontology record "$SOURCE_TASK_REF" --input "$RUN_DIR/record.input.json"
SIGNAL_ID="$(jq -r '.data.signals[0].id' "$RUN_DIR/record.output.json")"
assert_jq "$RUN_DIR/record.output.json" '.data.signals[0].status == "open"' "record should create an open signal"

run_json "$RUN_DIR/confirm.output.json" kb label ontology confirm "$SIGNAL_ID" \
  --reason "E2E reviewer confirmed this false-positive signal." \
  --actor-type agent --agent-type e2e-script
assert_jq "$RUN_DIR/confirm.output.json" '.data.action_type == "confirm" and .data.validation_status == "not_required"' "confirm should be lifecycle-only"

run_json "$RUN_DIR/apply.output.json" kb label ontology apply atom "$SIGNAL_ID" \
  --label cli --kind excludes-when --text "$NEGATIVE_ATOM_TEXT" \
  --reason "E2E adds a generalized false-positive suppression atom." \
  --actor-type agent --agent-type e2e-script
APPLY_ACTION_ID="$(jq -r '.data.id' "$RUN_DIR/apply.output.json")"
RESULT_ATOM_ID="$(jq -r '.data.result_atom_id' "$RUN_DIR/apply.output.json")"
RESULT_ATOM_HASH="$(jq -r '.data.result_atom_content_hash' "$RUN_DIR/apply.output.json")"
TARGET_LABEL_ID="$(jq -r '.data.target_label_id' "$RUN_DIR/apply.output.json")"
assert_jq "$RUN_DIR/apply.output.json" '.data.action_type == "add_negative_atom" and .data.validation_status == "pending"' "apply should create a pending negative atom action"
assert_jq "$RUN_DIR/apply.output.json" '.data.result_atom_id != null and .data.result_atom_content_hash != null' "apply should expose result atom evidence"

run_json "$RUN_DIR/atom-index.status.after-apply.json" kb label atom-index status --vector-config "$VECTOR_CONFIG"
assert_jq "$RUN_DIR/atom-index.status.after-apply.json" '(.data.board_dirty == true) or (.data.dirty == true) or ((.data.diagnostics // []) | index("label_atom_index_dirty"))' "negative atom apply should dirty the atom index"
run_json "$RUN_DIR/atom-index.rebuild.after-apply.json" kb label atom-index rebuild --vector-config "$VECTOR_CONFIG"
run_json "$RUN_DIR/atom-index.status.after-rebuild.json" kb label atom-index status --vector-config "$VECTOR_CONFIG"
assert_jq "$RUN_DIR/atom-index.status.after-rebuild.json" '.data.enabled == true and (.data.board_dirty == false or .data.board_dirty == null) and (.data.dirty == false or .data.dirty == null)' "rebuild should leave clean atom index"

run_json "$RUN_DIR/after.source.suggest.json" kb label suggest "$SOURCE_TASK_REF" \
  --vector-config "$VECTOR_CONFIG" --min-score 0.50 --candidate-limit 8 --atom-limit 16 --max-selected-labels 1
run_json "$RUN_DIR/after.positive-control.suggest.json" kb label suggest "$POSITIVE_CONTROL_REF" \
  --vector-config "$VECTOR_CONFIG" --min-score 0.50 --candidate-limit 8 --atom-limit 16 --max-selected-labels 1
assert_jq "$RUN_DIR/after.source.suggest.json" '((.data.selected_labels // []) | any(.label_name == "cli")) | not' "source should be suppressed after the negative atom"
assert_jq_with_args "$RUN_DIR/after.source.suggest.json" --arg atom "$RESULT_ATOM_ID" '
  (((.data.selected_labels // []) + (.data.candidates // [])
    | map(select(.label_name == "cli")) | first) as $target
    | ($target.score < 0.50)
      and (($target.negative_evidence_atoms // []) | any((.id // .atom_id) == $atom)))
' "source candidate should carry negative evidence from the new atom"
assert_jq "$RUN_DIR/after.positive-control.suggest.json" '(.data.selected_labels // []) | any(.label_name == "cli")' "positive control should not regress after the negative atom"

BEFORE_VALIDATE_COUNT="$(action_count "$DB" validate)"
run_fail "$RUN_DIR/validate.no-control.out.json" "$RUN_DIR/validate.no-control.err" kb label ontology validate \
  --trusted --status passed \
  --reason "Negative validation must require controls or a waiver." \
  --vector-config "$VECTOR_CONFIG" \
  --min-score 0.50 --candidate-limit 8 --atom-limit 16 --max-selected-labels 1 \
  "$APPLY_ACTION_ID" "$SIGNAL_ID"
assert_file_contains "$RUN_DIR/validate.no-control.err" "positive control" "missing controls should be rejected"
[[ "$(action_count "$DB" validate)" == "$BEFORE_VALIDATE_COUNT" ]] || fail "missing-control rejection must not write a validate action"

run_fail "$RUN_DIR/validate.agent-waiver.out.json" "$RUN_DIR/validate.agent-waiver.err" kb label ontology validate \
  --trusted --status passed \
  --reason "Agent waiver must be rejected." \
  --positive-control-waiver "No stable positive control exists." \
  --vector-config "$VECTOR_CONFIG" \
  --min-score 0.50 --candidate-limit 8 --atom-limit 16 --max-selected-labels 1 \
  --actor-type agent --agent-type e2e-script \
  "$APPLY_ACTION_ID" "$SIGNAL_ID"
assert_file_contains "$RUN_DIR/validate.agent-waiver.err" "actor_type=user" "agent waiver should be rejected"
[[ "$(action_count "$DB" validate)" == "$BEFORE_VALIDATE_COUNT" ]] || fail "agent-waiver rejection must not write a validate action"

cp "$DB" "$WAIVER_DB"
WAIVER_REASON="  E2E reviewer supplied a temporary positive-control waiver.  "
run_json "$RUN_DIR/validate.user-waiver.json" kb_waiver label ontology validate \
  --trusted --status passed \
  --reason "User waiver should be captured as trusted evidence." \
  --positive-control-waiver "$WAIVER_REASON" \
  --vector-config "$VECTOR_CONFIG" \
  --min-score 0.50 --candidate-limit 8 --atom-limit 16 --max-selected-labels 1 \
  --actor-type user \
  "$APPLY_ACTION_ID" "$SIGNAL_ID"
assert_jq_with_args "$RUN_DIR/validate.user-waiver.json" --arg reason "$WAIVER_REASON" '
  (.data.validation_json | fromjson) as $v
  | $v.manual.cases[0].after.positive_control_waiver.reason == $reason
' "user waiver reason should be preserved in trusted evidence"

jq -n \
  --arg signal "$SIGNAL_ID" \
  --arg target "$TARGET_LABEL_ID" \
  --arg atom "$RESULT_ATOM_ID" \
  --arg hash "$RESULT_ATOM_HASH" '
  {
    evidence_type: "trusted_automated",
    embedding_model: "forged-external",
    solver_options: {candidate_limit: 8, atom_limit: 16, min_score: 0.5},
    index: {status: "ready", dirty: false, generation: 1},
    cases: [{
      signal_id: $signal,
      case_type: "negative_atom",
      passed: true,
      target_label_id: $target,
      before: {target: {label_id: $target, selected: true, score: 0.71}},
      after: {
        target: {label_id: $target, selected: false, score: 0.14},
        evidence_atoms: [],
        negative_evidence_atoms: [{id: $atom, atom_id: $atom, content_hash: $hash, label_id: $target}],
        positive_controls: [{passed: true, regressed: false}]
      }
    }]
  }' >"$RUN_DIR/validate.fake-external.json"
run_fail "$RUN_DIR/validate.fake-external.out.json" "$RUN_DIR/validate.fake-external.err" kb label ontology validate \
  --status passed \
  --reason "External JSON must not forge trusted positive controls." \
  --input "$RUN_DIR/validate.fake-external.json" \
  "$APPLY_ACTION_ID" "$SIGNAL_ID"
assert_file_contains "$RUN_DIR/validate.fake-external.err" "external attestation cannot close ontology signals" "external trusted-looking JSON must not pass"

run_json "$RUN_DIR/validate.regression.json" kb label ontology validate \
  --trusted --status failed \
  --reason "Regression control is intentionally suppressed and should record a failed attempt." \
  --positive-control "$REGRESSION_CONTROL_REF" \
  --vector-config "$VECTOR_CONFIG" \
  --min-score 0.50 --candidate-limit 8 --atom-limit 16 --max-selected-labels 1 \
  --actor-type agent --agent-type e2e-script \
  "$APPLY_ACTION_ID" "$SIGNAL_ID"
assert_jq "$RUN_DIR/validate.regression.json" '.data.action_type == "validate" and .data.validation_status == "failed"' "regression control should record failed validation"
assert_jq "$RUN_DIR/validate.regression.json" '(.data.validation_json | fromjson).manual.cases[0].after.positive_controls[0].regressed == true' "failed validation should record positive control regression"
run_json "$RUN_DIR/after-failed.show.json" kb label ontology show "$SIGNAL_ID"
assert_jq "$RUN_DIR/after-failed.show.json" '.data.signal.status == "confirmed"' "failed validation should leave signal confirmed"

run_json "$RUN_DIR/validate.passed.json" kb label ontology validate \
  --trusted --status passed \
  --reason "Trusted E2E collector proved false-positive suppression and positive-control retention." \
  --positive-control "$POSITIVE_CONTROL_REF" \
  --vector-config "$VECTOR_CONFIG" \
  --min-score 0.50 --candidate-limit 8 --atom-limit 16 --max-selected-labels 1 \
  --actor-type agent --agent-type e2e-script \
  "$APPLY_ACTION_ID" "$SIGNAL_ID"
assert_jq "$RUN_DIR/validate.passed.json" '.data.action_type == "validate" and .data.validation_status == "passed"' "positive control should allow trusted passed validation"
assert_jq_with_args "$RUN_DIR/validate.passed.json" --arg atom "$RESULT_ATOM_ID" --arg model "$MODEL" '
  (.data.validation_json | fromjson) as $v
  | $v.manual.evidence_type == "trusted_automated"
    and $v.manual.collector.source == "label_ontology_validate_trusted"
    and $v.manual.embedding_model == $model
    and $v.manual.cases[0].case_type == "negative_atom"
    and $v.manual.cases[0].after.target.selected == false
    and ($v.manual.cases[0].after.negative_evidence_atoms | any((.id // .atom_id) == $atom))
    and (($v.manual.cases[0].after.evidence_atoms // []) | any((.id // .atom_id) == $atom) | not)
    and $v.manual.cases[0].after.positive_controls[0].passed == true
    and $v.manual.cases[0].after.positive_controls[0].regressed == false
' "passed validation should prove negative evidence slot and non-regressed positive control"

run_json "$RUN_DIR/after-passed.show.json" kb label ontology show "$SIGNAL_ID"
assert_jq "$RUN_DIR/after-passed.show.json" '.data.signal.status == "resolved"' "passed trusted validation should resolve signal"
run_json "$RUN_DIR/atom.explain.json" kb label atoms explain "$RESULT_ATOM_ID"
assert_jq_with_args "$RUN_DIR/atom.explain.json" --arg signal "$SIGNAL_ID" --arg action "$APPLY_ACTION_ID" '
  (.data.legacy_untracked == false)
  and (.data.provenance_actions | any(.action.id == $action))
  and (.data.supporting_signals | any(.signal.id == $signal))
  and ((.data.validation_history // []) | any(.action.validation_status == "passed"))
' "atom explain should trace negative atom action, signal, and passed validation"

run_json "$RUN_DIR/doctor.original.json" kb doctor
assert_jq "$RUN_DIR/doctor.original.json" '.data.ok == true' "original DB doctor should pass"
run_json "$RUN_DIR/export.output.json" kb export --out "$EXPORT_JSONL"
run_json "$RUN_DIR/import.output.json" kb_imported import --input "$EXPORT_JSONL" --replace
run_json "$RUN_DIR/doctor.imported.json" kb_imported doctor
assert_jq "$RUN_DIR/doctor.imported.json" '.data.ok == true' "imported DB doctor should pass"
run_json "$RUN_DIR/imported.show.json" kb_imported label ontology show "$SIGNAL_ID"
assert_jq "$RUN_DIR/imported.show.json" '.data.signal.status == "resolved"' "imported DB should preserve resolved signal"
run_json "$RUN_DIR/imported.atom.explain.json" kb_imported label atoms explain "$RESULT_ATOM_ID"
assert_jq_with_args "$RUN_DIR/imported.atom.explain.json" --arg signal "$SIGNAL_ID" --arg action "$APPLY_ACTION_ID" '
  (.data.provenance_actions | any(.action.id == $action))
  and (.data.supporting_signals | any(.signal.id == $signal))
' "imported DB should preserve negative atom provenance"

jq -n \
  --arg run_dir "$RUN_DIR" \
  --arg source_task "$SOURCE_TASK_REF" \
  --arg positive_control "$POSITIVE_CONTROL_REF" \
  --arg regression_control "$REGRESSION_CONTROL_REF" \
  --arg signal "$SIGNAL_ID" \
  --arg action "$APPLY_ACTION_ID" \
  --arg atom "$RESULT_ATOM_ID" \
  --arg atom_hash "$RESULT_ATOM_HASH" \
  '{
    status: "passed",
    run_dir: $run_dir,
    disposable_db_removed: (env.KEEP_DISPOSABLE_DB != "1"),
    source_task: $source_task,
    positive_control: $positive_control,
    regression_control: $regression_control,
    signal_id: $signal,
    apply_action_id: $action,
    result_atom_id: $atom,
    result_atom_content_hash: $atom_hash,
    evidence: [
      "false-positive source selected cli before the negative atom",
      "negative atom apply wrote one pending add_negative_atom action and dirtied the atom index",
      "after rebuild, source was suppressed with result atom in negative_evidence_atoms",
      "missing controls and agent waiver failed without writing validate actions",
      "user waiver evidence preserved the supplied reason in an isolated DB",
      "external trusted-looking JSON could not pass",
      "regression control recorded failed validation and left source signal confirmed",
      "positive control passed and resolved the source signal",
      "atom explain and JSONL import preserved negative atom provenance"
    ]
  }' >"$SUMMARY"

if [[ "${KEEP_DISPOSABLE_DB:-0}" != "1" ]]; then
  rm -f "$DB" "$WAIVER_DB" "$IMPORTED_DB"
  rm -rf "$RUN_DIR/index"
fi

log "PASS ontology negative-atom E2E"
log "summary: $SUMMARY"
log "raw JSON artifacts: $RUN_DIR"
