use crate::connect_file;

use super::{
    LabelOntologyActionInput, LabelOntologyActionRecord, LabelOntologyActionType,
    LabelOntologyActor, LabelOntologyAtomApplyInput, LabelOntologyCandidateAtomInput,
    LabelOntologyObservationRecord, LabelOntologyProposedAction, LabelOntologyRecordInput,
    LabelOntologyRetargetOptions, LabelOntologyRevertInput, LabelOntologyReviewAtomVariant,
    LabelOntologyReviewGroup, LabelOntologyReviewGroupBy, LabelOntologyReviewLabelRef,
    LabelOntologyReviewOptions, LabelOntologySignalDetail, LabelOntologySignalInput,
    LabelOntologySignalKind, LabelOntologySignalListOptions, LabelOntologySignalRecord,
    LabelOntologySignalStatus, LabelOntologyStructurePlanInput, LabelOntologySuggestState,
    LabelOntologyTrustedValidationInput, LabelOntologyValidationInput,
    LabelOntologyValidationStatus, LabelSemanticProposalRecord, LabelSemanticsMutationOptions,
    LabelSuggestionEvidenceAtom, LabelSuggestionOptions, LabelSuggestionResult,
    TaskOntologySignalSummary, TaskOntologySummary, TaskRecord, all_values, board_id,
    derived_status_by_name, exec, get_task_by_id, get_task_by_id_global_conn,
    label_atom_index_status_with, mark_label_atom_store_dirty, optional, required_row,
    resolve_task, storage, suggest_task_labels_with, upsert_label_semantics_in_tx,
    with_immediate_tx, with_read_tx,
};

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    str::FromStr,
};

use kanban_core::{Clock, KanbanError, Result, SystemClock, new_typed_id};
use kanban_indexer::LANCEDB_LABEL_ATOMS_STORE;
use kanban_labels::LabelDefinition;
use kanban_vector::{LabelAtomVectorStore, VectorStoreStatus};
use rusqlite::{Connection, OptionalExtension, Row, params, types::Value};
use serde_json::{Value as JsonValue, json};

const LABEL_ONTOLOGY_LIST_LIMIT_MAX: usize = 1000;
const LABEL_ONTOLOGY_VALIDATION_SCORE_THRESHOLD: f64 = 0.50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LabelOntologyValidationEvidenceSource {
    ExternalAttestation,
    TrustedCollector,
}

pub fn record_label_ontology_observation(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: &str,
    input: LabelOntologyRecordInput,
) -> Result<LabelOntologyObservationRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let task = resolve_task(&conn, &board_id, task_ref)?;
        if task.board_id != board_id {
            return Err(KanbanError::InvalidInput(
                "task ref resolves outside the requested board".into(),
            ));
        }
        if input.signals.is_empty() {
            return Err(KanbanError::InvalidInput(
                "at least one ontology signal is required".into(),
            ));
        }
        let input = derive_record_metrics_from_snapshot(input)?;
        ensure_observation_metric_contract(&input)?;
        ensure_raw_signal_metric_contract(&input.signals)?;

        let actor = normalize_actor(input.actor)?;
        let agent_candidates_json = normalize_json_field(
            &input.agent_candidates_json,
            "agent_candidates_json",
            JsonShape::Array,
        )?;
        let suggestion_snapshot_json = normalize_json_field(
            &input.suggestion_snapshot_json,
            "suggestion_snapshot_json",
            JsonShape::Object,
        )?;
        let final_decision_json = normalize_json_field(
            &input.final_decision_json,
            "final_decision_json",
            JsonShape::Object,
        )?;
        let diagnostics_json = normalize_json_field(
            &input.diagnostics_json,
            "diagnostics_json",
            JsonShape::Array,
        )?;
        let task_snapshot_json = task_snapshot_json(&task)?;
        let suggest_input_hash = suggest_input_hash_for_task(&task);
        let signal_fingerprint_json = serde_json::to_string(&input.signals)
            .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;
        let capture_fingerprint = input
            .capture_fingerprint
            .as_deref()
            .map(normalize_required_text)
            .transpose()?
            .unwrap_or_else(|| {
                stable_hash(&format!(
                    "{}\n{}\n{}\n{}\n{}",
                    task_snapshot_json,
                    agent_candidates_json,
                    suggestion_snapshot_json,
                    final_decision_json,
                    signal_fingerprint_json
                ))
            });

        let observation_id = new_typed_id("lor");
        exec(
            &conn,
            "INSERT INTO label_ontology_observations(\
             id, board_id, task_id, task_ref_snapshot, task_snapshot_json, suggest_input_hash, \
             agent_candidates_json, suggestion_snapshot_json, final_decision_json, suggest_coverage, suggest_coverage_cosine, \
             suggest_residual_norm, suggest_needs_new_label, suggest_degraded, diagnostics_json, \
             capture_fingerprint, created_by, created_by_type, agent_type, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                observation_id,
                board_id,
                task.id,
                task.task_ref,
                task_snapshot_json,
                suggest_input_hash,
                agent_candidates_json,
                suggestion_snapshot_json,
                final_decision_json,
                input.suggest_coverage,
                input.suggest_coverage_cosine,
                input.suggest_residual_norm,
                bool_int(input.suggest_needs_new_label),
                bool_int(input.suggest_degraded),
                diagnostics_json,
                capture_fingerprint,
                actor.name,
                actor.actor_type,
                actor.agent_type,
                now,
            ],
        )?;

        let signals = input
            .signals
            .into_iter()
            .map(|signal| insert_signal(&conn, &board_id, &observation_id, signal, now))
            .collect::<Result<Vec<_>>>()?;
        let mut observation = observation_by_id(&conn, &observation_id)?;
        observation.signals = signals;
        Ok(observation)
    })
}

pub fn list_label_ontology_signals(
    path: impl AsRef<Path>,
    board: &str,
    options: LabelOntologySignalListOptions,
) -> Result<Vec<LabelOntologySignalRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    with_read_tx(&conn, || {
        let mut conditions = vec!["s.board_id=?".to_owned()];
        let mut sql_params = vec![Value::Text(board_id.clone())];

        let statuses = if options.statuses.is_empty() && !options.include_all {
            vec![
                LabelOntologySignalStatus::Open,
                LabelOntologySignalStatus::Confirmed,
            ]
        } else {
            options.statuses.clone()
        };
        add_in_filter(
            &mut conditions,
            &mut sql_params,
            "s.status",
            statuses.iter().map(ToString::to_string),
        );
        add_in_filter(
            &mut conditions,
            &mut sql_params,
            "s.kind",
            options.kinds.iter().map(ToString::to_string),
        );

        if let Some(task_ref) = options.task_ref.as_deref() {
            let task = resolve_task(&conn, &board_id, task_ref)?;
            conditions.push("o.task_id=?".to_owned());
            sql_params.push(Value::Text(task.id));
        }
        if let Some(label_ref) = options.target_label_ref.as_deref() {
            let label = resolve_label(&conn, &board_id, label_ref)?;
            conditions.push("s.target_label_id=?".to_owned());
            sql_params.push(Value::Text(label.id));
        }
        if let Some(proposed_label_name) = options.proposed_label_name.as_deref() {
            conditions.push("s.proposed_label_name_normalized=?".to_owned());
            sql_params.push(Value::Text(normalize_label_name(proposed_label_name)?));
        }

        let limit = options.limit.clamp(1, LABEL_ONTOLOGY_LIST_LIMIT_MAX);
        sql_params.push(Value::Integer(limit as i64));
        let where_sql = conditions.join(" AND ");
        let sql = format!(
            "SELECT {} FROM label_ontology_signals s \
             JOIN label_ontology_observations o ON o.id=s.observation_id \
             WHERE {where_sql} ORDER BY s.created_at ASC, s.id ASC LIMIT ?",
            SIGNAL_COLUMNS
        );
        all_values(&conn, &sql, &sql_params, signal_from_row)
    })
}

pub fn review_label_ontology(
    path: impl AsRef<Path>,
    board: &str,
    options: LabelOntologyReviewOptions,
) -> Result<Vec<LabelOntologyReviewGroup>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    with_read_tx(&conn, || {
        let mut conditions = vec!["s.board_id=?".to_owned()];
        let mut sql_params = vec![Value::Text(board_id)];
        if !options.include_all {
            add_in_filter(
                &mut conditions,
                &mut sql_params,
                "s.status",
                [
                    LabelOntologySignalStatus::Open,
                    LabelOntologySignalStatus::Confirmed,
                ]
                .into_iter()
                .map(|status| status.to_string()),
            );
        }

        let where_sql = conditions.join(" AND ");
        let sql = format!(
            "SELECT s.id, o.task_id, o.task_ref_snapshot, o.suggest_degraded, s.status, \
             s.kind, s.proposed_action, s.target_label_id, s.target_label_name_snapshot, \
             s.candidate_atom_polarity, s.candidate_atom_kind, s.candidate_text, \
             s.candidate_content_hash, s.proposed_label_name, s.proposed_label_name_normalized, \
             s.suggest_score, s.created_at \
             FROM label_ontology_signals s \
             JOIN label_ontology_observations o ON o.id=s.observation_id \
             WHERE {where_sql} ORDER BY s.created_at ASC, s.id ASC"
        );
        let mut stmt = conn.prepare(&sql).map_err(storage)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(sql_params.iter()), |row| {
                review_signal_row_from_row(row)
            })
            .map_err(storage)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(storage)?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let signal_ids = rows
            .iter()
            .map(|row| row.signal_id.clone())
            .collect::<Vec<_>>();
        let action_links = review_action_links_for_signals(&conn, &signal_ids)?;
        let mut groups = BTreeMap::<String, ReviewGroupAccumulator>::new();
        for row in rows {
            let links = action_links
                .get(&row.signal_id)
                .cloned()
                .unwrap_or_default();
            let key = review_group_key(options.group_by, &row);
            groups
                .entry(key.clone())
                .or_insert_with(|| ReviewGroupAccumulator::new(options.group_by, key))
                .add(row, &links);
        }

        let limit = options.limit.clamp(1, LABEL_ONTOLOGY_LIST_LIMIT_MAX);
        let mut groups = groups
            .into_values()
            .map(ReviewGroupAccumulator::finish)
            .collect::<Vec<_>>();
        groups.sort_by(|left, right| {
            right
                .task_count
                .cmp(&left.task_count)
                .then_with(|| right.confirmed_count.cmp(&left.confirmed_count))
                .then_with(|| right.latest_signal_at.cmp(&left.latest_signal_at))
                .then_with(|| left.key.cmp(&right.key))
        });
        groups.truncate(limit);
        Ok(groups)
    })
}

pub fn task_ontology_summary(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: &str,
) -> Result<Option<TaskOntologySummary>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    task_ontology_summary_for_task(&conn, &task)
}

pub fn task_ontology_summary_by_id_global(
    path: impl AsRef<Path>,
    task_id: &str,
) -> Result<Option<TaskOntologySummary>> {
    let conn = connect_file(path.as_ref())?;
    let task = get_task_by_id_global_conn(&conn, task_id)?;
    task_ontology_summary_for_task(&conn, &task)
}

pub fn get_label_ontology_signal(
    path: impl AsRef<Path>,
    signal_id: &str,
) -> Result<LabelOntologySignalDetail> {
    let conn = connect_file(path.as_ref())?;
    with_read_tx(&conn, || {
        let signal = signal_by_id(&conn, signal_id)?;
        let mut observation = observation_by_id(&conn, &signal.observation_id)?;
        observation.signals = signals_for_observation(&conn, &observation.id)?;
        let actions = actions_for_signal(&conn, &signal.id)?;
        Ok(LabelOntologySignalDetail {
            signal,
            observation,
            actions,
        })
    })
}

pub fn create_label_ontology_action(
    path: impl AsRef<Path>,
    board: &str,
    input: LabelOntologyActionInput,
) -> Result<LabelOntologyActionRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        ensure_generic_lifecycle_action_input(&input)?;
        let board_id = board_id(&conn, board)?;
        let actor = normalize_actor(input.actor)?;
        let reason = normalize_required_text(&input.reason)?;
        let signal_ids = normalize_signal_ids(input.signal_ids)?;
        let signals = signal_ids
            .iter()
            .map(|signal_id| signal_by_id(&conn, signal_id))
            .collect::<Result<Vec<_>>>()?;
        for signal in &signals {
            if signal.board_id != board_id {
                return Err(KanbanError::InvalidInput(format!(
                    "signal {} belongs to a different board",
                    signal.id
                )));
            }
        }

        if let Some(parent_action_id) = input.parent_action_id.as_deref() {
            ensure_action_on_board(&conn, &board_id, parent_action_id)?;
        }
        let superseded_by_signal_id = input
            .superseded_by_signal_id
            .as_deref()
            .map(normalize_required_text)
            .transpose()?;
        if matches!(input.action_type, LabelOntologyActionType::Supersede)
            && superseded_by_signal_id.is_none()
        {
            return Err(KanbanError::InvalidInput(
                "supersede action requires superseded_by_signal_id".into(),
            ));
        }
        if let Some(replacement_id) = superseded_by_signal_id.as_deref() {
            let replacement = signal_by_id(&conn, replacement_id)?;
            if replacement.board_id != board_id {
                return Err(KanbanError::InvalidInput(
                    "replacement signal belongs to a different board".into(),
                ));
            }
            if signal_ids
                .iter()
                .any(|signal_id| signal_id == replacement_id)
            {
                return Err(KanbanError::InvalidInput(
                    "signal cannot supersede itself".into(),
                ));
            }
            if matches!(input.action_type, LabelOntologyActionType::Supersede) {
                ensure_no_supersede_cycle(&conn, &board_id, &signal_ids, replacement_id)?;
            }
        }

        let target_label_id = input
            .target_label_ref
            .as_deref()
            .map(|label_ref| resolve_label(&conn, &board_id, label_ref).map(|label| label.id))
            .transpose()?;
        let result_label_id = input
            .result_label_ref
            .as_deref()
            .map(|label_ref| resolve_label(&conn, &board_id, label_ref).map(|label| label.id))
            .transpose()?;
        if let Some(proposal_id) = input.result_proposal_id.as_deref() {
            ensure_proposal_on_board(&conn, &board_id, proposal_id)?;
        }
        let change_json = input
            .change_json
            .as_deref()
            .map(|json| normalize_json_field(json, "change_json", JsonShape::Object))
            .transpose()?
            .unwrap_or_else(|| "{}".to_owned());
        let validation_json = input
            .validation_json
            .as_deref()
            .map(|json| normalize_json_field(json, "validation_json", JsonShape::Object))
            .transpose()?
            .unwrap_or_else(|| "{}".to_owned());
        let validation_status = input
            .validation_status
            .unwrap_or(LabelOntologyValidationStatus::NotRequired);

        validate_status_transition(input.action_type, &signals)?;

        let action_id = new_typed_id("loa");
        exec(
            &conn,
            "INSERT INTO label_ontology_actions(\
             id, board_id, parent_action_id, action_type, reason, target_label_id, result_label_id, \
             result_atom_id, result_atom_content_hash, result_proposal_id, canonical_before_hash, \
             canonical_after_hash, change_json, validation_status, validation_json, created_by, \
             created_by_type, agent_type, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            params![
                action_id,
                board_id,
                input.parent_action_id,
                input.action_type.to_string(),
                reason,
                target_label_id,
                result_label_id,
                normalize_optional_text(input.result_atom_id)?,
                normalize_optional_text(input.result_atom_content_hash)?,
                input.result_proposal_id,
                normalize_optional_text(input.canonical_before_hash)?,
                normalize_optional_text(input.canonical_after_hash)?,
                change_json,
                validation_status.to_string(),
                validation_json,
                actor.name,
                actor.actor_type,
                actor.agent_type,
                now,
            ],
        )?;
        for signal_id in &signal_ids {
            exec(
                &conn,
                "INSERT INTO label_ontology_action_signals(board_id, action_id, signal_id, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![board_id, action_id, signal_id, now],
            )?;
        }
        apply_status_transition(
            &conn,
            input.action_type,
            &signal_ids,
            superseded_by_signal_id.as_deref(),
            &reason,
            now,
        )?;
        action_by_id_with_links(&conn, &action_id)
    })
}

pub fn plan_label_ontology_structure_change(
    path: impl AsRef<Path>,
    board: &str,
    input: LabelOntologyStructurePlanInput,
) -> Result<LabelOntologyActionRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        ensure_structure_plan_action_type(input.action_type)?;
        let board_id = board_id(&conn, board)?;
        let actor = normalize_actor(input.actor)?;
        let reason = normalize_required_text(&input.reason)?;
        let signal_ids = normalize_signal_ids(input.signal_ids)?;
        let signals = signal_ids
            .iter()
            .map(|signal_id| signal_by_id(&conn, signal_id))
            .collect::<Result<Vec<_>>>()?;
        ensure_signals_on_board_and_status(
            &signals,
            &board_id,
            &[LabelOntologySignalStatus::Confirmed],
        )?;

        let target_label = resolve_label(&conn, &board_id, &input.target_label_ref)?;
        let proposed_label_name = input
            .proposed_label_name
            .as_deref()
            .map(normalize_required_text)
            .transpose()?;
        let proposed_label_name_normalized = proposed_label_name
            .as_deref()
            .map(normalize_label_name)
            .transpose()?;
        let related_labels =
            resolve_structure_related_labels(&conn, &board_id, input.related_label_refs)?;
        ensure_structure_plan_contract(
            input.action_type,
            &target_label,
            proposed_label_name_normalized.as_deref(),
            &related_labels,
            &signals,
        )?;

        let target_snapshot = structure_label_snapshot_json(&conn, &board_id, &target_label)?;
        let related_snapshots = related_labels
            .iter()
            .map(|label| structure_label_snapshot_json(&conn, &board_id, label))
            .collect::<Result<Vec<_>>>()?;
        let before = json!({
            "labels": std::iter::once(target_snapshot.clone())
                .chain(related_snapshots.iter().cloned())
                .collect::<Vec<_>>(),
        });
        let task_binding_policy = input
            .task_binding_policy
            .as_deref()
            .map(normalize_required_text)
            .transpose()?
            .unwrap_or_else(|| default_structure_task_binding_policy(input.action_type).to_owned());
        let validation_policy = input
            .validation_policy_json
            .as_deref()
            .map(|json| parse_json_field(json, "validation_policy_json", JsonShape::Object))
            .transpose()?
            .unwrap_or_else(default_structure_validation_policy_json);
        let after = json!({
            "change_type": input.action_type.to_string(),
            "target_label": {
                "id": &target_label.id,
                "name": &target_label.name,
            },
            "proposed_label_name": &proposed_label_name,
            "proposed_label_name_normalized": &proposed_label_name_normalized,
            "related_labels": related_labels.iter().map(|label| {
                json!({
                    "id": &label.id,
                    "name": &label.name,
                })
            }).collect::<Vec<_>>(),
            "task_binding_migration_plan": {
                "policy": &task_binding_policy,
                "status": "planned",
            },
            "validation_policy": &validation_policy,
        });
        let before_hash = stable_hash(
            &serde_json::to_string(&before)
                .map_err(|err| KanbanError::InvalidInput(err.to_string()))?,
        );
        let after_hash = stable_hash(
            &serde_json::to_string(&after)
                .map_err(|err| KanbanError::InvalidInput(err.to_string()))?,
        );
        let change_json = serde_json::to_string(&json!({
            "phase": "planned_structure_change",
            "canonical_mutation_applied": false,
            "change_type": input.action_type.to_string(),
            "before": &before,
            "after": &after,
            "target_label": &target_snapshot,
            "related_labels": &related_snapshots,
            "source_signals": signals.iter().map(structure_source_signal_json).collect::<Vec<_>>(),
            "task_binding_migration_plan": {
                "policy": &task_binding_policy,
                "status": "planned",
            },
            "validation_policy": &validation_policy,
            "after_hash_kind": "planned_change_hash",
        }))
        .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;
        let validation_json = serde_json::to_string(&json!({
            "state": "pending_structure_change_plan",
            "trusted_validation_required_before_apply": true,
        }))
        .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;

        let action_id = insert_ontology_action(
            &conn,
            &board_id,
            InsertOntologyAction {
                action_type: input.action_type,
                reason,
                actor,
                parent_action_id: None,
                target_label_id: Some(target_label.id),
                result_label_id: None,
                result_atom_id: None,
                result_atom_content_hash: None,
                result_proposal_id: None,
                canonical_before_hash: Some(before_hash),
                canonical_after_hash: Some(after_hash),
                change_json,
                validation_status: LabelOntologyValidationStatus::Pending,
                validation_json,
            },
            now,
        )?;
        link_action_signals(&conn, &board_id, &action_id, &signal_ids, now)?;
        action_by_id_with_links(&conn, &action_id)
    })
}

