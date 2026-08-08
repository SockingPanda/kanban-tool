#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/scripts/cargo-build-lock.sh"
TARGET_DIR="$("$LOCK" --print-target-dir)"
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RUN_DIR="${RUN_DIR:-$ROOT/.omx/ultragoal/evidence/G007-ontology-closure-e2e-$TIMESTAMP}"
BOARD="default"
ACTOR="ontology-closure-e2e"
DB="$RUN_DIR/closure.db"
IMPORTED_DB="$RUN_DIR/imported.db"
EXPORT_JSONL="$RUN_DIR/export.jsonl"
COMMAND_LOG="$RUN_DIR/commands.log"
SUMMARY="$RUN_DIR/summary.json"

command -v jq >/dev/null 2>&1 || {
  echo "error: jq is required" >&2
  exit 1
}
command -v node >/dev/null 2>&1 || {
  echo "error: node is required" >&2
  exit 1
}
command -v sqlite3 >/dev/null 2>&1 || {
  echo "error: sqlite3 is required" >&2
  exit 1
}
command -v pnpm >/dev/null 2>&1 || {
  echo "error: pnpm is required" >&2
  exit 1
}

mkdir -p "$RUN_DIR"

log() {
  printf '%s\n' "$*" | tee -a "$COMMAND_LOG" >&2
}

fail() {
  echo "error: $*" >&2
  exit 1
}

ensure_kanban_bin() {
  if [[ "${KANBAN_CLOSURE_USE_EXTERNAL_BIN:-0}" == "1" ]]; then
    [[ -n "${KANBAN_BIN:-}" ]] || fail "KANBAN_CLOSURE_USE_EXTERNAL_BIN=1 requires KANBAN_BIN"
    [[ -x "$KANBAN_BIN" || "$(command -v "$KANBAN_BIN" 2>/dev/null)" ]] || {
      fail "KANBAN_BIN is not executable or on PATH: $KANBAN_BIN"
    }
    log "+ using external KANBAN_BIN=$KANBAN_BIN"
    return 0
  fi
  log "+ $LOCK -- cargo build -q -p kanban-cli --bin kanban"
  "$LOCK" -- cargo build -q -p kanban-cli --bin kanban
  KANBAN_BIN="$TARGET_DIR/debug/kanban"
  [[ -x "$KANBAN_BIN" ]] || fail "expected debug kanban binary at $KANBAN_BIN"
}

