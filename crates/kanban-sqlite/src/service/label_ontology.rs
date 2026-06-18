use crate::connect_file;

use super::{
    LabelOntologyActionInput, LabelOntologyActionRecord, LabelOntologyActionType,
    LabelOntologyActor, LabelOntologyAtomApplyInput, LabelOntologyCandidateAtomInput,
    LabelOntologyObservationRecord, LabelOntologyRecordInput, LabelOntologySignalDetail,
    LabelOntologySignalInput, LabelOntologySignalListOptions, LabelOntologySignalRecord,
    LabelOntologySignalStatus, LabelOntologyValidationInput, LabelOntologyValidationStatus,
    LabelSemanticProposalRecord, all_values, board_id, exec, get_task_by_id,
    mark_label_atom_store_dirty, optional, required_row, resolve_task, storage,
    upsert_label_semantics_in_tx, with_immediate_tx, with_read_tx,
};

use std::{path::Path, str::FromStr};

use kanban_core::{Clock, KanbanError, Result, SystemClock, new_typed_id};
use kanban_labels::LabelDefinition;
use rusqlite::{Connection, Row, params, types::Value};
use serde_json::{Value as JsonValue, json};

const LABEL_ONTOLOGY_LIST_LIMIT_MAX: usize = 1000;

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
             id, board_id, task_id, task_ref_snapshot, task_snapshot_json, agent_candidates_json, \
             suggestion_snapshot_json, final_decision_json, suggest_coverage, suggest_coverage_cosine, \
             suggest_residual_norm, suggest_needs_new_label, suggest_degraded, diagnostics_json, \
             capture_fingerprint, created_by, created_by_type, agent_type, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            params![
                observation_id,
                board_id,
                task.id,
                task.task_ref,
                task_snapshot_json,
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

pub fn apply_label_ontology_atom(
    path: impl AsRef<Path>,
    board: &str,
    input: LabelOntologyAtomApplyInput,
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
        let atom = normalize_candidate_atom(&LabelOntologyCandidateAtomInput {
            polarity: polarity_for_atom_kind(&input.kind)?.to_owned(),
            kind: input.kind,
            text: input.text,
        })?;
        let action_type = match atom.polarity.as_str() {
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
        if after_hash != before_hash {
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
            "changed": before_hash != after_hash,
            "before": semantics_json(&label, &before),
            "after": semantics_json(&label, &after),
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
                validation_status: LabelOntologyValidationStatus::Pending,
                validation_json: "{}".to_owned(),
            },
            now,
        )?;
        link_action_signals(&conn, &board_id, &action_id, &signal_ids, now)?;
        action_by_id_with_links(&conn, &action_id)
    })
}

pub fn validate_label_ontology_action(
    path: impl AsRef<Path>,
    board: &str,
    input: LabelOntologyValidationInput,
) -> Result<LabelOntologyActionRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let actor = normalize_actor(input.actor)?;
        let parent_action_id = normalize_required_text(&input.parent_action_id)?;
        let parent_action = action_by_id_with_links(&conn, &parent_action_id)?;
        if parent_action.board_id != board_id {
            return Err(KanbanError::InvalidInput(
                "parent action belongs to a different board".into(),
            ));
        }
        ensure_validatable_parent_action(&parent_action)?;
        if matches!(
            input.validation_status,
            LabelOntologyValidationStatus::Pending
        ) {
            return Err(KanbanError::InvalidInput(
                "validation action cannot record pending status".into(),
            ));
        }
        let explicit_signal_ids = !input.signal_ids.is_empty();
        let signal_ids = if explicit_signal_ids {
            normalize_signal_ids(input.signal_ids)?
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
            .map(|signal_id| signal_by_id(&conn, signal_id))
            .collect::<Result<Vec<_>>>()?;
        ensure_signals_on_board_and_status(
            &signals,
            &board_id,
            &[LabelOntologySignalStatus::Confirmed],
        )?;
        let validation_json = build_validation_json(
            &conn,
            &parent_action,
            &signals,
            &input.validation_json,
            input.validation_status,
        )?;
        let action_id = insert_ontology_action(
            &conn,
            &board_id,
            InsertOntologyAction {
                action_type: LabelOntologyActionType::Validate,
                reason: normalize_required_text(&input.reason)?,
                actor,
                parent_action_id: Some(parent_action_id),
                target_label_id: parent_action.target_label_id,
                result_label_id: parent_action.result_label_id,
                result_atom_id: parent_action.result_atom_id,
                result_atom_content_hash: parent_action.result_atom_content_hash,
                result_proposal_id: parent_action.result_proposal_id,
                canonical_before_hash: parent_action.canonical_before_hash,
                canonical_after_hash: parent_action.canonical_after_hash,
                change_json: "{}".to_owned(),
                validation_status: input.validation_status,
                validation_json,
            },
            now,
        )?;
        link_action_signals(&conn, &board_id, &action_id, &signal_ids, now)?;
        if matches!(
            input.validation_status,
            LabelOntologyValidationStatus::Passed
        ) {
            apply_status_transition(
                &conn,
                LabelOntologyActionType::ResolveNoChange,
                &signal_ids,
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
    actor: &str,
    reason: Option<&str>,
    source_signal_ids: Vec<String>,
    now: i64,
) -> Result<Option<String>> {
    if source_signal_ids.is_empty() {
        return Ok(None);
    }
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
    let label = LabelSnapshot {
        id: result_label_id.to_owned(),
        name: proposal.name.clone(),
    };
    let after = load_semantics_parts(conn, &proposal.board_id, result_label_id)?;
    let after_hash = semantics_hash(&label, &after)?;
    let reason = normalize_optional_text(reason.map(ToOwned::to_owned))?
        .unwrap_or_else(|| "accepted label proposal from ontology signals".to_owned());
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
        "semantics": semantics_json(&label, &after),
    }))
    .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;
    let action_id = insert_ontology_action(
        conn,
        &proposal.board_id,
        InsertOntologyAction {
            action_type: LabelOntologyActionType::BootstrapLabel,
            reason,
            actor: LabelOntologyActor {
                name: normalize_required_text(actor)?,
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            parent_action_id: None,
            target_label_id: None,
            result_label_id: Some(result_label_id.to_owned()),
            result_atom_id: None,
            result_atom_content_hash: None,
            result_proposal_id: Some(proposal.id.clone()),
            canonical_before_hash: None,
            canonical_after_hash: Some(after_hash),
            change_json,
            validation_status: LabelOntologyValidationStatus::Pending,
            validation_json: "{}".to_owned(),
        },
        now,
    )?;
    link_action_signals(conn, &proposal.board_id, &action_id, &signal_ids, now)?;
    Ok(Some(action_id))
}