pub fn apply_label_ontology_atom(
    path: impl AsRef<Path>,
    board: &str,
    input: LabelOntologyAtomApplyInput,
) -> Result<LabelOntologyActionRecord> {
    apply_label_ontology_atom_with_options(
        path,
        board,
        input,
        LabelOntologyRetargetOptions::default(),
    )
}

pub fn apply_label_ontology_atom_with_options(
    path: impl AsRef<Path>,
    board: &str,
    input: LabelOntologyAtomApplyInput,
    options: LabelOntologyRetargetOptions,
) -> Result<LabelOntologyActionRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let actor = normalize_actor(input.actor)?;
        let reason = normalize_required_text(&input.reason)?;
        let signal_ids = normalize_signal_ids(input.signal_ids)?;
        let signals = signal_ids
            .iter()
            .map(|signal_id| signal_by_id(&conn, signal_id))
            .collect::<Result<Vec<_>>>()?;
        ensure_signals_on_board_and_status(
            &signals,
            &board_id,
            &[LabelOntologySignalStatus::Confirmed],
        )?;
        let label = resolve_label(&conn, &board_id, &input.label_ref)?;
        let retarget_reason = normalize_retarget_reason(
            options.allow_retarget,
            options.retarget_reason,
            "apply atom",
        )?;
        let retarget_override =
            atom_apply_retarget_override(&signals, &label, retarget_reason.as_deref())?;
        let atom = normalize_candidate_atom(&LabelOntologyCandidateAtomInput {
            polarity: polarity_for_atom_kind(&input.kind)?.to_owned(),
            kind: input.kind,
            text: input.text,
        })?;
        let canonical_add_action_type = match atom.polarity.as_str() {
            "positive" => LabelOntologyActionType::AddPositiveAtom,
            "negative" => LabelOntologyActionType::AddNegativeAtom,
            _ => {
                return Err(KanbanError::InvalidInput(
                    "candidate atom polarity must be positive or negative".into(),
                ));
            }
        };

        let before = load_semantics_parts(&conn, &board_id, &label.id)?;
        let before_hash = semantics_hash(&label, &before)?;
        let mut after = before.clone();
        after.push_atom(&atom.kind, &atom.text);
        let after_hash = semantics_hash(&label, &after)?;
        let canonical_changed = after_hash != before_hash;
        let action_type = if canonical_changed {
            canonical_add_action_type
        } else {
            LabelOntologyActionType::AdoptExistingAtom
        };
        if canonical_changed {
            let definition = LabelDefinition {
                id: label.id.clone(),
                name: label.name.clone(),
                description: after.description.clone(),
                applies_when: after.applies_when.clone(),
                positive_examples: after.positive_examples.clone(),
                excludes_when: after.excludes_when.clone(),
                negative_examples: after.negative_examples.clone(),
            };
            upsert_label_semantics_in_tx(&conn, &board_id, &definition, now)?;
            mark_label_atom_store_dirty(&conn, &board_id, now)?;
        }

        let result_atom_content_hash = stable_hash(&format!(
            "{}\n{}\n{}\n{}",
            label.id,
            atom.polarity,
            atom.kind,
            normalize_atom_text(&atom.text)
        ));
        let result_atom_id = required_row(
            &conn,
            "SELECT id FROM label_atoms WHERE board_id=?1 AND label_id=?2 AND content_hash=?3",
            params![board_id, label.id, result_atom_content_hash],
            |row| row.get::<_, String>(0),
            || KanbanError::NotFound(format!("label atom {result_atom_content_hash}")),
        )?;
        let change_json = serde_json::to_string(&json!({
            "label": {"id": &label.id, "name": &label.name},
            "added_atom": {
                "polarity": &atom.polarity,
                "kind": &atom.kind,
                "text": &atom.text,
                "content_hash": &result_atom_content_hash,
                "id": &result_atom_id,
            },
            "changed": canonical_changed,
            "canonical_changed": canonical_changed,
            "provenance_only": !canonical_changed,
            "requested_action_type": canonical_add_action_type.to_string(),
            "before": semantics_json(&label, &before),
            "after": semantics_json(&label, &after),
            "retarget_override": retarget_override,
        }))
        .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;

        let action_id = insert_ontology_action(
            &conn,
            &board_id,
            InsertOntologyAction {
                action_type,
                reason,
                actor,
                parent_action_id: None,
                target_label_id: Some(label.id.clone()),
                result_label_id: None,
                result_atom_id: Some(result_atom_id),
                result_atom_content_hash: Some(result_atom_content_hash),
                result_proposal_id: None,
                canonical_before_hash: Some(before_hash),
                canonical_after_hash: Some(after_hash),
                change_json,
                validation_status: if canonical_changed {
                    LabelOntologyValidationStatus::Pending
                } else {
                    LabelOntologyValidationStatus::NotRequired
                },
                validation_json: "{}".to_owned(),
            },
            now,
        )?;
        link_action_signals(&conn, &board_id, &action_id, &signal_ids, now)?;
        action_by_id_with_links(&conn, &action_id)
    })
}

pub fn revert_label_ontology_mutation(
    path: impl AsRef<Path>,
    board: &str,
    input: LabelOntologyRevertInput,
) -> Result<LabelOntologyActionRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let actor = normalize_actor(input.actor)?;
        let reason = normalize_required_text(&input.reason)?;
        let target_action_id = normalize_required_text(&input.target_action_id)?;
        let expected_current_hash = input
            .expected_current_hash
            .as_deref()
            .map(normalize_required_text)
            .transpose()?;
        ensure_action_on_board(&conn, &board_id, &target_action_id)?;
        let target_action = action_by_id_with_links(&conn, &target_action_id)?;
        ensure_revertable_action_type(target_action.action_type)?;
        let target_label_id = target_action.target_label_id.as_deref().ok_or_else(|| {
            KanbanError::InvalidInput(format!(
                "ontology action {} has no target label to revert",
                target_action.id
            ))
        })?;
        let target_label = resolve_label(&conn, &board_id, target_label_id)?;
        let current_parts = load_semantics_parts(&conn, &board_id, &target_label.id)?;
        let current_hash = semantics_hash(&target_label, &current_parts)?;
        if let Some(expected) = expected_current_hash.as_deref()
            && current_hash != expected
        {
            return Err(KanbanError::InvalidInput(
                "expected_current_hash does not match current canonical ontology state".into(),
            ));
        }
        let target_after_hash = target_action
            .canonical_after_hash
            .as_deref()
            .ok_or_else(|| {
                KanbanError::InvalidInput(format!(
                    "ontology action {} has no canonical_after_hash to revert from",
                    target_action.id
                ))
            })?;
        if current_hash != target_after_hash {
            return Err(KanbanError::InvalidInput(
                "cannot revert ontology action because canonical ontology state changed after the target action".into(),
            ));
        }
        let target_before_hash =
            target_action
                .canonical_before_hash
                .as_deref()
                .ok_or_else(|| {
                    KanbanError::InvalidInput(format!(
                        "ontology action {} has no canonical_before_hash to restore",
                        target_action.id
                    ))
                })?;
        let change = parse_action_change_json(&target_action)?;
        let before_value = change.get("before").ok_or_else(|| {
            KanbanError::InvalidInput(format!(
                "ontology action {} change_json has no before snapshot",
                target_action.id
            ))
        })?;
        let restore_parts = semantics_parts_from_json(
            before_value,
            &format!("action {} before", target_action.id),
        )?;
        let restore_hash = semantics_hash(&target_label, &restore_parts)?;
        if restore_hash != target_before_hash {
            return Err(KanbanError::InvalidInput(format!(
                "target action {} before snapshot hash does not match canonical_before_hash",
                target_action.id
            )));
        }
        let definition = LabelDefinition {
            id: target_label.id.clone(),
            name: target_label.name.clone(),
            description: restore_parts.description.clone(),
            applies_when: restore_parts.applies_when.clone(),
            positive_examples: restore_parts.positive_examples.clone(),
            excludes_when: restore_parts.excludes_when.clone(),
            negative_examples: restore_parts.negative_examples.clone(),
        };
        upsert_label_semantics_in_tx(&conn, &board_id, &definition, now)?;
        mark_label_atom_store_dirty(&conn, &board_id, now)?;
        let restored = load_semantics_parts(&conn, &board_id, &target_label.id)?;
        let restored_hash = semantics_hash(&target_label, &restored)?;
        if restored_hash != target_before_hash {
            return Err(KanbanError::InvalidInput(format!(
                "reverted canonical hash {} does not match target before hash {}",
                restored_hash, target_before_hash
            )));
        }
        let change_json = serde_json::to_string(&json!({
            "reverted_action_id": &target_action.id,
            "reverted_action_type": target_action.action_type.to_string(),
            "label": {
                "id": &target_label.id,
                "name": &target_label.name,
            },
            "expected_current_hash": expected_current_hash,
            "reverted_canonical_before_hash": &target_action.canonical_before_hash,
            "reverted_canonical_after_hash": &target_action.canonical_after_hash,
            "before_revert": semantics_json(&target_label, &current_parts),
            "after_revert": semantics_json(&target_label, &restored),
            "index_dirty": true,
        }))
        .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;
        let validation_json = serde_json::to_string(&json!({
            "state": "pending_revert_validation",
            "reverted_action_id": &target_action.id,
            "reverted_action_type": target_action.action_type.to_string(),
        }))
        .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;
        let action_id = insert_ontology_action(
            &conn,
            &board_id,
            InsertOntologyAction {
                action_type: LabelOntologyActionType::RevertOntologyMutation,
                reason,
                actor,
                parent_action_id: Some(target_action.id.clone()),
                target_label_id: Some(target_label.id),
                result_label_id: None,
                result_atom_id: target_action.result_atom_id.clone(),
                result_atom_content_hash: target_action.result_atom_content_hash.clone(),
                result_proposal_id: None,
                canonical_before_hash: Some(current_hash),
                canonical_after_hash: Some(restored_hash),
                change_json,
                validation_status: LabelOntologyValidationStatus::Pending,
                validation_json,
            },
            now,
        )?;
        link_action_signals(&conn, &board_id, &action_id, &target_action.signal_ids, now)?;
        action_by_id_with_links(&conn, &action_id)
    })
}

pub fn validate_label_ontology_action(
    path: impl AsRef<Path>,
    board: &str,
    input: LabelOntologyValidationInput,
) -> Result<LabelOntologyActionRecord> {
    validate_label_ontology_action_inner(
        path,
        board,
        input,
        LabelOntologyValidationEvidenceSource::ExternalAttestation,
    )
}

pub fn validate_label_ontology_action_with_trusted_evidence(
    path: impl AsRef<Path>,
    board: &str,
    input: LabelOntologyValidationInput,
) -> Result<LabelOntologyActionRecord> {
    validate_label_ontology_action_inner(
        path,
        board,
        input,
        LabelOntologyValidationEvidenceSource::TrustedCollector,
    )
}

pub fn validate_label_ontology_action_with_trusted_suggestions(
    path: impl AsRef<Path>,
    board: &str,
    input: LabelOntologyTrustedValidationInput,
    store: &(impl LabelAtomVectorStore + ?Sized),
    options: LabelSuggestionOptions,
) -> Result<LabelOntologyActionRecord> {
    let path = path.as_ref();
    let context = validation_collection_context(
        path,
        board,
        &input.parent_action_id,
        input.signal_ids.clone(),
    )?;
    let mut collected_cases = Vec::with_capacity(context.signals.len());
    for signal in &context.signals {
        let observation = observation_for_collection(path, &signal.observation_id)?;
        let suggestion =
            suggest_task_labels_with(path, board, &observation.task_id, store, options)?;
        collected_cases.push(TrustedValidationCase {
            signal: signal.clone(),
            observation,
            suggestion,
        });
    }
    let index_status = label_atom_index_status_with(path, board, store)?;
    let validation_json = trusted_validation_json(
        &context.parent_action,
        collected_cases,
        &index_status,
        store.embedding_model(),
        options,
    )?;
    validate_label_ontology_action_with_trusted_evidence(
        path,
        board,
        LabelOntologyValidationInput {
            actor: input.actor,
            parent_action_id: input.parent_action_id,
            signal_ids: input.signal_ids,
            reason: input.reason,
            validation_status: input.validation_status,
            validation_json,
        },
    )
}

fn validate_label_ontology_action_inner(
    path: impl AsRef<Path>,
    board: &str,
    input: LabelOntologyValidationInput,
    evidence_source: LabelOntologyValidationEvidenceSource,
) -> Result<LabelOntologyActionRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let actor = normalize_actor(input.actor)?;
        if matches!(
            input.validation_status,
            LabelOntologyValidationStatus::Pending
        ) {
            return Err(KanbanError::InvalidInput(
                "validation action cannot record pending status".into(),
            ));
        }
        let context =
            resolve_validation_context(&conn, board, &input.parent_action_id, input.signal_ids)?;
        let validation_json = build_validation_json(
            &conn,
            &context.parent_action,
            &context.signals,
            &input.validation_json,
            input.validation_status,
            evidence_source,
        )?;
        let action_id = insert_ontology_action(
            &conn,
            &context.board_id,
            InsertOntologyAction {
                action_type: LabelOntologyActionType::Validate,
                reason: normalize_required_text(&input.reason)?,
                actor,
                parent_action_id: Some(context.parent_action_id.clone()),
                target_label_id: context.parent_action.target_label_id,
                result_label_id: context.parent_action.result_label_id,
                result_atom_id: context.parent_action.result_atom_id,
                result_atom_content_hash: context.parent_action.result_atom_content_hash,
                result_proposal_id: context.parent_action.result_proposal_id,
                canonical_before_hash: context.parent_action.canonical_before_hash,
                canonical_after_hash: context.parent_action.canonical_after_hash,
                change_json: "{}".to_owned(),
                validation_status: input.validation_status,
                validation_json,
            },
            now,
        )?;
        link_action_signals(
            &conn,
            &context.board_id,
            &action_id,
            &context.signal_ids,
            now,
        )?;
        if matches!(
            input.validation_status,
            LabelOntologyValidationStatus::Passed
        ) {
            apply_status_transition(
                &conn,
                LabelOntologyActionType::ResolveNoChange,
                &context.signal_ids,
                None,
                "validation passed",
                now,
            )?;
        }
        action_by_id_with_links(&conn, &action_id)
    })
}

pub(crate) fn record_label_ontology_proposal_bootstrap_in_tx(
    conn: &Connection,
    proposal: &LabelSemanticProposalRecord,
    result_label_id: &str,
    options: LabelOntologyProposalBootstrapOptions,
    now: i64,
) -> Result<Option<String>> {
    let LabelOntologyProposalBootstrapOptions {
        actor,
        reason,
        source_signal_ids,
        allow_retarget,
        retarget_reason,
    } = options;
    if source_signal_ids.is_empty() && (allow_retarget || retarget_reason.as_deref().is_some()) {
        return Err(KanbanError::InvalidInput(
            "proposal accept retarget options require source_signal_ids".into(),
        ));
    }
    let actor = normalize_actor(actor)?;
    let signal_ids = if source_signal_ids.is_empty() {
        Vec::new()
    } else {
        normalize_signal_ids(source_signal_ids)?
    };
    let signals = if signal_ids.is_empty() {
        Vec::new()
    } else {
        signal_ids
            .iter()
            .map(|signal_id| signal_by_id(conn, signal_id))
            .collect::<Result<Vec<_>>>()?
    };
    if !signals.is_empty() {
        ensure_signals_on_board_and_status(
            &signals,
            &proposal.board_id,
            &[LabelOntologySignalStatus::Confirmed],
        )?;
    }
    let retarget_reason = if signal_ids.is_empty() {
        None
    } else {
        normalize_retarget_reason(allow_retarget, retarget_reason, "proposal accept")?
    };
    let retarget_override = proposal_bootstrap_retarget_override(
        &signals,
        proposal,
        result_label_id,
        retarget_reason.as_deref(),
    )?;
    let label = LabelSnapshot {
        id: result_label_id.to_owned(),
        name: proposal.name.clone(),
    };
    let before = SemanticsParts::empty();
    let before_hash = semantics_hash(&label, &before)?;
    let before_json = semantics_json(&label, &before);
    let after = load_semantics_parts(conn, &proposal.board_id, result_label_id)?;
    let after_hash = semantics_hash(&label, &after)?;
    let after_json = semantics_json(&label, &after);
    let atoms = label_ontology_mutation_atoms(conn, &proposal.board_id, result_label_id)?;
    let reason = normalize_optional_text(reason)?
        .unwrap_or_else(|| "accepted label proposal from ontology signals".to_owned());
    let parent_action_id = proposal_creation_action_id(conn, &proposal.board_id, &proposal.id)?;
    let change_json = serde_json::to_string(&json!({
        "proposal": {
            "id": &proposal.id,
            "task_id": &proposal.task_id,
            "name": &proposal.name,
        },
        "result_label": {
            "id": result_label_id,
            "name": &proposal.name,
        },
        "semantics": &after_json,
        "retarget_override": &retarget_override,
    }))
    .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;
    let action_id = insert_ontology_action(
        conn,
        &proposal.board_id,
        InsertOntologyAction {
            action_type: LabelOntologyActionType::BootstrapLabel,
            reason: reason.clone(),
            actor: actor.clone(),
            parent_action_id,
            target_label_id: None,
            result_label_id: Some(result_label_id.to_owned()),
            result_atom_id: None,
            result_atom_content_hash: None,
            result_proposal_id: Some(proposal.id.clone()),
            canonical_before_hash: None,
            canonical_after_hash: Some(after_hash.clone()),
            change_json,
            validation_status: LabelOntologyValidationStatus::Pending,
            validation_json: "{}".to_owned(),
        },
        now,
    )?;
    link_action_signals(conn, &proposal.board_id, &action_id, &signal_ids, now)?;
    for atom in atoms {
        let atom_change_json = serde_json::to_string(&json!({
            "proposal": {
                "id": &proposal.id,
                "task_id": &proposal.task_id,
                "name": &proposal.name,
            },
            "result_label": {
                "id": result_label_id,
                "name": &proposal.name,
            },
            "changed": true,
            "before": &before_json,
            "after": &after_json,
            "atom": label_ontology_mutation_atom_json(&atom),
            "retarget_override": &retarget_override,
        }))
        .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;
        let atom_action_id = insert_ontology_action(
            conn,
            &proposal.board_id,
            InsertOntologyAction {
                action_type: LabelOntologyActionType::BootstrapLabel,
                reason: reason.clone(),
                actor: actor.clone(),
                parent_action_id: Some(action_id.clone()),
                target_label_id: None,
                result_label_id: Some(result_label_id.to_owned()),
                result_atom_id: Some(atom.id),
                result_atom_content_hash: Some(atom.content_hash),
                result_proposal_id: Some(proposal.id.clone()),
                canonical_before_hash: Some(before_hash.clone()),
                canonical_after_hash: Some(after_hash.clone()),
                change_json: atom_change_json,
                validation_status: LabelOntologyValidationStatus::Pending,
                validation_json: "{}".to_owned(),
            },
            now,
        )?;
        link_action_signals(conn, &proposal.board_id, &atom_action_id, &signal_ids, now)?;
    }
    Ok(Some(action_id))
}

#[derive(Debug, Clone)]
pub(crate) struct LabelOntologySemanticsSnapshot {
    pub(crate) hash: String,
    pub(crate) json: JsonValue,
}