kb() {
  "$KANBAN_BIN" --db "$DB" --board "$BOARD" --actor "$ACTOR" --json "$@"
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

sql_scalar() {
  local db="$1"
  local sql="$2"
  sqlite3 -readonly -batch -noheader "$db" "$sql"
}

write_counts() {
  local db="$1"
  local output="$2"
  sqlite3 -readonly -json "$db" <<'SQL' | jq -S '.[0]' >"$output"
SELECT
  (SELECT COUNT(*) FROM tasks) AS tasks,
  (SELECT COUNT(*) FROM labels) AS labels,
  (SELECT COUNT(*) FROM task_labels) AS task_labels,
  (SELECT COUNT(*) FROM task_events) AS task_events,
  (SELECT COUNT(*) FROM label_semantics) AS label_semantics,
  (SELECT COUNT(*) FROM label_atoms) AS label_atoms,
  (SELECT COUNT(*) FROM label_ontology_actions) AS actions,
  (SELECT COUNT(*) FROM label_ontology_action_atom_effects) AS effects,
  (SELECT COUNT(*) FROM label_ontology_actions WHERE action_type='update_semantics') AS update_semantics_actions,
  (SELECT COUNT(*) FROM label_ontology_actions WHERE action_type='revert_ontology_mutation') AS revert_actions,
  (SELECT COUNT(*) FROM label_ontology_actions WHERE action_type='confirm') AS confirm_actions;
SQL
}

assert_counts_equal() {
  local before="$1"
  local after="$2"
  local message="$3"
  if ! cmp -s "$before" "$after"; then
    echo "assertion failed: $message" >&2
    echo "before: $before" >&2
    cat "$before" >&2
    echo "after: $after" >&2
    cat "$after" >&2
    exit 1
  fi
}

latest_action_id() {
  local db="$1"
  local action_type="$2"
  sql_scalar "$db" "SELECT id FROM label_ontology_actions WHERE action_type='$action_type' ORDER BY created_at DESC, id DESC LIMIT 1"
}

latest_action_effect_count() {
  local db="$1"
  local action_id="$2"
  sql_scalar "$db" "SELECT COUNT(*) FROM label_ontology_action_atom_effects WHERE action_id='$action_id'"
}

foreign_key_check_count() {
  local db="$1"
  sqlite3 -readonly -json "$db" 'PRAGMA foreign_key_check;' | jq 'length'
}

assert_no_foreign_key_violations() {
  local db="$1"
  local count
  count="$(foreign_key_check_count "$db")"
  [[ "$count" == "0" ]] || fail "foreign_key_check returned $count violations for $db"
}

run_desktop_boundary_tests() {
  local output="$RUN_DIR/desktop-boundary-tests.log"
  log "+ pnpm --dir $ROOT --filter @kanban-tool/desktop test OntologyReviewWorkbench.test.tsx api.test.ts > $output"
  if ! pnpm --dir "$ROOT" --filter @kanban-tool/desktop test OntologyReviewWorkbench.test.tsx api.test.ts >"$output" 2>&1; then
    cat "$output" >&2 || true
    fail "Desktop lifecycle boundary tests failed"
  fi
}

ensure_kanban_bin

run_json "$RUN_DIR/init.json" kb init
write_counts "$DB" "$RUN_DIR/counts.after-init.json"

run_fail "$RUN_DIR/task-create-missing.out.json" "$RUN_DIR/task-create-missing.err" \
  kb task create "missing-label should not create task" \
    --description "This task attempts to bind a missing label." \
    --label missing-label
write_counts "$DB" "$RUN_DIR/counts.after-missing-label.json"
assert_counts_equal "$RUN_DIR/counts.after-init.json" "$RUN_DIR/counts.after-missing-label.json" \
  "task create with missing label must leave tasks/labels/bindings/events unchanged"
assert_file_contains "$RUN_DIR/task-create-missing.err" "does not exist" "missing label error should be explicit"

run_json "$RUN_DIR/label.docs.json" kb label create docs
write_counts "$DB" "$RUN_DIR/counts.after-label-create.json"
run_json "$RUN_DIR/task-create-existing-label.json" kb task create \
  "existing label bind" \
  --description "Task create may bind an already existing label." \
  --label docs
assert_jq "$RUN_DIR/task-create-existing-label.json" '.data.labels | map(.name) | index("docs")' \
  "task create should bind the existing docs label"
write_counts "$DB" "$RUN_DIR/counts.after-existing-label-task.json"
assert_jq "$RUN_DIR/counts.after-existing-label-task.json" '.tasks == 1 and .task_labels == 1 and .labels == 1' \
  "existing label task create should add one task and one binding"

run_json "$RUN_DIR/label.cli.json" kb label create cli
run_json "$RUN_DIR/semantics.seed.json" kb label semantics upsert cli \
  --description "Command-line behavior label." \
  --applies-when "Existing command help or JSON output changes." \
  --positive-example "Update command help for a flag." \
  --negative-example "Desktop visual polish only." \
  --reason "seed closure e2e cli semantics"
run_json "$RUN_DIR/semantics.before-patch.json" kb label semantics show cli
PATCH_HASH="$(jq -r '.data.semantics_hash' "$RUN_DIR/semantics.before-patch.json")"
PATCH_ACTIONS_BEFORE="$(sql_scalar "$DB" "SELECT COUNT(*) FROM label_ontology_actions")"
PATCH_EFFECTS_BEFORE="$(sql_scalar "$DB" "SELECT COUNT(*) FROM label_ontology_action_atom_effects")"
run_json "$RUN_DIR/semantics.patch.json" kb label semantics upsert cli \
  --expected-semantics-hash "$PATCH_HASH" \
  --applies-when "Closure E2E root-action patch atom." \
  --reason "add one closure e2e applies_when atom"
PATCH_ACTION_ID="$(latest_action_id "$DB" update_semantics)"
PATCH_ACTIONS_AFTER="$(sql_scalar "$DB" "SELECT COUNT(*) FROM label_ontology_actions")"
PATCH_EFFECTS_AFTER="$(sql_scalar "$DB" "SELECT COUNT(*) FROM label_ontology_action_atom_effects")"
[[ "$PATCH_ACTIONS_AFTER" -eq $((PATCH_ACTIONS_BEFORE + 1)) ]] || fail "semantics patch must write one root action"
[[ "$PATCH_EFFECTS_AFTER" -eq $((PATCH_EFFECTS_BEFORE + 1)) ]] || fail "semantics patch must write one added atom effect"
[[ "$(latest_action_effect_count "$DB" "$PATCH_ACTION_ID")" == "1" ]] || fail "patch action must own exactly one effect"

run_json "$RUN_DIR/semantics.before-clear.json" kb label semantics show cli
CLEAR_HASH="$(jq -r '.data.semantics_hash' "$RUN_DIR/semantics.before-clear.json")"
ATOMS_BEFORE_CLEAR="$(sql_scalar "$DB" "SELECT COUNT(*) FROM label_atoms WHERE label_id=(SELECT id FROM labels WHERE name='cli')")"
CLEAR_ACTIONS_BEFORE="$(sql_scalar "$DB" "SELECT COUNT(*) FROM label_ontology_actions")"
CLEAR_EFFECTS_BEFORE="$(sql_scalar "$DB" "SELECT COUNT(*) FROM label_ontology_action_atom_effects")"
run_json "$RUN_DIR/semantics.clear.json" kb label semantics delete cli \
  --expected-semantics-hash "$CLEAR_HASH" \
  --reason "closure e2e clear semantics with CAS"
CLEAR_ACTION_ID="$(latest_action_id "$DB" update_semantics)"
CLEAR_ACTIONS_AFTER="$(sql_scalar "$DB" "SELECT COUNT(*) FROM label_ontology_actions")"
CLEAR_EFFECTS_AFTER="$(sql_scalar "$DB" "SELECT COUNT(*) FROM label_ontology_action_atom_effects")"
[[ "$CLEAR_ACTIONS_AFTER" -eq $((CLEAR_ACTIONS_BEFORE + 1)) ]] || fail "semantics clear must write one root action"
[[ "$CLEAR_EFFECTS_AFTER" -eq $((CLEAR_EFFECTS_BEFORE + ATOMS_BEFORE_CLEAR)) ]] || fail "semantics clear must write removed effects for all atoms"
[[ "$(latest_action_effect_count "$DB" "$CLEAR_ACTION_ID")" == "$ATOMS_BEFORE_CLEAR" ]] || fail "clear action effect count must match removed atoms"
[[ "$(sql_scalar "$DB" "SELECT COUNT(*) FROM label_semantics WHERE label_id=(SELECT id FROM labels WHERE name='cli')")" == "0" ]] || fail "clear should delete label_semantics row"
[[ "$(sql_scalar "$DB" "SELECT COUNT(*) FROM label_atoms WHERE label_id=(SELECT id FROM labels WHERE name='cli')")" == "0" ]] || fail "clear should delete label_atoms rows"

REVERT_ACTIONS_BEFORE="$(sql_scalar "$DB" "SELECT COUNT(*) FROM label_ontology_actions")"
REVERT_EFFECTS_BEFORE="$(sql_scalar "$DB" "SELECT COUNT(*) FROM label_ontology_action_atom_effects")"
run_json "$RUN_DIR/semantics.revert.json" kb label ontology revert "$CLEAR_ACTION_ID" \
  --reason "closure e2e revert semantics clear"
REVERT_ACTION_ID="$(latest_action_id "$DB" revert_ontology_mutation)"
REVERT_ACTIONS_AFTER="$(sql_scalar "$DB" "SELECT COUNT(*) FROM label_ontology_actions")"
REVERT_EFFECTS_AFTER="$(sql_scalar "$DB" "SELECT COUNT(*) FROM label_ontology_action_atom_effects")"
[[ "$REVERT_ACTIONS_AFTER" -eq $((REVERT_ACTIONS_BEFORE + 1)) ]] || fail "revert must write one root action"
[[ "$REVERT_EFFECTS_AFTER" -eq $((REVERT_EFFECTS_BEFORE + ATOMS_BEFORE_CLEAR)) ]] || fail "revert must write added effects for restored atoms"
[[ "$(latest_action_effect_count "$DB" "$REVERT_ACTION_ID")" == "$ATOMS_BEFORE_CLEAR" ]] || fail "revert action effect count must match restored atoms"
run_json "$RUN_DIR/semantics.after-revert.json" kb label semantics show cli
assert_jq_with_args "$RUN_DIR/semantics.after-revert.json" --arg hash "$CLEAR_HASH" '.data.semantics_hash == $hash' \
  "revert should restore the pre-clear semantics hash"

GENERIC_ACTIONS_BEFORE="$(sql_scalar "$DB" "SELECT COUNT(*) FROM label_ontology_actions")"
run_fail "$RUN_DIR/generic-spoof.out.json" "$RUN_DIR/generic-spoof.err" \
  kb label ontology confirm los_fake --reason "attempt spoof" --canonical-before-hash sem_spoof
GENERIC_ACTIONS_AFTER="$(sql_scalar "$DB" "SELECT COUNT(*) FROM label_ontology_actions")"
[[ "$GENERIC_ACTIONS_AFTER" == "$GENERIC_ACTIONS_BEFORE" ]] || fail "generic lifecycle spoof must not write actions"
assert_file_contains "$RUN_DIR/generic-spoof.err" "unexpected argument" "generic spoof should be rejected by CLI"

run_json "$RUN_DIR/doctor.original.json" kb doctor
assert_jq "$RUN_DIR/doctor.original.json" '.data.ok == true' "closure DB doctor must pass"
assert_no_foreign_key_violations "$DB"
run_json "$RUN_DIR/export.output.json" kb export --out "$EXPORT_JSONL"
run_json "$RUN_DIR/import.output.json" kb_imported import --input "$EXPORT_JSONL" --replace
run_json "$RUN_DIR/doctor.imported.json" kb_imported doctor
assert_jq "$RUN_DIR/doctor.imported.json" '.data.ok == true' "imported closure DB doctor must pass"
assert_no_foreign_key_violations "$IMPORTED_DB"

for table in label_ontology_actions label_ontology_action_atom_effects label_ontology_action_signals; do
  source_count="$(sql_scalar "$DB" "SELECT COUNT(*) FROM $table")"
  imported_count="$(sql_scalar "$IMPORTED_DB" "SELECT COUNT(*) FROM $table")"
  [[ "$source_count" == "$imported_count" ]] || {
    fail "$table count mismatch after import: $source_count != $imported_count"
  }
done
missing_validation_requirement="$(sql_scalar "$IMPORTED_DB" "SELECT COUNT(*) FROM label_ontology_actions WHERE validation_requirement IS NULL OR validation_requirement=''")"
[[ "$missing_validation_requirement" == "0" ]] || {
  fail "imported actions must preserve validation_requirement"
}

run_desktop_boundary_tests

log "+ ontology-positive-atom-e2e"
RUN_DIR="$RUN_DIR/positive-atom" KANBAN_BIN="$KANBAN_BIN" bash "$ROOT/scripts/ontology-positive-atom-e2e.sh"
log "+ ontology-negative-atom-e2e"
RUN_DIR="$RUN_DIR/negative-atom" KANBAN_BIN="$KANBAN_BIN" bash "$ROOT/scripts/ontology-negative-atom-e2e.sh"
log "+ ontology-bootstrap-verify-e2e"
RUN_DIR="$RUN_DIR/bootstrap-verify" KANBAN_BIN="$KANBAN_BIN" bash "$ROOT/scripts/ontology-bootstrap-verify-e2e.sh"

jq -n \
  --arg run_dir "$RUN_DIR" \
  --arg kanban_bin "$KANBAN_BIN" \
  --arg patch_action "$PATCH_ACTION_ID" \
  --arg clear_action "$CLEAR_ACTION_ID" \
  --arg revert_action "$REVERT_ACTION_ID" \
  '{
    status: "passed",
    run_dir: $run_dir,
    kanban_bin: $kanban_bin,
    patch_action_id: $patch_action,
    clear_action_id: $clear_action,
    revert_action_id: $revert_action,
    evidence: [
      "task create with a missing label failed with no task/label/binding/event writes",
      "task create with an existing label bound exactly one label",
      "semantics patch, clear, and revert each wrote one root action and the expected atom effects",
      "generic lifecycle action could not accept canonical mutation spoof fields",
      "JSONL export/import preserved root actions, atom effects, and validation_requirement",
      "doctor and PRAGMA foreign_key_check passed for source and imported closure DBs",
      "Desktop boundary tests only expose lifecycle action controls",
      "positive atom trusted collector E2E passed",
      "negative atom trusted collector controls/regression E2E passed",
      "bootstrap staged verification E2E passed"
    ]
  }' >"$SUMMARY"

if [[ "${KEEP_DISPOSABLE_DB:-0}" != "1" ]]; then
  rm -f "$DB" "$IMPORTED_DB"
fi

log "PASS ontology closure E2E"
log "summary: $SUMMARY"
log "raw JSON artifacts: $RUN_DIR"
