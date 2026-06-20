#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KANBAN_BIN="${KANBAN_BIN:-kanban}"
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RUN_DIR="${RUN_DIR:-$ROOT/.omx/ultragoal/evidence/G021-ontology-positive-atom-e2e-$TIMESTAMP}"
BOARD="default"
ACTOR="ontology-e2e"
MODEL="mock-ontology-e2e-v1"
DIMENSIONS=4
ATOM_TEXT="terminal-surface command arguments, help output, and machine-readable command behavior"

command -v jq >/dev/null 2>&1 || {
  echo "error: jq is required" >&2
  exit 1
}
command -v python3 >/dev/null 2>&1 || {
  echo "error: python3 is required" >&2
  exit 1
}
[[ -x "$KANBAN_BIN" || "$(command -v "$KANBAN_BIN" 2>/dev/null)" ]] || {
  echo "error: KANBAN_BIN is not executable or on PATH: $KANBAN_BIN" >&2
  exit 1
}

mkdir -p "$RUN_DIR"
COMMAND_LOG="$RUN_DIR/commands.log"
SOURCE_DB="$RUN_DIR/source.db"
DB="$RUN_DIR/disposable.db"
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
  printf '%s\n' "$*" | tee -a "$COMMAND_LOG"
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

canonical_task_labels() {
  jq -S '.data.labels // []' "$1"
}

canonical_semantics() {
  jq -S '.data' "$1"
}

canonical_atoms() {
  jq -S '.data | sort_by(.id)' "$1"
}

canonical_index_status() {
  jq -S '.data | {enabled, dirty, board_dirty, generation, diagnostics}' "$1"
}

start_mock_ollama() {
  python3 -u - "$PORT_FILE" >"$MOCK_LOG" 2>&1 <<'PY' &
import json
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

port_file = sys.argv[1]

def vector_for(text, dimensions):
    lower = str(text).lower()
    if "terminal-surface" in lower or "terminal surface" in lower:
        base = [1.0, 0.0, 0.0, 0.0]
    elif any(token in lower for token in ["desktop", "visual", "layout", "css", "color", "palette", "dashboard"]):
        base = [0.0, 1.0, 0.0, 0.0]
    elif any(token in lower for token in ["api", "http", "server", "route"]):
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
  for _ in $(seq 1 50); do
    [[ -s "$PORT_FILE" ]] && return 0
    sleep 0.1
  done
  echo "error: mock Ollama server did not start" >&2
  exit 1
}

kb_source() {
  "$KANBAN_BIN" --db "$SOURCE_DB" --board "$BOARD" --actor "$ACTOR" --json "$@"
}

kb() {
  "$KANBAN_BIN" --db "$DB" --board "$BOARD" --actor "$ACTOR" --json "$@"
}

kb_imported() {
  "$KANBAN_BIN" --db "$IMPORTED_DB" --board "$BOARD" --actor "$ACTOR" --json "$@"
}