pub(crate) struct LabelOntologySemanticsMutationInput<'a> {
    pub(crate) board_id: &'a str,
    pub(crate) label_id: &'a str,
    pub(crate) label_name: &'a str,
    pub(crate) action_type: LabelOntologyActionType,
    pub(crate) before: LabelOntologySemanticsSnapshot,
    pub(crate) options: LabelSemanticsMutationOptions,
}

#[derive(Debug, Clone)]
struct LabelOntologyMutationAtom {
    id: String,
    content_hash: String,
    polarity: String,
    kind: String,
    text: String,
    ordinal: i64,
}

pub(crate) fn label_ontology_semantics_snapshot_in_tx(
    conn: &Connection,
    board_id: &str,
    label_id: &str,
    label_name: &str,
) -> Result<LabelOntologySemanticsSnapshot> {
    let label = LabelSnapshot {
        id: label_id.to_owned(),
        name: label_name.to_owned(),
    };
    let parts = load_semantics_parts(conn, board_id, label_id)?;
    Ok(LabelOntologySemanticsSnapshot {
        hash: semantics_hash(&label, &parts)?,
        json: semantics_json(&label, &parts),
    })
}

pub(crate) fn record_label_ontology_semantics_mutation_in_tx(
    conn: &Connection,
    input: LabelOntologySemanticsMutationInput<'_>,
    now: i64,
) -> Result<Vec<String>> {
    let LabelOntologySemanticsMutationInput {
        board_id,
        label_id,
        label_name,
        action_type,
        before,
        options,
    } = input;
    let (target_label_id, result_label_id, default_reason) = match action_type {
        LabelOntologyActionType::UpdateSemantics => (
            Some(label_id.to_owned()),
            None,
            "manual label semantics update",
        ),
        LabelOntologyActionType::BootstrapLabel => {
            (None, Some(label_id.to_owned()), "direct label bootstrap")
        }
        _ => {
            return Err(KanbanError::InvalidInput(format!(
                "action type {action_type} cannot record semantics mutation provenance"
            )));
        }
    };
    let actor = normalize_actor(options.actor)?;
    let reason = normalize_optional_text(options.reason)?.unwrap_or_else(|| default_reason.into());
    let signal_ids = if options.source_signal_ids.is_empty() {
        Vec::new()
    } else {
        normalize_signal_ids(options.source_signal_ids)?
    };
    if !signal_ids.is_empty() {
        let signals = signal_ids
            .iter()
            .map(|signal_id| signal_by_id(conn, signal_id))
            .collect::<Result<Vec<_>>>()?;
        ensure_signals_on_board_and_status(
            &signals,
            board_id,
            &[LabelOntologySignalStatus::Confirmed],
        )?;
    }

    let after = label_ontology_semantics_snapshot_in_tx(conn, board_id, label_id, label_name)?;
    let atoms = label_ontology_mutation_atoms(conn, board_id, label_id)?;
    let changed = before.hash != after.hash;
    let change_json = serde_json::to_string(&json!({
        "label": {
            "id": label_id,
            "name": label_name,
        },
        "changed": changed,
        "before": before.json,
        "after": after.json,
        "atoms": atoms.iter().map(label_ontology_mutation_atom_json).collect::<Vec<_>>(),
    }))
    .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;

    let mut action_ids = Vec::new();
    if atoms.is_empty() {
        let action_id = insert_ontology_action(
            conn,
            board_id,
            InsertOntologyAction {
                action_type,
                reason: reason.clone(),
                actor: actor.clone(),
                parent_action_id: None,
                target_label_id: target_label_id.clone(),
                result_label_id: result_label_id.clone(),
                result_atom_id: None,
                result_atom_content_hash: None,
                result_proposal_id: None,
                canonical_before_hash: Some(before.hash.clone()),
                canonical_after_hash: Some(after.hash.clone()),
                change_json: change_json.clone(),
                validation_status: LabelOntologyValidationStatus::Pending,
                validation_json: "{}".to_owned(),
            },
            now,
        )?;
        link_action_signals(conn, board_id, &action_id, &signal_ids, now)?;
        action_ids.push(action_id);
        return Ok(action_ids);
    }

    for atom in atoms {
        let action_id = insert_ontology_action(
            conn,
            board_id,
            InsertOntologyAction {
                action_type,
                reason: reason.clone(),
                actor: actor.clone(),
                parent_action_id: None,
                target_label_id: target_label_id.clone(),
                result_label_id: result_label_id.clone(),
                result_atom_id: Some(atom.id),
                result_atom_content_hash: Some(atom.content_hash),
                result_proposal_id: None,
                canonical_before_hash: Some(before.hash.clone()),
                canonical_after_hash: Some(after.hash.clone()),
                change_json: change_json.clone(),
                validation_status: LabelOntologyValidationStatus::Pending,
                validation_json: "{}".to_owned(),
            },
            now,
        )?;
        link_action_signals(conn, board_id, &action_id, &signal_ids, now)?;
        action_ids.push(action_id);
    }
    Ok(action_ids)
}

fn label_ontology_mutation_atoms(
    conn: &Connection,
    board_id: &str,
    label_id: &str,
) -> Result<Vec<LabelOntologyMutationAtom>> {
    let mut stmt = conn
        .prepare(
            "SELECT id,content_hash,polarity,kind,text,ordinal \
             FROM label_atoms WHERE board_id=?1 AND label_id=?2 ORDER BY ordinal ASC, id ASC",
        )
        .map_err(storage)?;
    stmt.query_map(params![board_id, label_id], |row| {
        Ok(LabelOntologyMutationAtom {
            id: row.get(0)?,
            content_hash: row.get(1)?,
            polarity: row.get(2)?,
            kind: row.get(3)?,
            text: row.get(4)?,
            ordinal: row.get(5)?,
        })
    })
    .map_err(storage)?
    .collect::<std::result::Result<Vec<_>, _>>()
    .map_err(storage)
}

fn label_ontology_mutation_atom_json(atom: &LabelOntologyMutationAtom) -> JsonValue {
    json!({
        "id": &atom.id,
        "content_hash": &atom.content_hash,
        "polarity": &atom.polarity,
        "kind": &atom.kind,
        "text": &atom.text,
        "ordinal": atom.ordinal,
    })
}

pub(crate) fn record_label_ontology_proposal_create_in_tx(
    conn: &Connection,
    proposal: &LabelSemanticProposalRecord,
    options: LabelOntologyProposalCreateOptions,
    now: i64,
) -> Result<Option<String>> {
    let LabelOntologyProposalCreateOptions {
        actor,
        source_signal_ids,
        allow_retarget,
        retarget_reason,
    } = options;
    if source_signal_ids.is_empty() {
        if allow_retarget || retarget_reason.as_deref().is_some() {
            return Err(KanbanError::InvalidInput(
                "proposal create retarget options require source_signal_ids".into(),
            ));
        }
        return Ok(None);
    }
    let actor = normalize_actor(actor)?;
    let signal_ids = normalize_signal_ids(source_signal_ids)?;
    let signals = signal_ids
        .iter()
        .map(|signal_id| signal_by_id(conn, signal_id))
        .collect::<Result<Vec<_>>>()?;
    ensure_signals_on_board_and_status(
        &signals,
        &proposal.board_id,
        &[LabelOntologySignalStatus::Confirmed],
    )?;
    let retarget_reason =
        normalize_retarget_reason(allow_retarget, retarget_reason, "proposal create")?;
    let retarget_override =
        proposal_create_retarget_override(&signals, proposal, retarget_reason.as_deref())?;
    let change_json = serde_json::to_string(&json!({
        "proposal": {
            "id": &proposal.id,
            "task_id": &proposal.task_id,
            "status": &proposal.status,
            "name": &proposal.name,
            "description": &proposal.description,
            "applies_when": &proposal.applies_when,
            "excludes_when": &proposal.excludes_when,
            "positive_examples": &proposal.positive_examples,
            "negative_examples": &proposal.negative_examples,
        },
        "retarget_override": retarget_override,
    }))
    .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;
    let action_id = insert_ontology_action(
        conn,
        &proposal.board_id,
        InsertOntologyAction {
            action_type: LabelOntologyActionType::CreateLabelProposal,
            reason: "created label proposal from ontology signals".to_owned(),
            actor,
            parent_action_id: None,
            target_label_id: None,
            result_label_id: None,
            result_atom_id: None,
            result_atom_content_hash: None,
            result_proposal_id: Some(proposal.id.clone()),
            canonical_before_hash: None,
            canonical_after_hash: None,
            change_json,
            validation_status: LabelOntologyValidationStatus::NotRequired,
            validation_json: "{}".to_owned(),
        },
        now,
    )?;
    link_action_signals(conn, &proposal.board_id, &action_id, &signal_ids, now)?;
    Ok(Some(action_id))
}

pub(crate) struct LabelOntologyProposalBootstrapOptions {
    pub actor: LabelOntologyActor,
    pub reason: Option<String>,
    pub source_signal_ids: Vec<String>,
    pub allow_retarget: bool,
    pub retarget_reason: Option<String>,
}

pub(crate) struct LabelOntologyProposalCreateOptions {
    pub actor: LabelOntologyActor,
    pub source_signal_ids: Vec<String>,
    pub allow_retarget: bool,
    pub retarget_reason: Option<String>,
}

struct ValidationContext {
    board_id: String,
    parent_action_id: String,
    parent_action: LabelOntologyActionRecord,
    signal_ids: Vec<String>,
    signals: Vec<LabelOntologySignalRecord>,
}

struct TrustedValidationCase {
    signal: LabelOntologySignalRecord,
    observation: LabelOntologyObservationRecord,
    suggestion: LabelSuggestionResult,
}

fn validation_collection_context(
    path: impl AsRef<Path>,
    board: &str,
    parent_action_id: &str,
    signal_ids: Vec<String>,
) -> Result<ValidationContext> {
    let conn = connect_file(path.as_ref())?;
    with_read_tx(&conn, || {
        resolve_validation_context(&conn, board, parent_action_id, signal_ids)
    })
}

fn observation_for_collection(
    path: impl AsRef<Path>,
    observation_id: &str,
) -> Result<LabelOntologyObservationRecord> {
    let conn = connect_file(path.as_ref())?;
    with_read_tx(&conn, || observation_by_id(&conn, observation_id))
}

fn resolve_validation_context(
    conn: &Connection,
    board: &str,
    parent_action_id: &str,
    signal_ids: Vec<String>,
) -> Result<ValidationContext> {
    let board_id = board_id(conn, board)?;
    let parent_action_id = normalize_required_text(parent_action_id)?;
    let parent_action = action_by_id_with_links(conn, &parent_action_id)?;
    if parent_action.board_id != board_id {
        return Err(KanbanError::InvalidInput(
            "parent action belongs to a different board".into(),
        ));
    }
    ensure_validatable_parent_action(&parent_action)?;
    let explicit_signal_ids = !signal_ids.is_empty();
    let signal_ids = if explicit_signal_ids {
        normalize_signal_ids(signal_ids)?
    } else {
        parent_action.signal_ids.clone()
    };
    if explicit_signal_ids {
        for signal_id in &signal_ids {
            if !parent_action.signal_ids.contains(signal_id) {
                return Err(KanbanError::InvalidInput(format!(
                    "validation signal {signal_id} is not linked to parent action {}",
                    parent_action.id
                )));
            }
        }
    }
    let signals = signal_ids
        .iter()
        .map(|signal_id| signal_by_id(conn, signal_id))
        .collect::<Result<Vec<_>>>()?;
    ensure_signals_on_board_and_status(
        &signals,
        &board_id,
        &[LabelOntologySignalStatus::Confirmed],
    )?;
    Ok(ValidationContext {
        board_id,
        parent_action_id,
        parent_action,
        signal_ids,
        signals,
    })
}

fn trusted_validation_json(
    parent_action: &LabelOntologyActionRecord,
    cases: Vec<TrustedValidationCase>,
    index_status: &VectorStoreStatus,
    embedding_model: &str,
    options: LabelSuggestionOptions,
) -> Result<String> {
    let collected_at = SystemClock.now_ms();
    let cases = cases
        .iter()
        .map(|case| trusted_validation_case_json(parent_action, case))
        .collect::<Result<Vec<_>>>()?;
    serde_json::to_string(&json!({
        "evidence_type": "trusted_automated",
        "collector": {
            "tool": "kanban",
            "source": "label_ontology_validate_trusted",
            "collected_at": collected_at,
        },
        "embedding_model": embedding_model,
        "solver_options": {
            "output_limit": options.output_limit,
            "candidate_limit": options.candidate_limit,
            "atom_limit": options.atom_limit,
            "max_selected_labels": options.max_selected_labels,
            "min_score": options.min_score,
        },
        "index": trusted_index_json(index_status),
        "cases": cases,
    }))
    .map_err(|err| KanbanError::InvalidInput(err.to_string()))
}

fn trusted_validation_case_json(
    parent_action: &LabelOntologyActionRecord,
    case: &TrustedValidationCase,
) -> Result<JsonValue> {
    let case_type = validation_case_type(parent_action.action_type)?;
    let target_label_id = validation_target_label_id(parent_action)?;
    let (after_target, evidence_atoms, negative_evidence_atoms) = target_label_id
        .map(|label_id| trusted_after_target(&case.suggestion, label_id))
        .transpose()?
        .unwrap_or_else(|| (JsonValue::Null, Vec::new(), Vec::new()));
    let before_target = target_label_id
        .map(|label_id| trusted_before_target(&case.signal, label_id))
        .unwrap_or(JsonValue::Null);
    Ok(json!({
        "signal_id": &case.signal.id,
        "case_type": case_type,
        "passed": true,
        "target_label_id": target_label_id,
        "before": {
            "target": before_target,
            "coverage": case.observation.suggest_coverage,
            "coverage_cosine": case.observation.suggest_coverage_cosine,
            "residual_norm": case.observation.suggest_residual_norm,
            "degraded": case.observation.suggest_degraded,
            "diagnostics": parse_json_field(&case.observation.diagnostics_json, "diagnostics_json", JsonShape::Array)?,
        },
        "after": {
            "degraded": case.suggestion.degraded,
            "diagnostics": &case.suggestion.diagnostics,
            "target": after_target,
            "coverage": case.suggestion.coverage,
            "coverage_cosine": case.suggestion.coverage_cosine,
            "residual_norm": case.suggestion.residual_norm,
            "needs_new_label": case.suggestion.needs_new_label,
            "evidence_atoms": evidence_atoms,
            "negative_evidence_atoms": negative_evidence_atoms,
            "selected_labels": &case.suggestion.selected_labels,
            "candidates": &case.suggestion.candidates,
        },
        "suggestion": &case.suggestion,
    }))
}

fn validation_case_type(action_type: LabelOntologyActionType) -> Result<&'static str> {
    match action_type {
        LabelOntologyActionType::AddPositiveAtom => Ok("positive_atom"),
        LabelOntologyActionType::AddNegativeAtom => Ok("negative_atom"),
        LabelOntologyActionType::BootstrapLabel => Ok("bootstrap_label"),
        LabelOntologyActionType::UpdateSemantics => Ok("update_semantics"),
        LabelOntologyActionType::RevertOntologyMutation => Ok("revert_ontology_mutation"),
        LabelOntologyActionType::RenameLabel => Ok("rename_label"),
        LabelOntologyActionType::SplitLabel => Ok("split_label"),
        LabelOntologyActionType::MergeLabels => Ok("merge_labels"),
        _ => Err(KanbanError::InvalidInput(
            "trusted validation parent action must be a canonical mutation action".into(),
        )),
    }
}

fn validation_target_label_id(action: &LabelOntologyActionRecord) -> Result<Option<&str>> {
    match action.action_type {
        LabelOntologyActionType::AddPositiveAtom | LabelOntologyActionType::AddNegativeAtom => {
            Ok(Some(required_parent_field(
                action.target_label_id.as_deref(),
                "atom validation requires a target label",
            )?))
        }
        LabelOntologyActionType::BootstrapLabel => Ok(Some(required_parent_field(
            action.result_label_id.as_deref(),
            "bootstrap label validation requires a result label",
        )?)),
        LabelOntologyActionType::UpdateSemantics
        | LabelOntologyActionType::RevertOntologyMutation
        | LabelOntologyActionType::RenameLabel
        | LabelOntologyActionType::SplitLabel
        | LabelOntologyActionType::MergeLabels => Ok(action
            .target_label_id
            .as_deref()
            .or(action.result_label_id.as_deref())),
        _ => Ok(None),
    }
}

fn trusted_before_target(signal: &LabelOntologySignalRecord, label_id: &str) -> JsonValue {
    json!({
        "label_id": label_id,
        "selected": signal.suggest_state == Some(LabelOntologySuggestState::Selected),
        "score": signal.suggest_score,
        "state": signal.suggest_state,
        "rank": signal.suggest_rank,
    })
}

fn trusted_after_target(
    suggestion: &LabelSuggestionResult,
    label_id: &str,
) -> Result<(JsonValue, Vec<JsonValue>, Vec<JsonValue>)> {
    if let Some(selected) = suggestion
        .selected_labels
        .iter()
        .find(|selected| selected.label_id == label_id)
    {
        return Ok((
            json!({
                "label_id": &selected.label_id,
                "label_name": &selected.label_name,
                "selected": true,
                "score": selected.score,
                "weight": selected.weight,
                "already_applied": selected.already_applied,
            }),
            evidence_atoms_json(&selected.evidence_atoms),
            evidence_atoms_json(&selected.negative_evidence_atoms),
        ));
    }
    if let Some(candidate) = suggestion
        .candidates
        .iter()
        .find(|candidate| candidate.label_id == label_id)
    {
        return Ok((
            json!({
                "label_id": &candidate.label_id,
                "label_name": &candidate.label_name,
                "selected": false,
                "score": candidate.score,
                "weight": candidate.weight,
                "already_applied": candidate.already_applied,
            }),
            evidence_atoms_json(&candidate.evidence_atoms),
            evidence_atoms_json(&candidate.negative_evidence_atoms),
        ));
    }
    Ok((
        json!({
            "label_id": label_id,
            "selected": false,
            "score": null,
        }),
        Vec::new(),
        Vec::new(),
    ))
}

fn evidence_atoms_json(atoms: &[LabelSuggestionEvidenceAtom]) -> Vec<JsonValue> {
    atoms
        .iter()
        .map(|atom| {
            json!({
                "id": &atom.atom_id,
                "atom_id": &atom.atom_id,
                "label_id": &atom.label_id,
                "label_name": &atom.label_name,
                "polarity": &atom.polarity,
                "kind": &atom.kind,
                "text": &atom.text,
                "score": atom.score,
            })
        })
        .collect()
}

fn trusted_index_json(status: &VectorStoreStatus) -> JsonValue {
    let dirty = status.dirty.unwrap_or(false) || status.board_dirty.unwrap_or(false);
    let has_error = status
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("error"));
    let state = if !status.enabled {
        "disabled"
    } else if dirty {
        "dirty"
    } else if has_error {
        "error"
    } else {
        "ready"
    };
    json!({
        "status": state,
        "backend": &status.backend,
        "enabled": status.enabled,
        "dirty": dirty,
        "store_dirty": status.dirty,
        "board_dirty": status.board_dirty,
        "diagnostics": &status.diagnostics,
        "message": &status.message,
        "generation": status.generation,
    })
}