fn build_validation_json(
    conn: &Connection,
    parent_action: &LabelOntologyActionRecord,
    signals: &[LabelOntologySignalRecord],
    supplied_json: &str,
    status: LabelOntologyValidationStatus,
) -> Result<String> {
    let manual = parse_json_field(supplied_json, "validation_json", JsonShape::Object)?;
    ensure_passed_validation_evidence(&manual, signals, status)?;
    let mut cases = Vec::with_capacity(signals.len());
    let mut stale_count = 0usize;
    let mut degraded_count = 0usize;
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
        let stale = captured_hash != current_hash;
        if stale {
            stale_count += 1;
        }
        if observation.suggest_degraded {
            degraded_count += 1;
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
                "manual": &manual,
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
            "incomparable_count": stale_count + degraded_count,
        }
    }))
    .map_err(|err| KanbanError::InvalidInput(err.to_string()))
}

fn ensure_validatable_parent_action(action: &LabelOntologyActionRecord) -> Result<()> {
    if !matches!(
        action.action_type,
        LabelOntologyActionType::AddPositiveAtom
            | LabelOntologyActionType::AddNegativeAtom
            | LabelOntologyActionType::UpdateSemantics
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
    signals: &[LabelOntologySignalRecord],
    status: LabelOntologyValidationStatus,
) -> Result<()> {
    if !matches!(status, LabelOntologyValidationStatus::Passed) {
        return Ok(());
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
    }
    Ok(())
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
    let related_labels_json = normalize_json_field(
        &input.related_labels_json,
        "related_labels_json",
        JsonShape::Array,
    )?;
    let proposal_json =
        normalize_json_field(&input.proposal_json, "proposal_json", JsonShape::Object)?;
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
    if let Some(confidence) = input.confidence
        && !(0.0..=1.0).contains(&confidence)
    {
        return Err(KanbanError::InvalidInput(
            "signal confidence must be between 0.0 and 1.0".into(),
        ));
    }

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
    Ok(LabelOntologyActor {
        name,
        actor_type,
        agent_type,
    })
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

const OBSERVATION_COLUMNS: &str = "id,board_id,task_id,task_ref_snapshot,task_snapshot_json,agent_candidates_json,suggestion_snapshot_json,final_decision_json,suggest_coverage,suggest_coverage_cosine,suggest_residual_norm,suggest_needs_new_label,suggest_degraded,diagnostics_json,capture_fingerprint,created_by,created_by_type,agent_type,created_at";

fn observation_from_row(row: &Row<'_>) -> rusqlite::Result<LabelOntologyObservationRecord> {
    Ok(LabelOntologyObservationRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        task_id: row.get(2)?,
        task_ref_snapshot: row.get(3)?,
        task_snapshot_json: row.get(4)?,
        agent_candidates_json: row.get(5)?,
        suggestion_snapshot_json: row.get(6)?,
        final_decision_json: row.get(7)?,
        suggest_coverage: row.get(8)?,
        suggest_coverage_cosine: row.get(9)?,
        suggest_residual_norm: row.get(10)?,
        suggest_needs_new_label: int_bool(row.get(11)?),
        suggest_degraded: int_bool(row.get(12)?),
        diagnostics_json: row.get(13)?,
        capture_fingerprint: row.get(14)?,
        created_by: row.get(15)?,
        created_by_type: row.get(16)?,
        agent_type: row.get(17)?,
        created_at: row.get(18)?,
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