source_or_candidate_score_filter='
  def labels: ((.data.selected_labels // []) + (.data.candidates // []));
  labels | map(select(.label_name == "cli")) | first | .score // null
'
source_or_candidate_selected_filter='
  (.data.selected_labels // []) | any(.label_name == "cli")
'

start_mock_ollama
PORT="$(cat "$PORT_FILE")"

run_json "$RUN_DIR/source.init.json" kb_source init
run_json "$RUN_DIR/vector.configure.json" kb_source vector configure \
  --vector-config "$VECTOR_CONFIG" \
  --endpoint "http://127.0.0.1:$PORT" \
  --model "$MODEL" \
  --dimensions "$DIMENSIONS"

run_json "$RUN_DIR/source.label.cli.json" kb_source label create cli
run_json "$RUN_DIR/source.semantics.cli.json" kb_source label semantics upsert cli \
  --description "Command-line workflow label for existing command flags and help text." \
  --applies-when "Changes existing CLI flags, subcommands, or help text." \
  --positive-example "Update a documented command option." \
  --negative-example "Desktop visual layout only."

run_json "$RUN_DIR/source.label.ui.json" kb_source label create ui
run_json "$RUN_DIR/source.semantics.ui.json" kb_source label semantics upsert ui \
  --description "Desktop and web interface presentation work." \
  --applies-when "Changes dashboard layout, CSS spacing, visual color, or UI polish." \
  --positive-example "Polish desktop dashboard colors and spacing." \
  --negative-example "Command-line parser behavior only."

run_json "$RUN_DIR/source.label.ops.json" kb_source label create ops
run_json "$RUN_DIR/source.semantics.ops.json" kb_source label semantics upsert ops \
  --description "Operational maintenance label that is intentionally too broad for command behavior." \
  --applies-when "Reviews terminal-surface operator runbooks and local maintenance notes." \
  --positive-example "Document terminal-surface operator runbook steps." \
  --negative-example "Parser behavior for a shipped command."

run_json "$RUN_DIR/source.task.source.json" kb_source task create \
  "Add terminal-surface option output" \
  --description "Implement terminal-surface argument parsing and machine-readable help output for local operator commands."
SOURCE_TASK_REF="$(jq -r '.data.ref' "$RUN_DIR/source.task.source.json")"

run_json "$RUN_DIR/source.task.positive-control.json" kb_source task create \
  "Regression fixture for terminal-surface help output" \
  --description "Verify terminal-surface help output and machine-readable command behavior stays covered."
POSITIVE_CONTROL_REF="$(jq -r '.data.ref' "$RUN_DIR/source.task.positive-control.json")"

run_json "$RUN_DIR/source.task.negative-control.json" kb_source task create \
  "Polish desktop dashboard colors" \
  --description "Update visual layout, CSS spacing, and palette for the local operator console."
NEGATIVE_CONTROL_REF="$(jq -r '.data.ref' "$RUN_DIR/source.task.negative-control.json")"

cp "$SOURCE_DB" "$DB"

run_json "$RUN_DIR/preflight.atom-index.rebuild.json" kb label atom-index rebuild --vector-config "$VECTOR_CONFIG"
run_json "$RUN_DIR/preflight.semantics.cli.json" kb label semantics show cli
run_json "$RUN_DIR/preflight.atoms.json" kb label atoms list
run_json "$RUN_DIR/preflight.atom-index.status.json" kb label atom-index status --vector-config "$VECTOR_CONFIG"
run_json "$RUN_DIR/before.source.suggest.json" kb label suggest "$SOURCE_TASK_REF" \
  --vector-config "$VECTOR_CONFIG" --min-score 0.05 --candidate-limit 8 --atom-limit 16 --max-selected-labels 2
run_json "$RUN_DIR/before.positive-control.suggest.json" kb label suggest "$POSITIVE_CONTROL_REF" \
  --vector-config "$VECTOR_CONFIG" --min-score 0.05 --candidate-limit 8 --atom-limit 16 --max-selected-labels 2
run_json "$RUN_DIR/before.negative-control.suggest.json" kb label suggest "$NEGATIVE_CONTROL_REF" \
  --vector-config "$VECTOR_CONFIG" --min-score 0.05 --candidate-limit 8 --atom-limit 16 --max-selected-labels 2

assert_jq "$RUN_DIR/before.source.suggest.json" '.data.degraded == false' "before source suggest must not be degraded"
if jq -e "$source_or_candidate_selected_filter" "$RUN_DIR/before.source.suggest.json" >/dev/null; then
  echo "assertion failed: before source suggest unexpectedly selected cli" >&2
  exit 1
fi
BEFORE_CLI_SCORE="$(jq -r "$source_or_candidate_score_filter" "$RUN_DIR/before.source.suggest.json")"
if [[ "$BEFORE_CLI_SCORE" != "null" ]]; then
  awk -v score="$BEFORE_CLI_SCORE" 'BEGIN { exit !(score < 0.50) }' || {
    echo "assertion failed: before cli score must be below 0.50, got $BEFORE_CLI_SCORE" >&2
    exit 1
  }
fi

run_json "$RUN_DIR/before-record.task.json" kb task show "$SOURCE_TASK_REF"
run_json "$RUN_DIR/before-record.semantics.cli.json" kb label semantics show cli
run_json "$RUN_DIR/before-record.atoms.json" kb label atoms list
run_json "$RUN_DIR/before-record.atom-index.status.json" kb label atom-index status --vector-config "$VECTOR_CONFIG"
canonical_task_labels "$RUN_DIR/before-record.task.json" >"$RUN_DIR/before-record.task-labels.canon.json"
canonical_semantics "$RUN_DIR/before-record.semantics.cli.json" >"$RUN_DIR/before-record.semantics.canon.json"
canonical_atoms "$RUN_DIR/before-record.atoms.json" >"$RUN_DIR/before-record.atoms.canon.json"
canonical_index_status "$RUN_DIR/before-record.atom-index.status.json" >"$RUN_DIR/before-record.index.canon.json"

jq -n \
  --slurpfile suggest "$RUN_DIR/before.source.suggest.json" \
  --arg atom "$ATOM_TEXT" \
  --arg score "$BEFORE_CLI_SCORE" '
  def labels: (($suggest[0].data.selected_labels // []) + ($suggest[0].data.candidates // []));
  def cli_label: labels | map(select(.label_name == "cli")) | first;
  {
    actor: {name: "ontology-e2e-labeler", type: "agent", agent_type: "e2e-script"},
    agent_candidates_json: ([{label: "cli", confidence: 0.94, reason: "terminal-surface behavior is a CLI surface facet"}] | tojson),
    suggestion_snapshot_json: ($suggest[0].data | tojson),
    final_decision_json: ({accepted_labels: ["cli"], rationale: "source task is a false-negative for existing cli semantics"} | tojson),
    suggest_coverage: $suggest[0].data.coverage,
    suggest_coverage_cosine: $suggest[0].data.coverage_cosine,
    suggest_residual_norm: $suggest[0].data.residual_norm,
    suggest_needs_new_label: $suggest[0].data.needs_new_label,
    suggest_degraded: $suggest[0].data.degraded,
    diagnostics_json: ($suggest[0].data.diagnostics | tojson),
    capture_fingerprint: "ontology-e2e-positive-atom-source",
    signals: [{
      kind: "false_negative",
      target_label_ref: "cli",
      related_labels_json: "[]",
      proposed_action: "add_positive_atom",
      candidate_atom: {polarity: "positive", kind: "applies_when", text: $atom},
      proposed_label_name: null,
      proposal_json: "{}",
      agent_selected: true,
      suggest_state: (if (($suggest[0].data.selected_labels // []) | any(.label_name == "cli")) then "selected" elif (cli_label != null) then "candidate" else "absent" end),
      suggest_score: (cli_label.score // null),
      suggest_rank: null,
      final_selected: true,
      rationale: "Existing cli label should cover terminal-surface command behavior, but before suggest did not select cli.",
      confidence: 0.94,
      signal_key: "ontology-e2e-cli-terminal-surface-false-negative"
    }]
  }' >"$RUN_DIR/record.input.json"

run_json "$RUN_DIR/record.output.json" kb label ontology record "$SOURCE_TASK_REF" --input "$RUN_DIR/record.input.json"
SIGNAL_ID="$(jq -r '.data.signals[0].id' "$RUN_DIR/record.output.json")"
assert_jq "$RUN_DIR/record.output.json" '.data.signals[0].status == "open"' "record should create open signal"

run_json "$RUN_DIR/after-record.task.json" kb task show "$SOURCE_TASK_REF"
run_json "$RUN_DIR/after-record.semantics.cli.json" kb label semantics show cli
run_json "$RUN_DIR/after-record.atoms.json" kb label atoms list
run_json "$RUN_DIR/after-record.atom-index.status.json" kb label atom-index status --vector-config "$VECTOR_CONFIG"
canonical_task_labels "$RUN_DIR/after-record.task.json" >"$RUN_DIR/after-record.task-labels.canon.json"
canonical_semantics "$RUN_DIR/after-record.semantics.cli.json" >"$RUN_DIR/after-record.semantics.canon.json"
canonical_atoms "$RUN_DIR/after-record.atoms.json" >"$RUN_DIR/after-record.atoms.canon.json"
canonical_index_status "$RUN_DIR/after-record.atom-index.status.json" >"$RUN_DIR/after-record.index.canon.json"
cmp "$RUN_DIR/before-record.task-labels.canon.json" "$RUN_DIR/after-record.task-labels.canon.json"
cmp "$RUN_DIR/before-record.semantics.canon.json" "$RUN_DIR/after-record.semantics.canon.json"
cmp "$RUN_DIR/before-record.atoms.canon.json" "$RUN_DIR/after-record.atoms.canon.json"
cmp "$RUN_DIR/before-record.index.canon.json" "$RUN_DIR/after-record.index.canon.json"

run_json "$RUN_DIR/confirm.output.json" kb label ontology confirm "$SIGNAL_ID" \
  --reason "E2E reviewer confirmed this is a low-risk false-negative signal." \
  --actor-type agent --agent-type e2e-script
assert_jq "$RUN_DIR/confirm.output.json" '.data.action_type == "confirm" and .data.validation_status == "not_required"' "confirm should be lifecycle-only"
run_json "$RUN_DIR/after-confirm.show.json" kb label ontology show "$SIGNAL_ID"
assert_jq "$RUN_DIR/after-confirm.show.json" '.data.signal.status == "confirmed"' "signal should be confirmed"

run_json "$RUN_DIR/after-confirm.semantics.cli.json" kb label semantics show cli
run_json "$RUN_DIR/after-confirm.atoms.json" kb label atoms list
canonical_semantics "$RUN_DIR/after-confirm.semantics.cli.json" >"$RUN_DIR/after-confirm.semantics.canon.json"
canonical_atoms "$RUN_DIR/after-confirm.atoms.json" >"$RUN_DIR/after-confirm.atoms.canon.json"
cmp "$RUN_DIR/before-record.semantics.canon.json" "$RUN_DIR/after-confirm.semantics.canon.json"
cmp "$RUN_DIR/before-record.atoms.canon.json" "$RUN_DIR/after-confirm.atoms.canon.json"

BEFORE_ATOM_COUNT="$(jq '.data | length' "$RUN_DIR/after-confirm.atoms.json")"
run_json "$RUN_DIR/apply.output.json" kb label ontology apply atom "$SIGNAL_ID" \
  --label cli --kind applies-when --text "$ATOM_TEXT" \
  --reason "E2E adds a generalized terminal-surface applies_when atom." \
  --actor-type agent --agent-type e2e-script
APPLY_ACTION_ID="$(jq -r '.data.id' "$RUN_DIR/apply.output.json")"
RESULT_ATOM_ID="$(jq -r '.data.result_atom_id' "$RUN_DIR/apply.output.json")"
RESULT_ATOM_HASH="$(jq -r '.data.result_atom_content_hash' "$RUN_DIR/apply.output.json")"
assert_jq "$RUN_DIR/apply.output.json" '.data.action_type == "add_positive_atom" and .data.validation_status == "pending"' "apply should create pending positive atom action"
assert_jq "$RUN_DIR/apply.output.json" '.data.result_atom_id != null and .data.result_atom_content_hash != null' "apply should expose result atom id/hash"

run_json "$RUN_DIR/after-apply.show.json" kb label ontology show "$SIGNAL_ID"
assert_jq "$RUN_DIR/after-apply.show.json" '.data.signal.status == "confirmed"' "apply should leave signal confirmed until validation"
run_json "$RUN_DIR/after-apply.semantics.cli.json" kb label semantics show cli
run_json "$RUN_DIR/after-apply.atoms.json" kb label atoms list
run_json "$RUN_DIR/after-apply.atom-index.status.json" kb label atom-index status --vector-config "$VECTOR_CONFIG"
AFTER_ATOM_COUNT="$(jq '.data | length' "$RUN_DIR/after-apply.atoms.json")"
[[ "$AFTER_ATOM_COUNT" -eq $((BEFORE_ATOM_COUNT + 1)) ]] || {
  echo "assertion failed: expected one new atom; before=$BEFORE_ATOM_COUNT after=$AFTER_ATOM_COUNT" >&2
  exit 1
}
assert_jq_with_args "$RUN_DIR/after-apply.semantics.cli.json" --arg atom "$ATOM_TEXT" '.data.applies_when | index($atom)' "new applies_when atom should be present"
assert_jq "$RUN_DIR/after-apply.semantics.cli.json" '.data.positive_examples | index("Update a documented command option.")' "existing positive example should be preserved"
assert_jq "$RUN_DIR/after-apply.atom-index.status.json" '(.data.board_dirty == true) or (.data.dirty == true) or ((.data.diagnostics // []) | index("label_atom_index_dirty"))' "apply should mark label atom index dirty"

run_json "$RUN_DIR/after-apply.atom-index.rebuild.json" kb label atom-index rebuild --vector-config "$VECTOR_CONFIG"
run_json "$RUN_DIR/after-rebuild.atom-index.status.json" kb label atom-index status --vector-config "$VECTOR_CONFIG"
assert_jq "$RUN_DIR/after-rebuild.atom-index.status.json" '.data.enabled == true and (.data.board_dirty == false or .data.board_dirty == null) and (.data.dirty == false or .data.dirty == null)' "rebuild should leave clean index"

run_json "$RUN_DIR/after.source.suggest.json" kb label suggest "$SOURCE_TASK_REF" \
  --vector-config "$VECTOR_CONFIG" --min-score 0.05 --candidate-limit 8 --atom-limit 16 --max-selected-labels 2
run_json "$RUN_DIR/after.positive-control.suggest.json" kb label suggest "$POSITIVE_CONTROL_REF" \
  --vector-config "$VECTOR_CONFIG" --min-score 0.05 --candidate-limit 8 --atom-limit 16 --max-selected-labels 2
run_json "$RUN_DIR/after.negative-control.suggest.json" kb label suggest "$NEGATIVE_CONTROL_REF" \
  --vector-config "$VECTOR_CONFIG" --min-score 0.05 --candidate-limit 8 --atom-limit 16 --max-selected-labels 2

for file in "$RUN_DIR/after.source.suggest.json" "$RUN_DIR/after.positive-control.suggest.json" "$RUN_DIR/after.negative-control.suggest.json"; do
  assert_jq "$file" '.data.degraded == false' "after suggest must not be degraded: $file"
done
assert_jq_with_args "$RUN_DIR/after.source.suggest.json" --arg atom "$RESULT_ATOM_ID" '
  (((.data.selected_labels // []) + (.data.candidates // [])
    | map(select(.label_name == "cli")) | first) as $target
    | ($target.score >= 0.50)
      and (($target.evidence_atoms // []) | any(.atom_id == $atom)))
' "source task should have cli score >= 0.50 and evidence from new atom"
assert_jq "$RUN_DIR/after.positive-control.suggest.json" '
  (((.data.selected_labels // []) + (.data.candidates // [])
    | map(select(.label_name == "cli")) | first.score) // 0) >= 0.50
' "positive control should keep cli score >= 0.50"
assert_jq "$RUN_DIR/after.negative-control.suggest.json" '
  (((.data.selected_labels // []) | any(.label_name == "cli")) | not)
  and ((((.data.selected_labels // []) + (.data.candidates // [])
    | map(select(.label_name == "cli")) | first.score) // 0) < 0.50)
' "negative control should not newly match cli"

run_json "$RUN_DIR/validate.output.json" kb label ontology validate \
  --trusted --status passed \
  --reason "Trusted E2E collector found source task evidence for the new positive atom." \
  --vector-config "$VECTOR_CONFIG" \
  --min-score 0.05 --candidate-limit 8 --atom-limit 16 --max-selected-labels 2 \
  --actor-type agent --agent-type e2e-script \
  "$APPLY_ACTION_ID" "$SIGNAL_ID"
assert_jq "$RUN_DIR/validate.output.json" '.data.action_type == "validate" and .data.validation_status == "passed"' "trusted validation should pass"
assert_jq_with_args "$RUN_DIR/validate.output.json" --arg atom "$RESULT_ATOM_ID" '
  (.data.validation_json | fromjson) as $v
  | $v.manual.evidence_type == "trusted_automated"
    and $v.manual.collector.source == "label_ontology_validate_trusted"
    and $v.manual.embedding_model == "mock-ontology-e2e-v1"
    and $v.manual.index.dirty == false
    and ($v.manual.cases[0].after.evidence_atoms | any((.id // .atom_id) == $atom))
' "validation JSON should contain trusted collector evidence from the new atom"

run_json "$RUN_DIR/after-validate.show.json" kb label ontology show "$SIGNAL_ID"
assert_jq "$RUN_DIR/after-validate.show.json" '.data.signal.status == "resolved"' "passed validation should resolve signal"
assert_jq "$RUN_DIR/after-validate.show.json" '
  [.data.actions[].action_type] | index("confirm") and index("add_positive_atom") and index("validate")
' "signal history should include confirm, apply, validate"

run_json "$RUN_DIR/atom.explain.json" kb label atoms explain "$RESULT_ATOM_ID"
assert_jq_with_args "$RUN_DIR/atom.explain.json" --arg signal "$SIGNAL_ID" --arg action "$APPLY_ACTION_ID" '
  (.data.legacy_untracked == false)
  and (.data.provenance_actions | any(.action.id == $action))
  and (.data.supporting_signals | any(.signal.id == $signal))
  and ((.data.validation_history // []) | any(.action.validation_status == "passed"))
' "atom explain should trace action, signal, and validation history"

run_json "$RUN_DIR/doctor.original.json" kb doctor
assert_jq "$RUN_DIR/doctor.original.json" '.data.ok == true' "original disposable DB doctor should pass"
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
' "imported DB should preserve atom provenance"

jq -n \
  --arg run_dir "$RUN_DIR" \
  --arg source_task "$SOURCE_TASK_REF" \
  --arg positive_control "$POSITIVE_CONTROL_REF" \
  --arg negative_control "$NEGATIVE_CONTROL_REF" \
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
    negative_control: $negative_control,
    signal_id: $signal,
    apply_action_id: $action,
    result_atom_id: $atom,
    result_atom_content_hash: $atom_hash,
    evidence: [
      "record preserved task labels, semantics, atoms, and index status",
      "confirm created lifecycle action only",
      "apply added exactly one positive atom and marked index dirty",
      "rebuild produced clean label atom index",
      "after suggest used true evidence from the new atom and controls did not regress",
      "trusted validation resolved the signal",
      "atom explain traces action, signal, and validation history",
      "JSONL export/import preserved provenance and doctor passed"
    ]
  }' >"$SUMMARY"

if [[ "${KEEP_DISPOSABLE_DB:-0}" != "1" ]]; then
  rm -f "$SOURCE_DB" "$DB" "$IMPORTED_DB"
  rm -rf "$RUN_DIR/index"
fi

log "PASS ontology positive-atom E2E"
log "summary: $SUMMARY"
log "raw JSON artifacts: $RUN_DIR"