fn build_validation_json(
    conn: &Connection,
    parent_action: &LabelOntologyActionRecord,
    signals: &[LabelOntologySignalRecord],
    supplied_json: &str,
    status: LabelOntologyValidationStatus,
    evidence_source: LabelOntologyValidationEvidenceSource,
) -> Result<String> {
    let manual = parse_json_field(supplied_json, "validation_json", JsonShape::Object)?;
    ensure_passed_validation_evidence(&manual, parent_action, signals, status, evidence_source)?;
    if matches!(status, LabelOntologyValidationStatus::Passed)
        && matches!(
            evidence_source,
            LabelOntologyValidationEvidenceSource::TrustedCollector
        )
        && is_tool_collected_trusted_validation(&manual)
    {
        ensure_trusted_validation_still_current(conn, parent_action, &manual)?;
    }
    let mut cases = Vec::with_capacity(signals.len());
    let mut stale_count = 0usize;
    let mut degraded_count = 0usize;
    let mut metadata_drift_count = 0usize;
    let mut label_binding_drift_count = 0usize;
    let mut suggest_input_drift_count = 0usize;
    let mut legacy_incomparable_count = 0usize;
    for signal in signals {
        let observation = observation_by_id(conn, &signal.observation_id)?;
        let captured_snapshot = parse_json_field(
            &observation.task_snapshot_json,
            "task_snapshot_json",
            JsonShape::Object,
        )?;
        let captured_hash = captured_snapshot
            .get("content_hash")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_owned();
        let captured_suggest_input_hash = observation.suggest_input_hash.clone();
        let current_task = get_task_by_id(conn, &observation.board_id, &observation.task_id)?;
        let current_snapshot_json = task_snapshot_json(&current_task)?;
        let current_snapshot = parse_json_field(
            &current_snapshot_json,
            "current_task_snapshot_json",
            JsonShape::Object,
        )?;
        let current_hash = current_snapshot
            .get("content_hash")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_owned();
        let current_suggest_input_hash = suggest_input_hash_for_task(&current_task);
        let legacy_incomparable = captured_suggest_input_hash.is_none();
        let suggest_input_drift = captured_suggest_input_hash
            .as_deref()
            .is_some_and(|hash| hash != current_suggest_input_hash);
        let stale = legacy_incomparable || suggest_input_drift;
        if stale {
            stale_count += 1;
        }
        if suggest_input_drift {
            suggest_input_drift_count += 1;
        }
        if legacy_incomparable {
            legacy_incomparable_count += 1;
        }
        let snapshot_drift = captured_hash != current_hash;
        let metadata_drift = snapshot_drift && !stale;
        if metadata_drift {
            metadata_drift_count += 1;
        }
        let label_binding_drift =
            metadata_drift && snapshot_labels_changed(&captured_snapshot, &current_snapshot);
        if label_binding_drift {
            label_binding_drift_count += 1;
        }
        if observation.suggest_degraded {
            degraded_count += 1;
        }
        let mut warnings = Vec::new();
        if legacy_incomparable {
            warnings.push("legacy_suggest_input_hash_missing");
        }
        if suggest_input_drift {
            warnings.push("suggest_input_drift");
        }
        if metadata_drift {
            warnings.push("task_metadata_drift");
        }
        if label_binding_drift {
            warnings.push("label_binding_drift");
        }
        cases.push(json!({
            "signal_id": &signal.id,
            "task_id": &observation.task_id,
            "task_ref_snapshot": &observation.task_ref_snapshot,
            "target_label_id": signal.target_label_id.as_ref().or(parent_action.target_label_id.as_ref()),
            "result_label_id": &parent_action.result_label_id,
            "result_atom_id": &parent_action.result_atom_id,
            "result_atom_content_hash": &parent_action.result_atom_content_hash,
            "result_proposal_id": &parent_action.result_proposal_id,
            "comparable": !stale,
            "stale": stale,
            "legacy_incomparable": legacy_incomparable,
            "warnings": warnings,
            "task_snapshot_hash": captured_hash,
            "current_task_snapshot_hash": current_hash,
            "suggest_input_hash": captured_suggest_input_hash,
            "current_suggest_input_hash": current_suggest_input_hash,
            "before": {
                "state": &signal.suggest_state,
                "score": signal.suggest_score,
                "rank": signal.suggest_rank,
                "coverage": observation.suggest_coverage,
                "coverage_cosine": observation.suggest_coverage_cosine,
                "residual_norm": observation.suggest_residual_norm,
                "degraded": observation.suggest_degraded,
                "diagnostics": parse_json_field(&observation.diagnostics_json, "diagnostics_json", JsonShape::Array)?,
            },
            "after": {
                "validation_status": status,
                "manual_case_ref": validation_manual_case_ref(&manual, &signal.id),
            },
            "passed": matches!(status, LabelOntologyValidationStatus::Passed) && !stale && !observation.suggest_degraded,
        }));
    }
    if matches!(status, LabelOntologyValidationStatus::Passed)
        && (stale_count > 0 || degraded_count > 0)
    {
        return Err(KanbanError::InvalidInput(
            "passed validation requires comparable, non-degraded source observations".into(),
        ));
    }
    serde_json::to_string(&json!({
        "manual": manual,
        "cases": cases,
        "summary": {
            "status": status,
            "case_count": signals.len(),
            "stale_count": stale_count,
            "degraded_count": degraded_count,
            "metadata_drift_count": metadata_drift_count,
            "label_binding_drift_count": label_binding_drift_count,
            "suggest_input_drift_count": suggest_input_drift_count,
            "legacy_incomparable_count": legacy_incomparable_count,
            "incomparable_count": stale_count + degraded_count,
        }
    }))
    .map_err(|err| KanbanError::InvalidInput(err.to_string()))
}

fn validation_manual_case_ref(manual: &JsonValue, signal_id: &str) -> JsonValue {
    let Some(cases) = manual.get("cases").and_then(JsonValue::as_array) else {
        return JsonValue::Null;
    };
    let Some((index, case)) = cases
        .iter()
        .enumerate()
        .find(|(_, case)| case.get("signal_id").and_then(JsonValue::as_str) == Some(signal_id))
    else {
        return JsonValue::Null;
    };
    json!({
        "source": "manual.cases",
        "index": index,
        "signal_id": case
            .get("signal_id")
            .and_then(JsonValue::as_str)
            .unwrap_or(signal_id),
    })
}

fn ensure_validatable_parent_action(action: &LabelOntologyActionRecord) -> Result<()> {
    if !matches!(
        action.action_type,
        LabelOntologyActionType::AddPositiveAtom
            | LabelOntologyActionType::AddNegativeAtom
            | LabelOntologyActionType::UpdateSemantics
            | LabelOntologyActionType::RevertOntologyMutation
            | LabelOntologyActionType::BootstrapLabel
            | LabelOntologyActionType::RenameLabel
            | LabelOntologyActionType::SplitLabel
            | LabelOntologyActionType::MergeLabels
    ) {
        return Err(KanbanError::InvalidInput(
            "validation parent action must be a canonical mutation action".into(),
        ));
    }
    if action.validation_status != LabelOntologyValidationStatus::Pending {
        return Err(KanbanError::InvalidInput(
            "validation parent action must have pending validation_status".into(),
        ));
    }

    let change = parse_json_field(
        &action.change_json,
        "parent action change_json",
        JsonShape::Object,
    )?;
    let has_change_snapshot = change.as_object().is_some_and(|object| !object.is_empty());
    let has_common_hash = action.canonical_after_hash.is_some();
    let has_evidence = match action.action_type {
        LabelOntologyActionType::AddPositiveAtom | LabelOntologyActionType::AddNegativeAtom => {
            action.target_label_id.is_some()
                && action.result_atom_id.is_some()
                && action.result_atom_content_hash.is_some()
                && action.canonical_before_hash.is_some()
                && action.canonical_after_hash.is_some()
                && change.get("added_atom").is_some()
        }
        LabelOntologyActionType::BootstrapLabel => {
            action.result_label_id.is_some()
                && action.result_proposal_id.is_some()
                && action.canonical_after_hash.is_some()
                && change.get("proposal").is_some()
                && change.get("result_label").is_some()
                && change.get("semantics").is_some()
        }
        LabelOntologyActionType::UpdateSemantics
        | LabelOntologyActionType::RevertOntologyMutation
        | LabelOntologyActionType::RenameLabel
        | LabelOntologyActionType::SplitLabel
        | LabelOntologyActionType::MergeLabels => {
            has_common_hash
                && has_change_snapshot
                && (action.target_label_id.is_some() || action.result_label_id.is_some())
        }
        _ => false,
    };
    if !has_evidence {
        return Err(KanbanError::InvalidInput(
            "validation parent action is missing canonical mutation evidence".into(),
        ));
    }
    Ok(())
}

fn ensure_passed_validation_evidence(
    manual: &JsonValue,
    parent_action: &LabelOntologyActionRecord,
    signals: &[LabelOntologySignalRecord],
    status: LabelOntologyValidationStatus,
    evidence_source: LabelOntologyValidationEvidenceSource,
) -> Result<()> {
    if !matches!(status, LabelOntologyValidationStatus::Passed) {
        return Ok(());
    }
    if matches!(
        evidence_source,
        LabelOntologyValidationEvidenceSource::ExternalAttestation
    ) {
        return Err(KanbanError::InvalidInput(
            "passed validation requires trusted evidence collected by the kanban tool; external attestation cannot close ontology signals".into(),
        ));
    }
    let cases = manual
        .get("cases")
        .and_then(JsonValue::as_array)
        .filter(|cases| !cases.is_empty())
        .ok_or_else(|| {
            KanbanError::InvalidInput(
                "passed validation requires structured validation evidence cases for every linked signal"
                    .into(),
            )
        })?;
    ensure_typed_validation_context(manual)?;
    for signal in signals {
        let Some(case) = cases.iter().find(|case| {
            case.get("signal_id").and_then(JsonValue::as_str) == Some(signal.id.as_str())
        }) else {
            return Err(KanbanError::InvalidInput(
                "passed validation requires structured validation evidence cases for every linked signal"
                    .into(),
            ));
        };
        if case.get("passed").and_then(JsonValue::as_bool) != Some(true) {
            return Err(KanbanError::InvalidInput(
                "passed validation requires each linked signal case to pass".into(),
            ));
        }
        ensure_case_type(parent_action.action_type, case)?;
        ensure_case_objects(case)?;
        match parent_action.action_type {
            LabelOntologyActionType::AddPositiveAtom => {
                ensure_positive_atom_validation_case(parent_action, case)?;
            }
            LabelOntologyActionType::AddNegativeAtom => {
                ensure_negative_atom_validation_case(parent_action, case)?;
            }
            LabelOntologyActionType::BootstrapLabel => {
                ensure_bootstrap_label_validation_case(parent_action, case)?;
            }
            LabelOntologyActionType::UpdateSemantics
            | LabelOntologyActionType::RevertOntologyMutation
            | LabelOntologyActionType::RenameLabel
            | LabelOntologyActionType::SplitLabel
            | LabelOntologyActionType::MergeLabels => {}
            _ => {
                return Err(KanbanError::InvalidInput(
                    "passed validation parent action must be a canonical mutation action".into(),
                ));
            }
        }
    }
    Ok(())
}

fn ensure_typed_validation_context(manual: &JsonValue) -> Result<()> {
    let evidence_type = required_string_field(manual, "evidence_type", "passed validation")?;
    if evidence_type != "trusted_automated" {
        return Err(KanbanError::InvalidInput(
            "passed validation requires trusted automated evidence collected by the kanban tool; reviewer attestation cannot pass hard validation".into(),
        ));
    }
    required_string_field(manual, "embedding_model", "passed validation")?;
    required_object_field(manual, "solver_options", "passed validation")?;
    let index = required_object_field(manual, "index", "passed validation")?;
    let index_status = required_string_field(index, "status", "passed validation index")?;
    if matches!(index_status, "dirty" | "error" | "disabled") {
        return Err(KanbanError::InvalidInput(
            "passed validation requires a clean, non-dirty atom index".into(),
        ));
    }
    if index.get("dirty").and_then(JsonValue::as_bool) == Some(true) {
        return Err(KanbanError::InvalidInput(
            "passed validation requires a clean, non-dirty atom index".into(),
        ));
    }
    let valid_generation = index.get("generation").is_some_and(|generation| {
        generation.is_number()
            || generation
                .as_str()
                .is_some_and(|text| !text.trim().is_empty())
    });
    if !valid_generation {
        return Err(KanbanError::InvalidInput(
            "passed validation requires atom index generation evidence".into(),
        ));
    }
    Ok(())
}

fn ensure_trusted_validation_still_current(
    conn: &Connection,
    parent_action: &LabelOntologyActionRecord,
    manual: &JsonValue,
) -> Result<()> {
    ensure_trusted_index_generation_current(conn, &parent_action.board_id, manual)?;
    ensure_parent_canonical_after_hash_current(conn, parent_action)?;
    Ok(())
}

fn is_tool_collected_trusted_validation(manual: &JsonValue) -> bool {
    manual
        .get("collector")
        .and_then(|collector| collector.get("source"))
        .and_then(JsonValue::as_str)
        == Some("label_ontology_validate_trusted")
}

fn ensure_trusted_index_generation_current(
    conn: &Connection,
    board_id: &str,
    manual: &JsonValue,
) -> Result<()> {
    let index = required_object_field(manual, "index", "trusted validation")?;
    let evidence_generation = required_index_generation(index)?;
    let state = derived_status_by_name(conn, LANCEDB_LABEL_ATOMS_STORE)?;
    let board_status = conn
        .query_row(
            "SELECT dirty,last_rebuild_at,last_error \
             FROM label_atom_index_boards WHERE store_name=?1 AND board_id=?2",
            params![LANCEDB_LABEL_ATOMS_STORE, board_id],
            |row| {
                Ok((
                    row.get::<_, bool>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(storage)?
        .unwrap_or((false, None, None));
    if state.dirty || board_status.0 || state.last_error.is_some() || board_status.2.is_some() {
        return Err(KanbanError::InvalidInput(
            "trusted validation evidence is stale because the atom index is dirty or errored"
                .into(),
        ));
    }
    if board_status.1 != Some(evidence_generation) {
        return Err(KanbanError::InvalidInput(
            "trusted validation evidence is stale because atom index generation changed".into(),
        ));
    }
    Ok(())
}

fn required_index_generation(index: &JsonValue) -> Result<i64> {
    let generation = index.get("generation").ok_or_else(|| {
        KanbanError::InvalidInput("trusted validation requires index.generation".into())
    })?;
    if let Some(value) = generation.as_i64() {
        return Ok(value);
    }
    if let Some(value) = generation.as_u64() {
        return i64::try_from(value).map_err(|_| {
            KanbanError::InvalidInput("trusted validation index.generation is out of range".into())
        });
    }
    if let Some(value) = generation.as_str() {
        return value.trim().parse::<i64>().map_err(|_| {
            KanbanError::InvalidInput("trusted validation index.generation must be numeric".into())
        });
    }
    Err(KanbanError::InvalidInput(
        "trusted validation index.generation must be numeric".into(),
    ))
}

fn ensure_parent_canonical_after_hash_current(
    conn: &Connection,
    parent_action: &LabelOntologyActionRecord,
) -> Result<()> {
    let Some(expected_hash) = parent_action.canonical_after_hash.as_deref() else {
        return Ok(());
    };
    let Some(label_id) = validation_target_label_id(parent_action)? else {
        return Ok(());
    };
    let label = resolve_label(conn, &parent_action.board_id, label_id)?;
    let current = load_semantics_parts(conn, &parent_action.board_id, &label.id)?;
    let current_hash = semantics_hash(&label, &current)?;
    if current_hash != expected_hash {
        return Err(KanbanError::InvalidInput(
            "trusted validation evidence is stale because canonical ontology state changed".into(),
        ));
    }
    Ok(())
}

fn ensure_case_type(
    action_type: LabelOntologyActionType,
    validation_case: &JsonValue,
) -> Result<()> {
    let case_type = required_string_field(validation_case, "case_type", "validation case")?;
    let expected = match action_type {
        LabelOntologyActionType::AddPositiveAtom => "positive_atom",
        LabelOntologyActionType::AddNegativeAtom => "negative_atom",
        LabelOntologyActionType::BootstrapLabel => "bootstrap_label",
        LabelOntologyActionType::UpdateSemantics => "update_semantics",
        LabelOntologyActionType::RevertOntologyMutation => "revert_ontology_mutation",
        LabelOntologyActionType::RenameLabel => "rename_label",
        LabelOntologyActionType::SplitLabel => "split_label",
        LabelOntologyActionType::MergeLabels => "merge_labels",
        _ => "unsupported",
    };
    if case_type != expected {
        return Err(KanbanError::InvalidInput(format!(
            "validation case type {case_type} does not match parent action {expected}"
        )));
    }
    Ok(())
}

fn ensure_case_objects(validation_case: &JsonValue) -> Result<()> {
    required_object_field(validation_case, "before", "validation case")?;
    let after = required_object_field(validation_case, "after", "validation case")?;
    if after.get("degraded").and_then(JsonValue::as_bool) != Some(false) {
        return Err(KanbanError::InvalidInput(
            "passed validation requires non-degraded after evidence".into(),
        ));
    }
    Ok(())
}

fn ensure_positive_atom_validation_case(
    parent_action: &LabelOntologyActionRecord,
    validation_case: &JsonValue,
) -> Result<()> {
    let target_label_id = required_parent_field(
        parent_action.target_label_id.as_deref(),
        "positive atom validation requires a target label",
    )?;
    ensure_target_label_matches(validation_case, "before", target_label_id, "positive atom")?;
    let after_target =
        ensure_target_label_matches(validation_case, "after", target_label_id, "positive atom")?;
    if after_target.selected != Some(true)
        && !after_target
            .score
            .is_some_and(|score| score >= LABEL_ONTOLOGY_VALIDATION_SCORE_THRESHOLD)
    {
        return Err(KanbanError::InvalidInput(
            "positive atom validation requires target label selected or score >= 0.50".into(),
        ));
    }
    ensure_score_and_coverage_not_worse(validation_case, "positive atom")?;
    ensure_result_atom_evidence(parent_action, validation_case, "positive atom")?;
    Ok(())
}

fn ensure_negative_atom_validation_case(
    parent_action: &LabelOntologyActionRecord,
    validation_case: &JsonValue,
) -> Result<()> {
    let target_label_id = required_parent_field(
        parent_action.target_label_id.as_deref(),
        "negative atom validation requires a target label",
    )?;
    let before_target =
        ensure_target_label_matches(validation_case, "before", target_label_id, "negative atom")?;
    let after_target =
        ensure_target_label_matches(validation_case, "after", target_label_id, "negative atom")?;
    let score_dropped = before_target
        .score
        .zip(after_target.score)
        .is_some_and(|(before, after)| after < before);
    let suppression_proven = after_target.selected == Some(false) || score_dropped;
    if !suppression_proven {
        return Err(KanbanError::InvalidInput(
            "negative atom validation requires false-positive target selected=false or a lower after score".into(),
        ));
    }
    ensure_result_negative_atom_evidence(parent_action, validation_case)?;
    ensure_positive_controls_or_waiver(validation_case)?;
    Ok(())
}

fn ensure_bootstrap_label_validation_case(
    parent_action: &LabelOntologyActionRecord,
    validation_case: &JsonValue,
) -> Result<()> {
    let result_label_id = required_parent_field(
        parent_action.result_label_id.as_deref(),
        "bootstrap label validation requires a result label",
    )?;
    let after_target =
        ensure_target_label_matches(validation_case, "after", result_label_id, "bootstrap label")?;
    if after_target.selected != Some(true)
        && !after_target
            .score
            .is_some_and(|score| score >= LABEL_ONTOLOGY_VALIDATION_SCORE_THRESHOLD)
    {
        return Err(KanbanError::InvalidInput(
            "bootstrap label validation requires new label selected or score >= 0.50".into(),
        ));
    }
    if !after_evidence_atoms(validation_case)
        .iter()
        .any(|atom| atom.get("label_id").and_then(JsonValue::as_str) == Some(result_label_id))
    {
        return Err(KanbanError::InvalidInput(
            "bootstrap label validation requires evidence from new label atoms".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct TargetEvidence {
    selected: Option<bool>,
    score: Option<f64>,
}

fn ensure_target_label_matches(
    validation_case: &JsonValue,
    phase: &str,
    label_id: &str,
    context: &str,
) -> Result<TargetEvidence> {
    let phase_object = required_object_field(validation_case, phase, "validation case")?;
    let target = required_object_field(phase_object, "target", context)?;
    let case_label_id = target
        .get("label_id")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            KanbanError::InvalidInput(format!("{context} validation requires target label id"))
        })?;
    if case_label_id != label_id {
        return Err(KanbanError::InvalidInput(format!(
            "{context} validation target label does not match parent action"
        )));
    }
    Ok(TargetEvidence {
        selected: target.get("selected").and_then(JsonValue::as_bool),
        score: target.get("score").and_then(JsonValue::as_f64),
    })
}

fn ensure_score_and_coverage_not_worse(validation_case: &JsonValue, context: &str) -> Result<()> {
    let before = required_object_field(validation_case, "before", "validation case")?;
    let after = required_object_field(validation_case, "after", "validation case")?;
    let before_score = before
        .get("target")
        .and_then(|target| target.get("score"))
        .and_then(JsonValue::as_f64);
    let after_score = after
        .get("target")
        .and_then(|target| target.get("score"))
        .and_then(JsonValue::as_f64);
    if before_score
        .zip(after_score)
        .is_some_and(|(before, after)| after < before)
    {
        return Err(KanbanError::InvalidInput(format!(
            "{context} validation target score regressed"
        )));
    }
    let before_coverage = before.get("coverage").and_then(JsonValue::as_f64);
    let after_coverage = after.get("coverage").and_then(JsonValue::as_f64);
    if before_coverage
        .zip(after_coverage)
        .is_some_and(|(before, after)| after < before)
    {
        return Err(KanbanError::InvalidInput(format!(
            "{context} validation coverage regressed"
        )));
    }
    Ok(())
}

fn ensure_result_atom_evidence(
    parent_action: &LabelOntologyActionRecord,
    validation_case: &JsonValue,
    context: &str,
) -> Result<()> {
    ensure_result_atom_evidence_in_field(parent_action, validation_case, context, "evidence_atoms")
}

fn ensure_result_negative_atom_evidence(
    parent_action: &LabelOntologyActionRecord,
    validation_case: &JsonValue,
) -> Result<()> {
    ensure_result_atom_evidence_in_field(
        parent_action,
        validation_case,
        "negative atom",
        "negative_evidence_atoms",
    )
}

fn ensure_result_atom_evidence_in_field(
    parent_action: &LabelOntologyActionRecord,
    validation_case: &JsonValue,
    context: &str,
    evidence_field: &str,
) -> Result<()> {
    let result_atom_id = required_parent_field(
        parent_action.result_atom_id.as_deref(),
        "atom validation requires a result atom id",
    )?;
    let result_atom_content_hash = required_parent_field(
        parent_action.result_atom_content_hash.as_deref(),
        "atom validation requires a result atom content hash",
    )?;
    if !after_evidence_atoms_field(validation_case, evidence_field)
        .iter()
        .any(|atom| {
            atom.get("id").and_then(JsonValue::as_str) == Some(result_atom_id)
                || atom.get("content_hash").and_then(JsonValue::as_str)
                    == Some(result_atom_content_hash)
        })
    {
        return Err(KanbanError::InvalidInput(format!(
            "{context} validation requires result atom evidence in after.{evidence_field}"
        )));
    }
    Ok(())
}

fn ensure_positive_controls_or_waiver(validation_case: &JsonValue) -> Result<()> {
    let after = required_object_field(validation_case, "after", "validation case")?;
    let Some(controls) = after.get("positive_controls") else {
        return ensure_positive_control_waiver(after);
    };
    let controls = controls.as_array().ok_or_else(|| {
        KanbanError::InvalidInput(
            "negative atom validation positive controls must be an array".into(),
        )
    })?;
    if controls.is_empty() {
        return ensure_positive_control_waiver(after);
    }
    for control in controls {
        if control.get("passed").and_then(JsonValue::as_bool) != Some(true)
            || control.get("regressed").and_then(JsonValue::as_bool) == Some(true)
        {
            return Err(KanbanError::InvalidInput(
                "negative atom validation requires every positive control to pass without regression"
                    .into(),
            ));
        }
    }
    Ok(())
}

fn ensure_positive_control_waiver(after: &JsonValue) -> Result<()> {
    let waiver = after
        .get("positive_control_waiver")
        .or_else(|| after.get("positive_controls_waiver"))
        .ok_or_else(|| {
            KanbanError::InvalidInput(
                "negative atom validation requires at least one positive control or a waiver reason"
                    .into(),
            )
        })?;
    let reason = waiver
        .as_str()
        .or_else(|| waiver.get("reason").and_then(JsonValue::as_str));
    if reason.is_some_and(|reason| !reason.trim().is_empty()) {
        return Ok(());
    }
    Err(KanbanError::InvalidInput(
        "negative atom validation positive control waiver requires a non-empty reason".into(),
    ))
}

fn after_evidence_atoms(validation_case: &JsonValue) -> Vec<&JsonValue> {
    after_evidence_atoms_field(validation_case, "evidence_atoms")
}

fn after_evidence_atoms_field<'a>(
    validation_case: &'a JsonValue,
    evidence_field: &str,
) -> Vec<&'a JsonValue> {
    validation_case
        .get("after")
        .and_then(|after| after.get(evidence_field))
        .and_then(JsonValue::as_array)
        .map(|atoms| atoms.iter().collect())
        .unwrap_or_default()
}

fn required_parent_field<'a>(value: Option<&'a str>, message: &str) -> Result<&'a str> {
    value.ok_or_else(|| KanbanError::InvalidInput(message.into()))
}

fn required_object_field<'a>(
    value: &'a JsonValue,
    field: &str,
    context: &str,
) -> Result<&'a JsonValue> {
    value
        .get(field)
        .filter(|child| child.is_object())
        .ok_or_else(|| {
            KanbanError::InvalidInput(format!("{context} requires object field {field}"))
        })
}

fn required_string_field<'a>(value: &'a JsonValue, field: &str, context: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| {
            KanbanError::InvalidInput(format!("{context} requires string field {field}"))
        })
}

fn ensure_no_supersede_cycle(
    conn: &Connection,
    board_id: &str,
    source_signal_ids: &[String],
    replacement_signal_id: &str,
) -> Result<()> {
    let mut visited = Vec::new();
    let mut current_id = replacement_signal_id.to_owned();
    loop {
        if source_signal_ids
            .iter()
            .any(|source_id| source_id == &current_id)
        {
            return Err(KanbanError::InvalidInput(format!(
                "supersede cycle detected: replacement {replacement_signal_id} reaches source signal {current_id}"
            )));
        }
        if visited.iter().any(|seen| seen == &current_id) {
            return Err(KanbanError::InvalidInput(format!(
                "supersede cycle detected in replacement chain from {replacement_signal_id} at signal {current_id}"
            )));
        }
        visited.push(current_id.clone());
        let current = signal_by_id(conn, &current_id)?;
        if current.board_id != board_id {
            return Err(KanbanError::InvalidInput(format!(
                "supersede replacement chain signal {} belongs to a different board",
                current.id
            )));
        }
        let Some(next_id) = current.superseded_by_signal_id else {
            return Ok(());
        };
        current_id = next_id;
    }
}

fn insert_signal(
    conn: &Connection,
    board_id: &str,
    observation_id: &str,
    input: LabelOntologySignalInput,
    now: i64,
) -> Result<LabelOntologySignalRecord> {
    let target_label = input
        .target_label_ref
        .as_deref()
        .map(|label_ref| resolve_label(conn, board_id, label_ref))
        .transpose()?;
    let related_labels = parse_json_field(
        &input.related_labels_json,
        "related_labels_json",
        JsonShape::Array,
    )?;
    let related_labels_json = serde_json::to_string(&related_labels)
        .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;
    let proposal = parse_json_field(&input.proposal_json, "proposal_json", JsonShape::Object)?;
    let proposal_json = serde_json::to_string(&proposal)
        .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;
    let proposed_label_name = input
        .proposed_label_name
        .as_deref()
        .map(normalize_required_text)
        .transpose()?;
    let proposed_label_name_normalized = proposed_label_name
        .as_deref()
        .map(normalize_label_name)
        .transpose()?;
    let candidate_atom = input
        .candidate_atom
        .as_ref()
        .map(normalize_candidate_atom)
        .transpose()?;
    ensure_signal_action_contract(
        input.proposed_action,
        target_label.as_ref(),
        candidate_atom.as_ref(),
        proposed_label_name.as_deref(),
        &related_labels,
    )?;
    let candidate_content_hash = candidate_atom.as_ref().and_then(|candidate| {
        target_label.as_ref().map(|label| {
            stable_hash(&format!(
                "{}\n{}\n{}\n{}",
                label.id,
                candidate.polarity,
                candidate.kind,
                normalize_atom_text(&candidate.text)
            ))
        })
    });
    let signal_key = input
        .signal_key
        .as_deref()
        .map(normalize_required_text)
        .transpose()?
        .unwrap_or_else(|| {
            stable_hash(&format!(
                "{}\n{}\n{}\n{}\n{}\n{}",
                input.kind,
                input.proposed_action,
                target_label
                    .as_ref()
                    .map(|label| label.id.as_str())
                    .unwrap_or(""),
                candidate_content_hash.as_deref().unwrap_or(""),
                proposed_label_name_normalized.as_deref().unwrap_or(""),
                input.rationale
            ))
        });
    let rationale = normalize_required_text(&input.rationale)?;

    let signal_id = new_typed_id("los");
    let candidate_polarity = candidate_atom
        .as_ref()
        .map(|candidate| candidate.polarity.clone());
    let candidate_kind = candidate_atom
        .as_ref()
        .map(|candidate| candidate.kind.clone());
    let candidate_text = candidate_atom
        .as_ref()
        .map(|candidate| candidate.text.clone());

    exec(
        conn,
        "INSERT INTO label_ontology_signals(\
         id, observation_id, board_id, kind, status, target_label_id, target_label_name_snapshot, \
         related_labels_json, proposed_action, candidate_atom_polarity, candidate_atom_kind, \
         candidate_text, candidate_content_hash, proposed_label_name, proposed_label_name_normalized, \
         proposal_json, agent_selected, suggest_state, suggest_score, suggest_rank, final_selected, \
         rationale, confidence, signal_key, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'open', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?24)",
        params![
            signal_id,
            observation_id,
            board_id,
            input.kind.to_string(),
            target_label.as_ref().map(|label| label.id.as_str()),
            target_label.as_ref().map(|label| label.name.as_str()),
            related_labels_json,
            input.proposed_action.to_string(),
            candidate_polarity,
            candidate_kind,
            candidate_text,
            candidate_content_hash,
            proposed_label_name,
            proposed_label_name_normalized,
            proposal_json,
            bool_int(input.agent_selected),
            input.suggest_state.map(|state| state.to_string()),
            input.suggest_score,
            input.suggest_rank,
            bool_int(input.final_selected),
            rationale,
            input.confidence,
            signal_key,
            now,
        ],
    )?;
    signal_by_id(conn, &signal_id)
}

fn normalize_actor(actor: LabelOntologyActor) -> Result<LabelOntologyActor> {
    let name = normalize_required_text(&actor.name)?;
    let actor_type = normalize_required_text(&actor.actor_type)?;
    if !matches!(actor_type.as_str(), "user" | "agent") {
        return Err(KanbanError::InvalidInput(
            "ontology actor type must be user or agent".into(),
        ));
    }
    let agent_type = actor
        .agent_type
        .as_deref()
        .map(normalize_required_text)
        .transpose()?;
    if actor_type == "user" && agent_type.is_some() {
        return Err(KanbanError::InvalidInput(
            "ontology agent_type is only allowed when actor type is agent".into(),
        ));
    }
    if actor_type == "agent" && agent_type.is_none() {
        return Err(KanbanError::InvalidInput(
            "ontology agent_type is required when actor type is agent".into(),
        ));
    }
    Ok(LabelOntologyActor {
        name,
        actor_type,
        agent_type,
    })
}

fn ensure_observation_metric_contract(input: &LabelOntologyRecordInput) -> Result<()> {
    ensure_unit_metric(input.suggest_coverage, "suggest_coverage")?;
    ensure_unit_metric(input.suggest_coverage_cosine, "suggest_coverage_cosine")?;
    ensure_unit_metric(input.suggest_residual_norm, "suggest_residual_norm")?;
    Ok(())
}

fn derive_record_metrics_from_snapshot(
    mut input: LabelOntologyRecordInput,
) -> Result<LabelOntologyRecordInput> {
    let snapshot = parse_json_field(
        &input.suggestion_snapshot_json,
        "suggestion_snapshot_json",
        JsonShape::Object,
    )?;
    input.suggest_coverage = derive_snapshot_f64(
        input.suggest_coverage,
        &snapshot,
        "coverage",
        "suggest_coverage",
    )?;
    input.suggest_coverage_cosine = derive_snapshot_f64(
        input.suggest_coverage_cosine,
        &snapshot,
        "coverage_cosine",
        "suggest_coverage_cosine",
    )?;
    input.suggest_residual_norm = derive_snapshot_f64(
        input.suggest_residual_norm,
        &snapshot,
        "residual_norm",
        "suggest_residual_norm",
    )?;
    input.suggest_needs_new_label =
        derive_snapshot_bool(input.suggest_needs_new_label, &snapshot, "needs_new_label")?;
    input.suggest_degraded = derive_snapshot_bool(input.suggest_degraded, &snapshot, "degraded")?;
    input.diagnostics_json = derive_diagnostics_json(&input.diagnostics_json, &snapshot)?;
    Ok(input)
}

fn derive_snapshot_f64(
    supplied: Option<f64>,
    snapshot: &JsonValue,
    snapshot_field: &str,
    supplied_field: &str,
) -> Result<Option<f64>> {
    let derived = optional_snapshot_f64(snapshot, snapshot_field)?;
    if let (Some(supplied), Some(derived)) = (supplied, derived)
        && (supplied - derived).abs() > f64::EPSILON
    {
        return Err(KanbanError::InvalidInput(format!(
            "{supplied_field} conflicts with suggestion_snapshot_json.{snapshot_field}"
        )));
    }
    Ok(supplied.or(derived))
}

fn derive_snapshot_bool(
    supplied: bool,
    snapshot: &JsonValue,
    snapshot_field: &str,
) -> Result<bool> {
    let derived = optional_snapshot_bool(snapshot, snapshot_field)?;
    Ok(derived.unwrap_or(supplied))
}

fn derive_diagnostics_json(input: &str, snapshot: &JsonValue) -> Result<String> {
    let supplied = parse_json_field(input, "diagnostics_json", JsonShape::Array)?;
    let derived = optional_snapshot_array(snapshot, "diagnostics")?;
    if let Some(derived) = derived {
        if supplied != serde_json::json!([]) && supplied != derived {
            return Err(KanbanError::InvalidInput(
                "diagnostics_json conflicts with suggestion_snapshot_json.diagnostics".into(),
            ));
        }
        serde_json::to_string(&derived).map_err(|err| KanbanError::InvalidInput(err.to_string()))
    } else {
        serde_json::to_string(&supplied).map_err(|err| KanbanError::InvalidInput(err.to_string()))
    }
}

fn optional_snapshot_f64(snapshot: &JsonValue, field: &str) -> Result<Option<f64>> {
    let Some(value) = snapshot.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value.as_f64().map(Some).ok_or_else(|| {
        KanbanError::InvalidInput(format!(
            "suggestion_snapshot_json.{field} must be a JSON number"
        ))
    })
}

fn optional_snapshot_bool(snapshot: &JsonValue, field: &str) -> Result<Option<bool>> {
    let Some(value) = snapshot.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value.as_bool().map(Some).ok_or_else(|| {
        KanbanError::InvalidInput(format!(
            "suggestion_snapshot_json.{field} must be a JSON boolean"
        ))
    })
}

fn optional_snapshot_array(snapshot: &JsonValue, field: &str) -> Result<Option<JsonValue>> {
    let Some(value) = snapshot.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    if !value.is_array() {
        return Err(KanbanError::InvalidInput(format!(
            "suggestion_snapshot_json.{field} must be a JSON array"
        )));
    }
    Ok(Some(value.clone()))
}

fn ensure_raw_signal_metric_contract(signals: &[LabelOntologySignalInput]) -> Result<()> {
    for signal in signals {
        ensure_unit_metric(signal.suggest_score, "suggest_score")?;
        ensure_unit_metric(signal.confidence, "signal confidence")?;
        if let Some(rank) = signal.suggest_rank
            && rank < 1
        {
            return Err(KanbanError::InvalidInput(
                "suggest_rank must be null or >= 1".into(),
            ));
        }
        if let Some(candidate_atom) = signal.candidate_atom.as_ref() {
            normalize_candidate_atom(candidate_atom)?;
        }
    }
    Ok(())
}

fn ensure_unit_metric(value: Option<f64>, field: &str) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    if !value.is_finite() {
        return Err(KanbanError::InvalidInput(format!("{field} must be finite")));
    }
    if !(0.0..=1.0).contains(&value) {
        return Err(KanbanError::InvalidInput(format!(
            "{field} must be between 0.0 and 1.0"
        )));
    }
    Ok(())
}

fn ensure_signal_action_contract(
    proposed_action: LabelOntologyProposedAction,
    target_label: Option<&LabelSnapshot>,
    candidate_atom: Option<&LabelOntologyCandidateAtomInput>,
    proposed_label_name: Option<&str>,
    related_labels: &JsonValue,
) -> Result<()> {
    match proposed_action {
        LabelOntologyProposedAction::Observe => {}
        LabelOntologyProposedAction::AddPositiveAtom => {
            if target_label.is_none() {
                return Err(KanbanError::InvalidInput(
                    "add_positive_atom requires target_label_ref".into(),
                ));
            }
            match candidate_atom {
                Some(atom) if atom.polarity == "positive" => {}
                Some(_) => {
                    return Err(KanbanError::InvalidInput(
                        "add_positive_atom requires positive candidate_atom".into(),
                    ));
                }
                None => {
                    return Err(KanbanError::InvalidInput(
                        "add_positive_atom requires candidate_atom".into(),
                    ));
                }
            }
        }
        LabelOntologyProposedAction::AddNegativeAtom => {
            if target_label.is_none() {
                return Err(KanbanError::InvalidInput(
                    "add_negative_atom requires target_label_ref".into(),
                ));
            }
            match candidate_atom {
                Some(atom) if atom.polarity == "negative" => {}
                Some(_) => {
                    return Err(KanbanError::InvalidInput(
                        "add_negative_atom requires negative candidate_atom".into(),
                    ));
                }
                None => {
                    return Err(KanbanError::InvalidInput(
                        "add_negative_atom requires candidate_atom".into(),
                    ));
                }
            }
        }
        LabelOntologyProposedAction::UpdateSemantics => {
            if target_label.is_none() {
                return Err(KanbanError::InvalidInput(
                    "update_semantics requires target_label_ref".into(),
                ));
            }
        }
        LabelOntologyProposedAction::BootstrapLabel => {
            if proposed_label_name.is_none() {
                return Err(KanbanError::InvalidInput(
                    "bootstrap_label requires proposed_label_name".into(),
                ));
            }
        }
        LabelOntologyProposedAction::RenameLabel => {
            if target_label.is_none() {
                return Err(KanbanError::InvalidInput(
                    "rename_label requires target_label_ref".into(),
                ));
            }
            if proposed_label_name.is_none() {
                return Err(KanbanError::InvalidInput(
                    "rename_label requires proposed_label_name".into(),
                ));
            }
        }
        LabelOntologyProposedAction::SplitLabel => {
            if target_label.is_none() {
                return Err(KanbanError::InvalidInput(
                    "split_label requires target_label_ref".into(),
                ));
            }
            if !json_array_is_non_empty(related_labels) {
                return Err(KanbanError::InvalidInput(
                    "split_label requires non-empty related_labels_json".into(),
                ));
            }
        }
        LabelOntologyProposedAction::MergeLabels => {
            if target_label.is_none() {
                return Err(KanbanError::InvalidInput(
                    "merge_labels requires target_label_ref".into(),
                ));
            }
            if !json_array_is_non_empty(related_labels) {
                return Err(KanbanError::InvalidInput(
                    "merge_labels requires non-empty related_labels_json".into(),
                ));
            }
        }
    }
    Ok(())
}

fn json_array_is_non_empty(value: &JsonValue) -> bool {
    value.as_array().is_some_and(|items| !items.is_empty())
}

fn normalize_candidate_atom(
    input: &LabelOntologyCandidateAtomInput,
) -> Result<LabelOntologyCandidateAtomInput> {
    let polarity = normalize_required_text(&input.polarity)?;
    if !matches!(polarity.as_str(), "positive" | "negative") {
        return Err(KanbanError::InvalidInput(
            "candidate atom polarity must be positive or negative".into(),
        ));
    }
    let kind = normalize_required_text(&input.kind)?;
    if !matches!(
        kind.as_str(),
        "applies_when" | "positive_example" | "excludes_when" | "negative_example"
    ) {
        return Err(KanbanError::InvalidInput(
            "candidate atom kind is invalid".into(),
        ));
    }
    let expected_polarity = polarity_for_atom_kind(&kind)?;
    if polarity != expected_polarity {
        return Err(KanbanError::InvalidInput(format!(
            "candidate atom polarity {polarity} does not match kind {kind}; {kind} requires {expected_polarity} polarity"
        )));
    }
    let text = normalize_atom_text(&input.text);
    if text.is_empty() {
        return Err(KanbanError::InvalidInput(
            "candidate atom text is required".into(),
        ));
    }
    Ok(LabelOntologyCandidateAtomInput {
        polarity,
        kind,
        text,
    })
}

fn normalize_required_text(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(KanbanError::InvalidInput("required text is empty".into()));
    }
    Ok(value.to_owned())
}

fn normalize_optional_text(value: Option<String>) -> Result<Option<String>> {
    value.as_deref().map(normalize_required_text).transpose()
}

fn normalize_signal_ids(signal_ids: Vec<String>) -> Result<Vec<String>> {
    let mut normalized = Vec::new();
    for signal_id in signal_ids {
        let signal_id = normalize_required_text(&signal_id)?;
        if !signal_id.starts_with("los_") {
            return Err(KanbanError::InvalidInput(
                "signal id must be a canonical los_ id".into(),
            ));
        }
        if !normalized.contains(&signal_id) {
            normalized.push(signal_id);
        }
    }
    if normalized.is_empty() {
        return Err(KanbanError::InvalidInput(
            "at least one ontology signal id is required".into(),
        ));
    }
    Ok(normalized)
}

fn normalize_label_name(value: &str) -> Result<String> {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.is_empty() {
        return Err(KanbanError::InvalidInput("label name is required".into()));
    }
    Ok(value.to_ascii_lowercase())
}

#[derive(Clone, Copy)]
enum JsonShape {
    Array,
    Object,
}

fn normalize_json_field(input: &str, field: &str, shape: JsonShape) -> Result<String> {
    let value = parse_json_field(input, field, shape)?;
    serde_json::to_string(&value).map_err(|err| KanbanError::InvalidInput(err.to_string()))
}

fn parse_json_field(input: &str, field: &str, shape: JsonShape) -> Result<JsonValue> {
    let value = serde_json::from_str::<JsonValue>(input)
        .map_err(|err| KanbanError::InvalidInput(format!("{field} must be valid JSON: {err}")))?;
    match shape {
        JsonShape::Array if !value.is_array() => {
            return Err(KanbanError::InvalidInput(format!(
                "{field} must be a JSON array"
            )));
        }
        JsonShape::Object if !value.is_object() => {
            return Err(KanbanError::InvalidInput(format!(
                "{field} must be a JSON object"
            )));
        }
        _ => {}
    }
    Ok(value)
}

fn task_snapshot_json(task: &super::TaskRecord) -> Result<String> {
    let mut labels = task
        .labels
        .iter()
        .map(|label| {
            json!({
                "id": &label.id,
                "board_id": &label.board_id,
                "name": &label.name,
                "color": &label.color,
            })
        })
        .collect::<Vec<_>>();
    labels.sort_by(|left, right| {
        let left_name = left.get("name").and_then(JsonValue::as_str).unwrap_or("");
        let right_name = right.get("name").and_then(JsonValue::as_str).unwrap_or("");
        left_name.cmp(right_name).then_with(|| {
            let left_id = left.get("id").and_then(JsonValue::as_str).unwrap_or("");
            let right_id = right.get("id").and_then(JsonValue::as_str).unwrap_or("");
            left_id.cmp(right_id)
        })
    });
    let labels_json =
        serde_json::to_string(&labels).map_err(|err| KanbanError::InvalidInput(err.to_string()))?;
    let content_hash = stable_hash(&format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        task.title,
        task.description.as_deref().unwrap_or(""),
        task.status.as_str(),
        task.updated_at,
        task.lock_version,
        labels_json
    ));
    serde_json::to_string(&json!({
        "id": &task.id,
        "board_id": &task.board_id,
        "ref": &task.task_ref,
        "seq": task.seq,
        "title": &task.title,
        "description": &task.description,
        "status": task.status,
        "labels": labels,
        "updated_at": task.updated_at,
        "lock_version": task.lock_version,
        "content_hash": content_hash
    }))
    .map_err(|err| KanbanError::InvalidInput(err.to_string()))
}

fn suggest_input_hash_for_task(task: &super::TaskRecord) -> String {
    stable_hash(&task_suggest_input_text(
        &task.title,
        task.description.as_deref(),
    ))
}

fn task_suggest_input_text(title: &str, description: Option<&str>) -> String {
    match description.map(str::trim).filter(|value| !value.is_empty()) {
        Some(description) => format!("{}\n\n{}", title.trim(), description),
        None => title.trim().to_owned(),
    }
}

fn snapshot_labels_changed(captured: &JsonValue, current: &JsonValue) -> bool {
    captured.get("labels") != current.get("labels")
}

fn add_in_filter(
    conditions: &mut Vec<String>,
    sql_params: &mut Vec<Value>,
    column: &str,
    values: impl IntoIterator<Item = String>,
) {
    let values = values.into_iter().collect::<Vec<_>>();
    if values.is_empty() {
        return;
    }
    let placeholders = vec!["?"; values.len()].join(",");
    conditions.push(format!("{column} IN ({placeholders})"));
    sql_params.extend(values.into_iter().map(Value::Text));
}

#[derive(Clone)]
struct ReviewActionLink {
    action_id: String,
    result_proposal_id: Option<String>,
}

struct ReviewSignalRow {
    signal_id: String,
    task_id: String,
    task_ref_snapshot: String,
    suggest_degraded: bool,
    status: LabelOntologySignalStatus,
    signal_kind: LabelOntologySignalKind,
    proposed_action: LabelOntologyProposedAction,
    target_label_id: Option<String>,
    target_label_name_snapshot: Option<String>,
    candidate_atom_polarity: Option<String>,
    candidate_atom_kind: Option<String>,
    candidate_text: Option<String>,
    candidate_content_hash: Option<String>,
    proposed_label_name: Option<String>,
    proposed_label_name_normalized: Option<String>,
    suggest_score: Option<f64>,
    created_at: i64,
}

fn review_signal_row_from_row(row: &Row<'_>) -> rusqlite::Result<ReviewSignalRow> {
    let status: String = row.get(4)?;
    let signal_kind: String = row.get(5)?;
    let proposed_action: String = row.get(6)?;
    Ok(ReviewSignalRow {
        signal_id: row.get(0)?,
        task_id: row.get(1)?,
        task_ref_snapshot: row.get(2)?,
        suggest_degraded: int_bool(row.get(3)?),
        status: parse_row_enum(&status)?,
        signal_kind: parse_row_enum(&signal_kind)?,
        proposed_action: parse_row_enum(&proposed_action)?,
        target_label_id: row.get(7)?,
        target_label_name_snapshot: row.get(8)?,
        candidate_atom_polarity: row.get(9)?,
        candidate_atom_kind: row.get(10)?,
        candidate_text: row.get(11)?,
        candidate_content_hash: row.get(12)?,
        proposed_label_name: row.get(13)?,
        proposed_label_name_normalized: row.get(14)?,
        suggest_score: row.get(15)?,
        created_at: row.get(16)?,
    })
}

fn review_group_key(group_by: LabelOntologyReviewGroupBy, row: &ReviewSignalRow) -> String {
    match group_by {
        LabelOntologyReviewGroupBy::Label => row
            .target_label_id
            .clone()
            .unwrap_or_else(|| "no-target-label".to_owned()),
        LabelOntologyReviewGroupBy::CandidateAtom => row
            .candidate_content_hash
            .clone()
            .unwrap_or_else(|| review_empty_candidate_group_key(row)),
        LabelOntologyReviewGroupBy::ProposedLabel => row
            .proposed_label_name_normalized
            .clone()
            .unwrap_or_else(|| "no-proposed-label".to_owned()),
    }
}

fn review_empty_candidate_group_key(row: &ReviewSignalRow) -> String {
    let target = row
        .target_label_id
        .as_deref()
        .map(|label| format!("target:{label}"))
        .or_else(|| {
            row.target_label_name_snapshot
                .as_deref()
                .map(|label| format!("target-name:{label}"))
        })
        .or_else(|| {
            row.proposed_label_name_normalized
                .as_deref()
                .map(|label| format!("proposed:{label}"))
        })
        .unwrap_or_else(|| "no-target-or-proposed-label".to_owned());
    format!(
        "no-candidate-atom|kind:{}|{}|action:{}",
        row.signal_kind, target, row.proposed_action
    )
}

fn review_action_links_for_signals(
    conn: &Connection,
    signal_ids: &[String],
) -> Result<BTreeMap<String, Vec<ReviewActionLink>>> {
    if signal_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let placeholders = vec!["?"; signal_ids.len()].join(",");
    let sql = format!(
        "SELECT x.signal_id, a.id, a.result_proposal_id \
         FROM label_ontology_action_signals x \
         JOIN label_ontology_actions a ON a.id=x.action_id \
         WHERE x.signal_id IN ({placeholders}) \
         ORDER BY a.created_at ASC, a.id ASC"
    );
    let values = signal_ids
        .iter()
        .cloned()
        .map(Value::Text)
        .collect::<Vec<_>>();
    let mut stmt = conn.prepare(&sql).map_err(storage)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                ReviewActionLink {
                    action_id: row.get(1)?,
                    result_proposal_id: row.get(2)?,
                },
            ))
        })
        .map_err(storage)?;
    let mut links = BTreeMap::<String, Vec<ReviewActionLink>>::new();
    for row in rows {
        let (signal_id, link) = row.map_err(storage)?;
        links.entry(signal_id).or_default().push(link);
    }
    Ok(links)
}

struct ReviewAtomVariantAccumulator {
    content_hash: String,
    polarity: Option<String>,
    kind: Option<String>,
    text: Option<String>,
    signal_count: i64,
}

struct ReviewGroupAccumulator {
    group_by: LabelOntologyReviewGroupBy,
    key: String,
    label_id: Option<String>,
    label_name: Option<String>,
    candidate_atom_polarity: Option<String>,
    candidate_atom_kind: Option<String>,
    candidate_text: Option<String>,
    candidate_content_hash: Option<String>,
    proposed_label_name: Option<String>,
    proposed_label_name_normalized: Option<String>,
    task_ids: BTreeSet<String>,
    signal_count: i64,
    open_count: i64,
    confirmed_count: i64,
    resolved_count: i64,
    rejected_count: i64,
    superseded_count: i64,
    degraded_count: i64,
    scores: Vec<f64>,
    oldest_signal_at: Option<i64>,
    latest_signal_at: Option<i64>,
    sample_task_refs: Vec<String>,
    signal_ids: Vec<String>,
    action_ids: BTreeSet<String>,
    proposal_ids: BTreeSet<String>,
    labels: BTreeMap<String, Option<String>>,
    candidate_atom_variants: BTreeMap<String, ReviewAtomVariantAccumulator>,
}

impl ReviewGroupAccumulator {
    fn new(group_by: LabelOntologyReviewGroupBy, key: String) -> Self {
        Self {
            group_by,
            key,
            label_id: None,
            label_name: None,
            candidate_atom_polarity: None,
            candidate_atom_kind: None,
            candidate_text: None,
            candidate_content_hash: None,
            proposed_label_name: None,
            proposed_label_name_normalized: None,
            task_ids: BTreeSet::new(),
            signal_count: 0,
            open_count: 0,
            confirmed_count: 0,
            resolved_count: 0,
            rejected_count: 0,
            superseded_count: 0,
            degraded_count: 0,
            scores: Vec::new(),
            oldest_signal_at: None,
            latest_signal_at: None,
            sample_task_refs: Vec::new(),
            signal_ids: Vec::new(),
            action_ids: BTreeSet::new(),
            proposal_ids: BTreeSet::new(),
            labels: BTreeMap::new(),
            candidate_atom_variants: BTreeMap::new(),
        }
    }

    fn add(&mut self, row: ReviewSignalRow, action_links: &[ReviewActionLink]) {
        self.signal_count += 1;
        match row.status {
            LabelOntologySignalStatus::Open => self.open_count += 1,
            LabelOntologySignalStatus::Confirmed => self.confirmed_count += 1,
            LabelOntologySignalStatus::Resolved => self.resolved_count += 1,
            LabelOntologySignalStatus::Rejected => self.rejected_count += 1,
            LabelOntologySignalStatus::Superseded => self.superseded_count += 1,
        }
        if row.suggest_degraded {
            self.degraded_count += 1;
        }
        if self.task_ids.insert(row.task_id.clone()) && self.sample_task_refs.len() < 5 {
            self.sample_task_refs.push(row.task_ref_snapshot.clone());
        }
        if let Some(score) = row.suggest_score {
            self.scores.push(score);
        }
        self.oldest_signal_at = Some(match self.oldest_signal_at {
            Some(existing) => existing.min(row.created_at),
            None => row.created_at,
        });
        self.latest_signal_at = Some(match self.latest_signal_at {
            Some(existing) => existing.max(row.created_at),
            None => row.created_at,
        });
        self.signal_ids.push(row.signal_id);
        if self.label_id.is_none() {
            self.label_id = row.target_label_id.clone();
            self.label_name = row.target_label_name_snapshot.clone();
        }
        if self.candidate_content_hash.is_none() {
            self.candidate_atom_polarity = row.candidate_atom_polarity.clone();
            self.candidate_atom_kind = row.candidate_atom_kind.clone();
            self.candidate_text = row.candidate_text.clone();
            self.candidate_content_hash = row.candidate_content_hash.clone();
        }
        if self.proposed_label_name_normalized.is_none() {
            self.proposed_label_name = row.proposed_label_name.clone();
            self.proposed_label_name_normalized = row.proposed_label_name_normalized.clone();
        }
        if let Some(label_id) = row.target_label_id {
            self.labels
                .entry(label_id)
                .or_insert(row.target_label_name_snapshot);
        }
        if let Some(content_hash) = row.candidate_content_hash {
            let entry = self
                .candidate_atom_variants
                .entry(content_hash.clone())
                .or_insert(ReviewAtomVariantAccumulator {
                    content_hash,
                    polarity: row.candidate_atom_polarity,
                    kind: row.candidate_atom_kind,
                    text: row.candidate_text,
                    signal_count: 0,
                });
            entry.signal_count += 1;
        }
        self.add_action_links(action_links);
    }

    fn add_action_links(&mut self, action_links: &[ReviewActionLink]) {
        for link in action_links {
            self.action_ids.insert(link.action_id.clone());
            if let Some(proposal_id) = link.result_proposal_id.as_deref() {
                self.proposal_ids.insert(proposal_id.to_owned());
            }
        }
    }

    fn finish(mut self) -> LabelOntologyReviewGroup {
        self.scores.sort_by(f64::total_cmp);
        let average_score = if self.scores.is_empty() {
            None
        } else {
            Some(self.scores.iter().sum::<f64>() / self.scores.len() as f64)
        };
        let median_score = median_score(&self.scores);
        let mut labels = self
            .labels
            .into_iter()
            .map(|(id, name)| LabelOntologyReviewLabelRef { id, name })
            .collect::<Vec<_>>();
        labels.sort_by(|left, right| left.id.cmp(&right.id));
        let mut variants = self
            .candidate_atom_variants
            .into_values()
            .map(|variant| LabelOntologyReviewAtomVariant {
                content_hash: variant.content_hash,
                polarity: variant.polarity,
                kind: variant.kind,
                text: variant.text,
                signal_count: variant.signal_count,
            })
            .collect::<Vec<_>>();
        variants.sort_by(|left, right| {
            right
                .signal_count
                .cmp(&left.signal_count)
                .then_with(|| left.content_hash.cmp(&right.content_hash))
        });
        LabelOntologyReviewGroup {
            group_by: self.group_by,
            key: self.key,
            label_id: self.label_id,
            label_name: self.label_name,
            candidate_atom_polarity: self.candidate_atom_polarity,
            candidate_atom_kind: self.candidate_atom_kind,
            candidate_text: self.candidate_text,
            candidate_content_hash: self.candidate_content_hash,
            proposed_label_name: self.proposed_label_name,
            proposed_label_name_normalized: self.proposed_label_name_normalized,
            task_count: self.task_ids.len() as i64,
            signal_count: self.signal_count,
            open_count: self.open_count,
            confirmed_count: self.confirmed_count,
            resolved_count: self.resolved_count,
            rejected_count: self.rejected_count,
            superseded_count: self.superseded_count,
            degraded_count: self.degraded_count,
            average_score,
            median_score,
            oldest_signal_at: self.oldest_signal_at.unwrap_or_default(),
            latest_signal_at: self.latest_signal_at.unwrap_or_default(),
            sample_task_refs: self.sample_task_refs,
            signal_ids: self.signal_ids,
            action_count: self.action_ids.len() as i64,
            action_ids: self.action_ids.into_iter().collect(),
            proposal_ids: self.proposal_ids.into_iter().collect(),
            labels,
            candidate_atom_variants: variants,
        }
    }
}

fn median_score(scores: &[f64]) -> Option<f64> {
    match scores.len() {
        0 => None,
        len if len % 2 == 1 => Some(scores[len / 2]),
        len => Some((scores[(len / 2) - 1] + scores[len / 2]) / 2.0),
    }
}

#[derive(Clone, Default)]
struct SignalActionStats {
    action_count: i64,
    latest_action_at: Option<i64>,
}

struct TaskOntologySignalRow {
    observation_id: String,
    signal_id: String,
    kind: LabelOntologySignalKind,
    status: LabelOntologySignalStatus,
    proposed_action: LabelOntologyProposedAction,
    target_label_id: Option<String>,
    target_label_name_snapshot: Option<String>,
    candidate_atom_polarity: Option<String>,
    candidate_atom_kind: Option<String>,
    candidate_text: Option<String>,
    candidate_content_hash: Option<String>,
    proposed_label_name: Option<String>,
    proposed_label_name_normalized: Option<String>,
    suggest_score: Option<f64>,
    suggest_rank: Option<i64>,
    suggest_degraded: bool,
    suggest_input_hash: Option<String>,
    created_at: i64,
    updated_at: i64,
}

fn task_ontology_summary_for_task(
    conn: &Connection,
    task: &TaskRecord,
) -> Result<Option<TaskOntologySummary>> {
    let sql = "SELECT o.id, s.id, s.kind, s.status, s.proposed_action, \
               s.target_label_id, s.target_label_name_snapshot, s.candidate_atom_polarity, \
               s.candidate_atom_kind, s.candidate_text, s.candidate_content_hash, \
               s.proposed_label_name, s.proposed_label_name_normalized, s.suggest_score, \
               s.suggest_rank, o.suggest_degraded, o.suggest_input_hash, s.created_at, s.updated_at \
               FROM label_ontology_observations o \
               JOIN label_ontology_signals s ON s.observation_id=o.id \
               WHERE o.board_id=? AND o.task_id=? \
               ORDER BY s.created_at ASC, s.id ASC";
    let mut rows = all_values(
        conn,
        sql,
        &[
            Value::Text(task.board_id.clone()),
            Value::Text(task.id.clone()),
        ],
        task_ontology_signal_row_from_row,
    )?;
    if rows.is_empty() {
        return Ok(None);
    }

    let signal_ids = rows
        .iter()
        .map(|row| row.signal_id.clone())
        .collect::<Vec<_>>();
    let signal_count = signal_ids.len() as i64;
    let action_stats = task_ontology_action_stats_for_signals(conn, &signal_ids)?;
    let current_suggest_input_hash = suggest_input_hash_for_task(task);
    let now = SystemClock.now_ms();
    let mut observation_ids = BTreeSet::new();
    let mut open_count = 0;
    let mut confirmed_count = 0;
    let mut resolved_count = 0;
    let mut rejected_count = 0;
    let mut superseded_count = 0;
    let mut degraded_count = 0;
    let mut stale_count = 0;
    let mut suggest_input_drift_count = 0;
    let mut legacy_incomparable_count = 0;
    let mut action_count = 0;
    let mut oldest_open_confirmed_signal_at: Option<i64> = None;
    let mut latest_signal_at: Option<i64> = None;
    let mut latest_action_at: Option<i64> = None;
    let mut sample_signals = Vec::with_capacity(rows.len());

    for row in rows.drain(..) {
        observation_ids.insert(row.observation_id.clone());
        match row.status {
            LabelOntologySignalStatus::Open => open_count += 1,
            LabelOntologySignalStatus::Confirmed => confirmed_count += 1,
            LabelOntologySignalStatus::Resolved => resolved_count += 1,
            LabelOntologySignalStatus::Rejected => rejected_count += 1,
            LabelOntologySignalStatus::Superseded => superseded_count += 1,
        }
        if matches!(
            row.status,
            LabelOntologySignalStatus::Open | LabelOntologySignalStatus::Confirmed
        ) {
            oldest_open_confirmed_signal_at = Some(match oldest_open_confirmed_signal_at {
                Some(existing) => existing.min(row.created_at),
                None => row.created_at,
            });
        }
        latest_signal_at = Some(match latest_signal_at {
            Some(existing) => existing.max(row.created_at),
            None => row.created_at,
        });
        if row.suggest_degraded {
            degraded_count += 1;
        }
        let legacy_incomparable = row.suggest_input_hash.is_none();
        let suggest_input_drift = row
            .suggest_input_hash
            .as_deref()
            .is_some_and(|hash| hash != current_suggest_input_hash);
        let stale = legacy_incomparable || suggest_input_drift;
        if stale {
            stale_count += 1;
        }
        if suggest_input_drift {
            suggest_input_drift_count += 1;
        }
        if legacy_incomparable {
            legacy_incomparable_count += 1;
        }
        let stats = action_stats
            .get(&row.signal_id)
            .cloned()
            .unwrap_or_default();
        action_count += stats.action_count;
        if let Some(action_at) = stats.latest_action_at {
            latest_action_at = Some(match latest_action_at {
                Some(existing) => existing.max(action_at),
                None => action_at,
            });
        }
        sample_signals.push(TaskOntologySignalSummary {
            id: row.signal_id,
            kind: row.kind,
            status: row.status,
            proposed_action: row.proposed_action,
            target_label_id: row.target_label_id,
            target_label_name: row.target_label_name_snapshot,
            candidate_atom_polarity: row.candidate_atom_polarity,
            candidate_atom_kind: row.candidate_atom_kind,
            candidate_text: row.candidate_text,
            candidate_content_hash: row.candidate_content_hash,
            proposed_label_name: row.proposed_label_name,
            proposed_label_name_normalized: row.proposed_label_name_normalized,
            suggest_score: row.suggest_score,
            suggest_rank: row.suggest_rank,
            degraded: row.suggest_degraded,
            stale,
            legacy_incomparable,
            suggest_input_drift,
            created_at: row.created_at,
            updated_at: row.updated_at,
            latest_action_at: stats.latest_action_at,
            action_count: stats.action_count,
        });
    }

    sample_signals.sort_by(|left, right| {
        task_ontology_signal_priority(left)
            .cmp(&task_ontology_signal_priority(right))
            .then_with(|| right.created_at.cmp(&left.created_at))
            .then_with(|| left.id.cmp(&right.id))
    });
    sample_signals.truncate(5);
    let oldest_open_confirmed_signal_age_ms =
        oldest_open_confirmed_signal_at.map(|value| (now - value).max(0));
    Ok(Some(TaskOntologySummary {
        task_id: task.id.clone(),
        observation_count: observation_ids.len() as i64,
        signal_count,
        open_count,
        confirmed_count,
        resolved_count,
        rejected_count,
        superseded_count,
        degraded_count,
        stale_count,
        suggest_input_drift_count,
        legacy_incomparable_count,
        incomparable_count: stale_count + degraded_count,
        action_count,
        oldest_open_confirmed_signal_at,
        oldest_open_confirmed_signal_age_ms,
        latest_signal_at,
        latest_action_at,
        current_suggest_input_hash,
        sample_signals,
    }))
}

fn task_ontology_signal_priority(signal: &TaskOntologySignalSummary) -> u8 {
    match signal.status {
        LabelOntologySignalStatus::Open => 0,
        LabelOntologySignalStatus::Confirmed => 1,
        LabelOntologySignalStatus::Resolved => 2,
        LabelOntologySignalStatus::Rejected => 3,
        LabelOntologySignalStatus::Superseded => 4,
    }
}

fn task_ontology_signal_row_from_row(row: &Row<'_>) -> rusqlite::Result<TaskOntologySignalRow> {
    let kind: String = row.get(2)?;
    let status: String = row.get(3)?;
    let proposed_action: String = row.get(4)?;
    Ok(TaskOntologySignalRow {
        observation_id: row.get(0)?,
        signal_id: row.get(1)?,
        kind: parse_row_enum(&kind)?,
        status: parse_row_enum(&status)?,
        proposed_action: parse_row_enum(&proposed_action)?,
        target_label_id: row.get(5)?,
        target_label_name_snapshot: row.get(6)?,
        candidate_atom_polarity: row.get(7)?,
        candidate_atom_kind: row.get(8)?,
        candidate_text: row.get(9)?,
        candidate_content_hash: row.get(10)?,
        proposed_label_name: row.get(11)?,
        proposed_label_name_normalized: row.get(12)?,
        suggest_score: row.get(13)?,
        suggest_rank: row.get(14)?,
        suggest_degraded: int_bool(row.get(15)?),
        suggest_input_hash: row.get(16)?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
    })
}

fn task_ontology_action_stats_for_signals(
    conn: &Connection,
    signal_ids: &[String],
) -> Result<BTreeMap<String, SignalActionStats>> {
    if signal_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let placeholders = vec!["?"; signal_ids.len()].join(",");
    let sql = format!(
        "SELECT x.signal_id, COUNT(a.id), MAX(a.created_at) \
         FROM label_ontology_action_signals x \
         JOIN label_ontology_actions a ON a.id=x.action_id \
         WHERE x.signal_id IN ({placeholders}) \
         GROUP BY x.signal_id"
    );
    let values = signal_ids
        .iter()
        .cloned()
        .map(Value::Text)
        .collect::<Vec<_>>();
    let mut stmt = conn.prepare(&sql).map_err(storage)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                SignalActionStats {
                    action_count: row.get(1)?,
                    latest_action_at: row.get(2)?,
                },
            ))
        })
        .map_err(storage)?;
    let mut stats = BTreeMap::new();
    for row in rows {
        let (signal_id, signal_stats) = row.map_err(storage)?;
        stats.insert(signal_id, signal_stats);
    }
    Ok(stats)
}

fn observation_by_id(
    conn: &Connection,
    observation_id: &str,
) -> Result<LabelOntologyObservationRecord> {
    required_row(
        conn,
        &format!("SELECT {OBSERVATION_COLUMNS} FROM label_ontology_observations WHERE id=?1"),
        [observation_id],
        observation_from_row,
        || KanbanError::NotFound(format!("label ontology observation {observation_id}")),
    )
}

fn signals_for_observation(
    conn: &Connection,
    observation_id: &str,
) -> Result<Vec<LabelOntologySignalRecord>> {
    super::all(
        conn,
        &format!(
            "SELECT {SIGNAL_COLUMNS} FROM label_ontology_signals s WHERE s.observation_id=?1 ORDER BY s.created_at ASC, s.id ASC"
        ),
        [observation_id],
        signal_from_row,
    )
}

fn signal_by_id(conn: &Connection, signal_id: &str) -> Result<LabelOntologySignalRecord> {
    required_row(
        conn,
        &format!("SELECT {SIGNAL_COLUMNS} FROM label_ontology_signals s WHERE s.id=?1"),
        [signal_id],
        signal_from_row,
        || KanbanError::NotFound(format!("label ontology signal {signal_id}")),
    )
}

fn actions_for_signal(
    conn: &Connection,
    signal_id: &str,
) -> Result<Vec<LabelOntologyActionRecord>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {} FROM label_ontology_actions a \
             JOIN label_ontology_action_signals x ON x.action_id=a.id \
             WHERE x.signal_id=?1 ORDER BY a.created_at ASC, a.id ASC",
            ACTION_COLUMNS
        ))
        .map_err(storage)?;
    let actions = stmt
        .query_map([signal_id], action_from_row)
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    actions
        .into_iter()
        .map(|mut action| {
            action.signal_ids = action_signal_ids(conn, &action.id)?;
            Ok(action)
        })
        .collect()
}

fn action_signal_ids(conn: &Connection, action_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare(
            "SELECT signal_id FROM label_ontology_action_signals WHERE action_id=?1 ORDER BY signal_id ASC",
        )
        .map_err(storage)?;
    stmt.query_map([action_id], |row| row.get::<_, String>(0))
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn action_by_id_with_links(
    conn: &Connection,
    action_id: &str,
) -> Result<LabelOntologyActionRecord> {
    let mut action = required_row(
        conn,
        &format!("SELECT {ACTION_COLUMNS} FROM label_ontology_actions a WHERE a.id=?1"),
        [action_id],
        action_from_row,
        || KanbanError::NotFound(format!("label ontology action {action_id}")),
    )?;
    action.signal_ids = action_signal_ids(conn, &action.id)?;
    Ok(action)
}

fn proposal_creation_action_id(
    conn: &Connection,
    board_id: &str,
    proposal_id: &str,
) -> Result<Option<String>> {
    let mut stmt = conn
        .prepare(
            "SELECT id FROM label_ontology_actions \
             WHERE board_id=?1 AND result_proposal_id=?2 AND action_type='create_label_proposal' \
             ORDER BY created_at ASC, id ASC LIMIT 2",
        )
        .map_err(storage)?;
    let ids = stmt
        .query_map(params![board_id, proposal_id], |row| {
            row.get::<_, String>(0)
        })
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    match ids.as_slice() {
        [] => Ok(None),
        [id] => Ok(Some(id.clone())),
        _ => Err(KanbanError::InvalidInput(format!(
            "multiple create_label_proposal actions found for proposal {proposal_id}"
        ))),
    }
}

fn ensure_action_on_board(conn: &Connection, board_id: &str, action_id: &str) -> Result<()> {
    let action_board_id: String = required_row(
        conn,
        "SELECT board_id FROM label_ontology_actions WHERE id=?1",
        [action_id],
        |row| row.get(0),
        || KanbanError::NotFound(format!("label ontology action {action_id}")),
    )?;
    if action_board_id != board_id {
        return Err(KanbanError::InvalidInput(
            "parent action belongs to a different board".into(),
        ));
    }
    Ok(())
}

fn ensure_proposal_on_board(conn: &Connection, board_id: &str, proposal_id: &str) -> Result<()> {
    let proposal_board_id: String = required_row(
        conn,
        "SELECT board_id FROM label_semantic_proposals WHERE id=?1",
        [proposal_id],
        |row| row.get(0),
        || KanbanError::NotFound(format!("label semantic proposal {proposal_id}")),
    )?;
    if proposal_board_id != board_id {
        return Err(KanbanError::InvalidInput(
            "label semantic proposal belongs to a different board".into(),
        ));
    }
    Ok(())
}

fn ensure_revertable_action_type(action_type: LabelOntologyActionType) -> Result<()> {
    if matches!(
        action_type,
        LabelOntologyActionType::AddPositiveAtom
            | LabelOntologyActionType::AddNegativeAtom
            | LabelOntologyActionType::UpdateSemantics
    ) {
        Ok(())
    } else {
        Err(KanbanError::InvalidInput(format!(
            "ontology action type {action_type} cannot be reverted by label-scoped semantics revert"
        )))
    }
}

fn parse_action_change_json(action: &LabelOntologyActionRecord) -> Result<JsonValue> {
    let value: JsonValue = serde_json::from_str(&action.change_json)
        .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;
    if !value.is_object() {
        return Err(KanbanError::InvalidInput(format!(
            "ontology action {} change_json must be an object",
            action.id
        )));
    }
    Ok(value)
}

struct InsertOntologyAction {
    action_type: LabelOntologyActionType,
    reason: String,
    actor: LabelOntologyActor,
    parent_action_id: Option<String>,
    target_label_id: Option<String>,
    result_label_id: Option<String>,
    result_atom_id: Option<String>,
    result_atom_content_hash: Option<String>,
    result_proposal_id: Option<String>,
    canonical_before_hash: Option<String>,
    canonical_after_hash: Option<String>,
    change_json: String,
    validation_status: LabelOntologyValidationStatus,
    validation_json: String,
}

fn insert_ontology_action(
    conn: &Connection,
    board_id: &str,
    input: InsertOntologyAction,
    now: i64,
) -> Result<String> {
    let action_id = new_typed_id("loa");
    exec(
        conn,
        "INSERT INTO label_ontology_actions(\
         id, board_id, parent_action_id, action_type, reason, target_label_id, result_label_id, \
         result_atom_id, result_atom_content_hash, result_proposal_id, canonical_before_hash, \
         canonical_after_hash, change_json, validation_status, validation_json, created_by, \
         created_by_type, agent_type, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
        params![
            action_id,
            board_id,
            input.parent_action_id,
            input.action_type.to_string(),
            input.reason,
            input.target_label_id,
            input.result_label_id,
            input.result_atom_id,
            input.result_atom_content_hash,
            input.result_proposal_id,
            input.canonical_before_hash,
            input.canonical_after_hash,
            input.change_json,
            input.validation_status.to_string(),
            input.validation_json,
            input.actor.name,
            input.actor.actor_type,
            input.actor.agent_type,
            now,
        ],
    )?;
    Ok(action_id)
}

fn link_action_signals(
    conn: &Connection,
    board_id: &str,
    action_id: &str,
    signal_ids: &[String],
    now: i64,
) -> Result<()> {
    for signal_id in signal_ids {
        exec(
            conn,
            "INSERT INTO label_ontology_action_signals(board_id, action_id, signal_id, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![board_id, action_id, signal_id, now],
        )?;
    }
    Ok(())
}

fn ensure_signals_on_board_and_status(
    signals: &[LabelOntologySignalRecord],
    board_id: &str,
    statuses: &[LabelOntologySignalStatus],
) -> Result<()> {
    for signal in signals {
        if signal.board_id != board_id {
            return Err(KanbanError::InvalidInput(format!(
                "signal {} belongs to a different board",
                signal.id
            )));
        }
        if !statuses.contains(&signal.status) {
            return Err(KanbanError::InvalidTransition(format!(
                "signal {} must be one of {:?}, found {}",
                signal.id, statuses, signal.status
            )));
        }
    }
    Ok(())
}

fn ensure_structure_plan_action_type(action_type: LabelOntologyActionType) -> Result<()> {
    if matches!(
        action_type,
        LabelOntologyActionType::RenameLabel
            | LabelOntologyActionType::SplitLabel
            | LabelOntologyActionType::MergeLabels
    ) {
        Ok(())
    } else {
        Err(KanbanError::InvalidInput(format!(
            "structure plan action must be rename_label, split_label, or merge_labels; found {action_type}"
        )))
    }
}

fn resolve_structure_related_labels(
    conn: &Connection,
    board_id: &str,
    label_refs: Vec<String>,
) -> Result<Vec<LabelSnapshot>> {
    let mut labels = Vec::new();
    for label_ref in label_refs {
        let label = resolve_label(conn, board_id, &label_ref)?;
        if !labels
            .iter()
            .any(|existing: &LabelSnapshot| existing.id == label.id)
        {
            labels.push(label);
        }
    }
    Ok(labels)
}

fn ensure_structure_plan_contract(
    action_type: LabelOntologyActionType,
    target_label: &LabelSnapshot,
    proposed_label_name_normalized: Option<&str>,
    related_labels: &[LabelSnapshot],
    signals: &[LabelOntologySignalRecord],
) -> Result<()> {
    match action_type {
        LabelOntologyActionType::RenameLabel => {
            if proposed_label_name_normalized.is_none() {
                return Err(KanbanError::InvalidInput(
                    "rename_label structure plan requires proposed_label_name".into(),
                ));
            }
            if !related_labels.is_empty() {
                return Err(KanbanError::InvalidInput(
                    "rename_label structure plan does not accept related labels".into(),
                ));
            }
        }
        LabelOntologyActionType::SplitLabel | LabelOntologyActionType::MergeLabels => {
            if related_labels.is_empty() {
                return Err(KanbanError::InvalidInput(format!(
                    "{action_type} structure plan requires at least one related label"
                )));
            }
        }
        _ => unreachable!("validated by ensure_structure_plan_action_type"),
    }

    let proposed_action = structure_plan_proposed_action(action_type);
    for signal in signals {
        if signal.proposed_action != proposed_action {
            return Err(KanbanError::InvalidInput(format!(
                "signal {} proposed action {} does not match structure plan action {action_type}",
                signal.id, signal.proposed_action
            )));
        }
        if signal.target_label_id.as_deref() != Some(target_label.id.as_str()) {
            return Err(KanbanError::InvalidInput(format!(
                "signal {} does not target label {}",
                signal.id, target_label.name
            )));
        }
        if action_type == LabelOntologyActionType::RenameLabel
            && signal.proposed_label_name_normalized.as_deref() != proposed_label_name_normalized
        {
            return Err(KanbanError::InvalidInput(format!(
                "signal {} proposed label does not match structure plan proposed_label_name",
                signal.id
            )));
        }
    }
    Ok(())
}

fn structure_plan_proposed_action(
    action_type: LabelOntologyActionType,
) -> LabelOntologyProposedAction {
    match action_type {
        LabelOntologyActionType::RenameLabel => LabelOntologyProposedAction::RenameLabel,
        LabelOntologyActionType::SplitLabel => LabelOntologyProposedAction::SplitLabel,
        LabelOntologyActionType::MergeLabels => LabelOntologyProposedAction::MergeLabels,
        _ => unreachable!("validated by ensure_structure_plan_action_type"),
    }
}

fn default_structure_task_binding_policy(action_type: LabelOntologyActionType) -> &'static str {
    match action_type {
        LabelOntologyActionType::RenameLabel => "preserve_bindings",
        LabelOntologyActionType::SplitLabel => "manual_map_required",
        LabelOntologyActionType::MergeLabels => "move_related_to_target",
        _ => unreachable!("validated by ensure_structure_plan_action_type"),
    }
}

fn default_structure_validation_policy_json() -> JsonValue {
    json!({
        "required": true,
        "policy": "structure_change_plan_review",
        "trusted_validation_required_before_apply": true,
    })
}

fn structure_label_snapshot_json(
    conn: &Connection,
    board_id: &str,
    label: &LabelSnapshot,
) -> Result<JsonValue> {
    let parts = load_semantics_parts(conn, board_id, &label.id)?;
    Ok(json!({
        "id": &label.id,
        "name": &label.name,
        "semantics_hash": semantics_hash(label, &parts)?,
        "semantics": semantics_json(label, &parts),
        "task_binding_count": structure_label_task_binding_count(conn, board_id, &label.id)?,
    }))
}

fn structure_label_task_binding_count(
    conn: &Connection,
    board_id: &str,
    label_id: &str,
) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM task_labels WHERE board_id=?1 AND label_id=?2",
        params![board_id, label_id],
        |row| row.get(0),
    )
    .map_err(storage)
}

fn structure_source_signal_json(signal: &LabelOntologySignalRecord) -> JsonValue {
    json!({
        "id": &signal.id,
        "kind": signal.kind.to_string(),
        "status": signal.status.to_string(),
        "proposed_action": signal.proposed_action.to_string(),
        "target_label_id": &signal.target_label_id,
        "target_label_name_snapshot": &signal.target_label_name_snapshot,
        "related_labels": serde_json::from_str::<JsonValue>(&signal.related_labels_json)
            .unwrap_or(JsonValue::Null),
        "proposed_label_name": &signal.proposed_label_name,
        "proposed_label_name_normalized": &signal.proposed_label_name_normalized,
        "proposal": serde_json::from_str::<JsonValue>(&signal.proposal_json).unwrap_or(JsonValue::Null),
    })
}

fn normalize_retarget_reason(
    allow_retarget: bool,
    reason: Option<String>,
    context: &str,
) -> Result<Option<String>> {
    let reason = normalize_optional_text(reason)?;
    match (allow_retarget, reason) {
        (true, Some(reason)) => Ok(Some(reason)),
        (true, None) => Err(KanbanError::InvalidInput(format!(
            "{context} retarget override requires retarget_reason"
        ))),
        (false, Some(_)) => Err(KanbanError::InvalidInput(format!(
            "{context} retarget_reason requires allow_retarget"
        ))),
        (false, None) => Ok(None),
    }
}

fn atom_apply_retarget_override(
    signals: &[LabelOntologySignalRecord],
    label: &LabelSnapshot,
    retarget_reason: Option<&str>,
) -> Result<JsonValue> {
    let mismatched = signals
        .iter()
        .filter(|signal| {
            signal
                .target_label_id
                .as_deref()
                .is_some_and(|target_label_id| target_label_id != label.id)
        })
        .collect::<Vec<_>>();
    if retarget_reason.is_none() && !mismatched.is_empty() {
        let ids = mismatched
            .iter()
            .map(|signal| signal.id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        return Err(KanbanError::InvalidInput(format!(
            "source signals do not target label {}: {ids}",
            label.name
        )));
    }
    if let Some(reason) = retarget_reason {
        return Ok(json!({
            "reason": reason,
            "signals": signals.iter().map(source_signal_retarget_json).collect::<Vec<_>>(),
            "target_label": {
                "id": &label.id,
                "name": &label.name,
            },
        }));
    }
    Ok(JsonValue::Null)
}

fn proposal_bootstrap_retarget_override(
    signals: &[LabelOntologySignalRecord],
    proposal: &LabelSemanticProposalRecord,
    result_label_id: &str,
    retarget_reason: Option<&str>,
) -> Result<JsonValue> {
    let proposal_name_normalized = normalize_label_name(&proposal.name)?;
    let invalid_sources = signals
        .iter()
        .filter(|signal| {
            signal.kind != LabelOntologySignalKind::VocabularyGap
                || signal.proposed_action != LabelOntologyProposedAction::BootstrapLabel
        })
        .collect::<Vec<_>>();
    if !invalid_sources.is_empty() {
        let ids = invalid_sources
            .iter()
            .map(|signal| signal.id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        return Err(KanbanError::InvalidInput(format!(
            "proposal source signals must be confirmed vocabulary_gap/bootstrap_label signals: {ids}"
        )));
    }
    let mismatched_label = signals
        .iter()
        .filter(|signal| {
            signal.proposed_label_name_normalized.as_deref()
                != Some(proposal_name_normalized.as_str())
        })
        .collect::<Vec<_>>();
    if retarget_reason.is_none() && !mismatched_label.is_empty() {
        let ids = mismatched_label
            .iter()
            .map(|signal| signal.id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        return Err(KanbanError::InvalidInput(format!(
            "source signals do not match proposal proposed label {}: {ids}",
            proposal.name
        )));
    }
    if let Some(reason) = retarget_reason {
        return Ok(json!({
            "reason": reason,
            "signals": signals.iter().map(source_signal_retarget_json).collect::<Vec<_>>(),
            "proposal": {
                "id": &proposal.id,
                "name": &proposal.name,
                "name_normalized": proposal_name_normalized,
            },
            "result_label": {
                "id": result_label_id,
                "name": &proposal.name,
            },
        }));
    }
    Ok(JsonValue::Null)
}

fn proposal_create_retarget_override(
    signals: &[LabelOntologySignalRecord],
    proposal: &LabelSemanticProposalRecord,
    retarget_reason: Option<&str>,
) -> Result<JsonValue> {
    let proposal_name_normalized = normalize_label_name(&proposal.name)?;
    let invalid_sources = signals
        .iter()
        .filter(|signal| {
            signal.kind != LabelOntologySignalKind::VocabularyGap
                || signal.proposed_action != LabelOntologyProposedAction::BootstrapLabel
        })
        .collect::<Vec<_>>();
    if !invalid_sources.is_empty() {
        let ids = invalid_sources
            .iter()
            .map(|signal| signal.id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        return Err(KanbanError::InvalidInput(format!(
            "proposal source signals must be confirmed vocabulary_gap/bootstrap_label signals: {ids}"
        )));
    }
    let mismatched_label = signals
        .iter()
        .filter(|signal| {
            signal.proposed_label_name_normalized.as_deref()
                != Some(proposal_name_normalized.as_str())
        })
        .collect::<Vec<_>>();
    if retarget_reason.is_none() && !mismatched_label.is_empty() {
        let ids = mismatched_label
            .iter()
            .map(|signal| signal.id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        return Err(KanbanError::InvalidInput(format!(
            "source signals do not match proposal proposed label {}: {ids}",
            proposal.name
        )));
    }
    if let Some(reason) = retarget_reason {
        return Ok(json!({
            "reason": reason,
            "signals": signals.iter().map(source_signal_retarget_json).collect::<Vec<_>>(),
            "proposal": {
                "id": &proposal.id,
                "name": &proposal.name,
                "name_normalized": proposal_name_normalized,
            },
        }));
    }
    Ok(JsonValue::Null)
}

fn source_signal_retarget_json(signal: &LabelOntologySignalRecord) -> JsonValue {
    json!({
        "id": &signal.id,
        "kind": signal.kind.to_string(),
        "proposed_action": signal.proposed_action.to_string(),
        "target_label_id": &signal.target_label_id,
        "target_label_name_snapshot": &signal.target_label_name_snapshot,
        "proposed_label_name": &signal.proposed_label_name,
        "proposed_label_name_normalized": &signal.proposed_label_name_normalized,
    })
}

fn validate_status_transition(
    action_type: LabelOntologyActionType,
    signals: &[LabelOntologySignalRecord],
) -> Result<()> {
    let allowed = match action_type {
        LabelOntologyActionType::Confirm => Some(&[LabelOntologySignalStatus::Open][..]),
        LabelOntologyActionType::Reject
        | LabelOntologyActionType::Supersede
        | LabelOntologyActionType::ResolveNoChange => Some(
            &[
                LabelOntologySignalStatus::Open,
                LabelOntologySignalStatus::Confirmed,
            ][..],
        ),
        LabelOntologyActionType::AddPositiveAtom
        | LabelOntologyActionType::AddNegativeAtom
        | LabelOntologyActionType::AdoptExistingAtom
        | LabelOntologyActionType::UpdateSemantics
        | LabelOntologyActionType::CreateLabelProposal
        | LabelOntologyActionType::BootstrapLabel => {
            Some(&[LabelOntologySignalStatus::Confirmed][..])
        }
        _ => None,
    };
    if let Some(allowed) = allowed {
        for signal in signals {
            if !allowed.contains(&signal.status) {
                return Err(KanbanError::InvalidTransition(format!(
                    "cannot apply {action_type} to signal {} in status {}",
                    signal.id, signal.status
                )));
            }
        }
    }
    Ok(())
}

fn ensure_generic_lifecycle_action_input(input: &LabelOntologyActionInput) -> Result<()> {
    if !matches!(
        input.action_type,
        LabelOntologyActionType::Confirm
            | LabelOntologyActionType::Reject
            | LabelOntologyActionType::Supersede
            | LabelOntologyActionType::ResolveNoChange
    ) {
        return Err(KanbanError::InvalidInput(
            "generic label ontology action endpoint only accepts lifecycle actions; use a dedicated canonical mutation endpoint"
                .into(),
        ));
    }

    if input.parent_action_id.is_some()
        || input.target_label_ref.is_some()
        || input.result_label_ref.is_some()
        || input.result_atom_id.is_some()
        || input.result_atom_content_hash.is_some()
        || input.result_proposal_id.is_some()
        || input.canonical_before_hash.is_some()
        || input.canonical_after_hash.is_some()
        || input.change_json.is_some()
        || input.validation_status.is_some()
        || input.validation_json.is_some()
    {
        return Err(KanbanError::InvalidInput(
            "generic label ontology action endpoint cannot set canonical mutation provenance fields; use a dedicated canonical mutation endpoint"
                .into(),
        ));
    }

    Ok(())
}

fn apply_status_transition(
    conn: &Connection,
    action_type: LabelOntologyActionType,
    signal_ids: &[String],
    superseded_by_signal_id: Option<&str>,
    reason: &str,
    now: i64,
) -> Result<()> {
    let Some(status) = status_for_action(action_type) else {
        return Ok(());
    };
    let closed_at = if matches!(status, LabelOntologySignalStatus::Confirmed) {
        None
    } else {
        Some(now)
    };
    for signal_id in signal_ids {
        exec(
            conn,
            "UPDATE label_ontology_signals \
             SET status=?1, status_reason=?2, superseded_by_signal_id=?3, updated_at=?4, reviewed_at=COALESCE(reviewed_at, ?4), closed_at=?5 \
             WHERE id=?6",
            params![
                status.to_string(),
                reason,
                superseded_by_signal_id,
                now,
                closed_at,
                signal_id,
            ],
        )?;
    }
    Ok(())
}

fn status_for_action(action_type: LabelOntologyActionType) -> Option<LabelOntologySignalStatus> {
    match action_type {
        LabelOntologyActionType::Confirm => Some(LabelOntologySignalStatus::Confirmed),
        LabelOntologyActionType::Reject => Some(LabelOntologySignalStatus::Rejected),
        LabelOntologyActionType::Supersede => Some(LabelOntologySignalStatus::Superseded),
        LabelOntologyActionType::ResolveNoChange => Some(LabelOntologySignalStatus::Resolved),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SemanticsParts {
    description: Option<String>,
    applies_when: Vec<String>,
    excludes_when: Vec<String>,
    positive_examples: Vec<String>,
    negative_examples: Vec<String>,
}

impl SemanticsParts {
    fn empty() -> Self {
        Self {
            description: None,
            applies_when: Vec::new(),
            excludes_when: Vec::new(),
            positive_examples: Vec::new(),
            negative_examples: Vec::new(),
        }
    }

    fn push_atom(&mut self, kind: &str, text: &str) {
        let target = match kind {
            "applies_when" => &mut self.applies_when,
            "positive_example" => &mut self.positive_examples,
            "excludes_when" => &mut self.excludes_when,
            "negative_example" => &mut self.negative_examples,
            _ => return,
        };
        if !target
            .iter()
            .any(|existing| normalize_atom_text(existing) == normalize_atom_text(text))
        {
            target.push(text.to_owned());
        }
    }
}

fn load_semantics_parts(
    conn: &Connection,
    board_id: &str,
    label_id: &str,
) -> Result<SemanticsParts> {
    optional(
        conn,
        "SELECT description,applies_when,excludes_when,positive_examples,negative_examples \
         FROM label_semantics WHERE board_id=?1 AND label_id=?2",
        params![board_id, label_id],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        },
    )?
    .map(
        |(description, applies_when, excludes_when, positive_examples, negative_examples)| {
            Ok(SemanticsParts {
                description,
                applies_when: json_vec(applies_when)?,
                excludes_when: json_vec(excludes_when)?,
                positive_examples: json_vec(positive_examples)?,
                negative_examples: json_vec(negative_examples)?,
            })
        },
    )
    .unwrap_or_else(|| Ok(SemanticsParts::empty()))
}

fn semantics_parts_from_json(value: &JsonValue, context: &str) -> Result<SemanticsParts> {
    let object = value.as_object().ok_or_else(|| {
        KanbanError::InvalidInput(format!("{context} semantics snapshot must be an object"))
    })?;
    let description = match object.get("description") {
        Some(JsonValue::Null) | None => None,
        Some(JsonValue::String(value)) => Some(value.clone()),
        Some(_) => {
            return Err(KanbanError::InvalidInput(format!(
                "{context} description must be a string or null"
            )));
        }
    };
    Ok(SemanticsParts {
        description,
        applies_when: required_string_array_field(object, "applies_when", context)?,
        excludes_when: required_string_array_field(object, "excludes_when", context)?,
        positive_examples: required_string_array_field(object, "positive_examples", context)?,
        negative_examples: required_string_array_field(object, "negative_examples", context)?,
    })
}

fn required_string_array_field(
    object: &serde_json::Map<String, JsonValue>,
    field: &str,
    context: &str,
) -> Result<Vec<String>> {
    let value = object.get(field).ok_or_else(|| {
        KanbanError::InvalidInput(format!("{context} semantics snapshot missing {field}"))
    })?;
    serde_json::from_value(value.clone()).map_err(|err| {
        KanbanError::InvalidInput(format!("{context} {field} must be a string array: {err}"))
    })
}

fn semantics_json(label: &LabelSnapshot, parts: &SemanticsParts) -> JsonValue {
    json!({
        "label_id": &label.id,
        "label_name": &label.name,
        "description": &parts.description,
        "applies_when": &parts.applies_when,
        "excludes_when": &parts.excludes_when,
        "positive_examples": &parts.positive_examples,
        "negative_examples": &parts.negative_examples,
    })
}

fn semantics_hash(label: &LabelSnapshot, parts: &SemanticsParts) -> Result<String> {
    let snapshot = serde_json::to_string(&semantics_json(label, parts))
        .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;
    Ok(stable_hash(&snapshot))
}

fn polarity_for_atom_kind(kind: &str) -> Result<&'static str> {
    match kind {
        "applies_when" | "positive_example" => Ok("positive"),
        "excludes_when" | "negative_example" => Ok("negative"),
        _ => Err(KanbanError::InvalidInput(
            "candidate atom kind is invalid".into(),
        )),
    }
}

fn json_vec(json: String) -> Result<Vec<String>> {
    serde_json::from_str(&json).map_err(|err| KanbanError::Storage(err.to_string()))
}

const OBSERVATION_COLUMNS: &str = "id,board_id,task_id,task_ref_snapshot,task_snapshot_json,suggest_input_hash,agent_candidates_json,suggestion_snapshot_json,final_decision_json,suggest_coverage,suggest_coverage_cosine,suggest_residual_norm,suggest_needs_new_label,suggest_degraded,diagnostics_json,capture_fingerprint,created_by,created_by_type,agent_type,created_at";

fn observation_from_row(row: &Row<'_>) -> rusqlite::Result<LabelOntologyObservationRecord> {
    Ok(LabelOntologyObservationRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        task_id: row.get(2)?,
        task_ref_snapshot: row.get(3)?,
        task_snapshot_json: row.get(4)?,
        suggest_input_hash: row.get(5)?,
        agent_candidates_json: row.get(6)?,
        suggestion_snapshot_json: row.get(7)?,
        final_decision_json: row.get(8)?,
        suggest_coverage: row.get(9)?,
        suggest_coverage_cosine: row.get(10)?,
        suggest_residual_norm: row.get(11)?,
        suggest_needs_new_label: int_bool(row.get(12)?),
        suggest_degraded: int_bool(row.get(13)?),
        diagnostics_json: row.get(14)?,
        capture_fingerprint: row.get(15)?,
        created_by: row.get(16)?,
        created_by_type: row.get(17)?,
        agent_type: row.get(18)?,
        created_at: row.get(19)?,
        signals: Vec::new(),
    })
}

const SIGNAL_COLUMNS: &str = "s.id,s.observation_id,s.board_id,s.kind,s.status,s.target_label_id,s.target_label_name_snapshot,s.related_labels_json,s.proposed_action,s.candidate_atom_polarity,s.candidate_atom_kind,s.candidate_text,s.candidate_content_hash,s.proposed_label_name,s.proposed_label_name_normalized,s.proposal_json,s.agent_selected,s.suggest_state,s.suggest_score,s.suggest_rank,s.final_selected,s.rationale,s.confidence,s.signal_key,s.superseded_by_signal_id,s.status_reason,s.created_at,s.updated_at,s.reviewed_at,s.closed_at";

fn signal_from_row(row: &Row<'_>) -> rusqlite::Result<LabelOntologySignalRecord> {
    let kind: String = row.get(3)?;
    let status: String = row.get(4)?;
    let proposed_action: String = row.get(8)?;
    let suggest_state: Option<String> = row.get(17)?;
    Ok(LabelOntologySignalRecord {
        id: row.get(0)?,
        observation_id: row.get(1)?,
        board_id: row.get(2)?,
        kind: parse_row_enum(&kind)?,
        status: parse_row_enum(&status)?,
        target_label_id: row.get(5)?,
        target_label_name_snapshot: row.get(6)?,
        related_labels_json: row.get(7)?,
        proposed_action: parse_row_enum(&proposed_action)?,
        candidate_atom_polarity: row.get(9)?,
        candidate_atom_kind: row.get(10)?,
        candidate_text: row.get(11)?,
        candidate_content_hash: row.get(12)?,
        proposed_label_name: row.get(13)?,
        proposed_label_name_normalized: row.get(14)?,
        proposal_json: row.get(15)?,
        agent_selected: int_bool(row.get(16)?),
        suggest_state: suggest_state.as_deref().map(parse_row_enum).transpose()?,
        suggest_score: row.get(18)?,
        suggest_rank: row.get(19)?,
        final_selected: int_bool(row.get(20)?),
        rationale: row.get(21)?,
        confidence: row.get(22)?,
        signal_key: row.get(23)?,
        superseded_by_signal_id: row.get(24)?,
        status_reason: row.get(25)?,
        created_at: row.get(26)?,
        updated_at: row.get(27)?,
        reviewed_at: row.get(28)?,
        closed_at: row.get(29)?,
    })
}

const ACTION_COLUMNS: &str = "a.id,a.board_id,a.parent_action_id,a.action_type,a.reason,a.target_label_id,a.result_label_id,a.result_atom_id,a.result_atom_content_hash,a.result_proposal_id,a.canonical_before_hash,a.canonical_after_hash,a.change_json,a.validation_status,a.validation_json,a.created_by,a.created_by_type,a.agent_type,a.created_at";

fn action_from_row(row: &Row<'_>) -> rusqlite::Result<LabelOntologyActionRecord> {
    let action_type: String = row.get(3)?;
    let validation_status: String = row.get(13)?;
    Ok(LabelOntologyActionRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        parent_action_id: row.get(2)?,
        action_type: parse_row_enum(&action_type)?,
        reason: row.get(4)?,
        target_label_id: row.get(5)?,
        result_label_id: row.get(6)?,
        result_atom_id: row.get(7)?,
        result_atom_content_hash: row.get(8)?,
        result_proposal_id: row.get(9)?,
        canonical_before_hash: row.get(10)?,
        canonical_after_hash: row.get(11)?,
        change_json: row.get(12)?,
        validation_status: parse_row_enum(&validation_status)?,
        validation_json: row.get(14)?,
        created_by: row.get(15)?,
        created_by_type: row.get(16)?,
        agent_type: row.get(17)?,
        created_at: row.get(18)?,
        signal_ids: Vec::new(),
    })
}

fn parse_row_enum<T>(value: &str) -> rusqlite::Result<T>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    value
        .parse()
        .map_err(|err: T::Err| rusqlite::Error::InvalidParameterName(err.to_string()))
}

struct LabelSnapshot {
    id: String,
    name: String,
}

fn resolve_label(conn: &Connection, board_id: &str, label_ref: &str) -> Result<LabelSnapshot> {
    let label_ref = normalize_required_text(label_ref)?;
    let by_name = optional(
        conn,
        "SELECT id,name FROM labels WHERE board_id=?1 AND name=?2",
        params![board_id, label_ref],
        |row| {
            Ok(LabelSnapshot {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        },
    )?;
    if let Some(label) = by_name {
        return Ok(label);
    }
    if label_ref.starts_with("l_") {
        return required_row(
            conn,
            "SELECT id,name FROM labels WHERE board_id=?1 AND id=?2",
            params![board_id, label_ref],
            |row| {
                Ok(LabelSnapshot {
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            },
            || KanbanError::NotFound(format!("label {label_ref}")),
        );
    }
    Err(KanbanError::NotFound(format!("label {label_ref}")))
}

fn bool_int(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

fn int_bool(value: i64) -> bool {
    value != 0
}

fn normalize_atom_text(text: &str) -> String {
    text.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn stable_hash(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
