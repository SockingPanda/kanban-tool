use crate::connect_file;

use super::{
    AddTaskLabelsResult, BootstrapTaskLabel, BootstrapTaskLabelResult, CreateLabel, CreateTask,
    DeleteLabelResult, LabelOntologyActionType, LabelOntologyProposalBootstrapOptions,
    LabelOntologySemanticsMutationInput, LabelRecord, LabelSemanticProposalRecord,
    LabelSemanticsMutationOptions, LabelSemanticsRecord, MAX_TASK_LIST_LIMIT, StepPlanState,
    TaskListOptions, TaskListPage, TaskListSort, TaskPatch, TaskPlanFilter, TaskRecord,
    add_dependency_in_current_tx, all, all_values, board_id, board_id_any, ensure_changed_one,
    exec, exec_named, exec_one_named, insert_event, json_valid,
    label_ontology_semantics_snapshot_in_tx, mark_label_atom_store_dirty, optional,
    recompute_ready_status, record_label_ontology_proposal_bootstrap_in_tx,
    record_label_ontology_semantics_mutation_in_tx, required_row, scalar,
    upsert_label_semantics_candidate_in_tx, validate_priority, with_immediate_tx,
};
use super::{
    BootstrapTaskLabelVerification, BootstrapTaskLabelVerifiedResult,
    DEFAULT_LABEL_SUGGESTION_ATOM_LIMIT, DEFAULT_LABEL_SUGGESTION_CANDIDATE_LIMIT,
    DEFAULT_LABEL_SUGGESTION_MAX_SELECTED_LABELS, LabelSuggestionOptions,
    label_atom_vectors_for_board, label_atom_vectors_for_definition, suggest_task_labels_with,
    vector_storage, with_read_tx,
};

use std::path::Path;

use kanban_core::new_label_id;
use kanban_core::{
    Clock, KanbanError, ReadinessFacts, Result, SystemClock, TaskStatus,
    initial_status as core_initial_status, is_active_recomputable_status, new_task_id,
};
use kanban_labels::LabelDefinition;
use kanban_vector::{
    LabelAtomHit, LabelAtomVector, LabelAtomVectorHit, LabelAtomVectorQuery, LabelAtomVectorStore,
    VectorError, VectorStoreBackend, VectorStoreStatus,
};

use rusqlite::{Connection, Row, named_params, params, types::Value};

use serde_json::json;

pub fn create_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    input: CreateTask,
) -> Result<TaskRecord> {
    create_task_with_dependencies(path, board, actor, input, &[])
}

pub fn create_task_with_dependencies(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    input: CreateTask,
    depends_on: &[String],
) -> Result<TaskRecord> {
    create_task_with_labels_and_dependencies(path, board, actor, input, &[], depends_on)
}

pub fn create_task_with_labels(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    input: CreateTask,
    labels: &[String],
) -> Result<TaskRecord> {
    create_task_with_labels_and_dependencies(path, board, actor, input, labels, &[])
}

pub fn create_task_with_labels_and_dependencies(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    input: CreateTask,
    labels: &[String],
    depends_on: &[String],
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    validate_retry_policy(input.max_retries)?;
    validate_priority(input.priority)?;
    let labels = normalize_label_names(labels)?;
    let title = input.title.trim().to_owned();
    if title.is_empty() {
        return Err(KanbanError::InvalidInput("title is required".into()));
    }
    if !json_valid(&conn, &input.metadata_json)? {
        return Err(KanbanError::InvalidInput(
            "metadata_json must be valid JSON".into(),
        ));
    }
    let mut status = core_initial_status(
        input.status,
        ReadinessFacts {
            title: &title,
            description: input.description.as_deref(),
            scheduled_at: input.scheduled_at,
            dependencies_done: true,
        },
        now,
    )?;
    if status == TaskStatus::Ready {
        status = TaskStatus::Todo;
    }
    let id = new_task_id();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let resolved_labels = labels
            .iter()
            .map(|label| {
                label_for_task_binding_in_current_tx(&conn, &board_id, label, false, now)
                    .map(|(label, _created)| label)
            })
            .collect::<Result<Vec<_>>>()?;
        let seq: i64 = scalar(
            &conn,
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM tasks WHERE board_id=:board_id",
            named_params! { ":board_id": board_id },
            |r| r.get(0),
        )?;
        exec_named(
            &conn,
            "INSERT INTO tasks(id, board_id, seq, title, description, status, assignee, priority, position, scheduled_at, due_at, created_by, created_at, updated_at, metadata_json) \
             VALUES (:id, :board_id, :seq, :title, :description, :status, :assignee, :priority, :seq * 1024, :scheduled_at, :due_at, :created_by, :now, :now, :metadata_json)",
            named_params! {
                ":id": id,
                ":board_id": board_id,
                ":seq": seq,
                ":title": title,
                ":description": input.description,
                ":status": status.as_str(),
                ":assignee": input.assignee,
                ":priority": input.priority,
                ":scheduled_at": input.scheduled_at,
                ":due_at": input.due_at,
                ":created_by": actor,
                ":now": now,
                ":metadata_json": input.metadata_json,
            },
        )?;
        let payload = json!({ "status": status.as_str() }).to_string();
        insert_event(
            &conn,
            &board_id,
            Some(&id),
            None,
            "task.created",
            actor,
            &payload,
            now,
        )?;
        if input.max_retries.is_some() {
            update_retry_policy_in_current_tx(
                &conn,
                &board_id,
                actor,
                &id,
                input.max_retries,
                now,
            )?;
        }
        for parent_ref in depends_on {
            let parent = resolve_task(&conn, &board_id, parent_ref)?;
            let child = get_task_by_id(&conn, &board_id, &id)?;
            add_dependency_in_current_tx(&conn, &board_id, actor, &parent, &child, now)?;
        }
        for label in &resolved_labels {
            attach_label_in_current_tx(&conn, &board_id, actor, &id, &label.id, now)?;
        }
        get_task_by_id(&conn, &board_id, &id)
    })
}

pub fn create_label(
    path: impl AsRef<Path>,
    board: &str,
    input: CreateLabel,
) -> Result<LabelRecord> {
    create_label_with_actor(path, board, "system", input)
}

pub fn create_label_with_actor(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    input: CreateLabel,
) -> Result<LabelRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let name = normalize_label_name(&input.name)?;
        if let Some(existing) = label_by_name(&conn, &board_id, &name)? {
            return Ok(existing);
        }
        let label =
            ensure_label_in_current_tx(&conn, &board_id, &name, input.color.as_deref(), now)?;
        insert_event(
            &conn,
            &board_id,
            None,
            None,
            "label.created",
            actor,
            &json!({
                "label_id": label.id,
                "label": label.name,
                "color": label.color,
            })
            .to_string(),
            now,
        )?;
        Ok(label)
    })
}

pub fn list_labels(path: impl AsRef<Path>, board: &str) -> Result<Vec<LabelRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    list_labels_conn(&conn, &board_id)
}

pub fn list_task_labels(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: &str,
) -> Result<Vec<LabelRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    task_labels_conn(&conn, &board_id, &task.id)
}

pub fn delete_label(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    label_ref: &str,
    force: bool,
) -> Result<DeleteLabelResult> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let label = resolve_label(&conn, &board_id, label_ref)?;
        let removed_task_bindings = count_task_label_bindings(&conn, &board_id, &label.id)?;
        if removed_task_bindings > 0 && !force {
            return Err(KanbanError::InvalidInput(format!(
                "label {} is attached to {} task(s); pass --force to delete it",
                label.name, removed_task_bindings
            )));
        }
        let removed_semantics = count_label_semantics(&conn, &board_id, &label.id)? > 0;
        let removed_atoms = count_label_atoms(&conn, &board_id, &label.id)?;
        if removed_semantics || removed_atoms > 0 {
            return Err(KanbanError::InvalidInput(format!(
                "label {} has semantics or atoms; clear semantics with reason and expected hash before deleting identity",
                label.name
            )));
        }
        if removed_task_bindings > 0 {
            exec(
                &conn,
                "DELETE FROM task_labels WHERE board_id=?1 AND label_id=?2",
                params![board_id, label.id],
            )?;
        }
        let changed = exec(
            &conn,
            "DELETE FROM labels WHERE board_id=?1 AND id=?2",
            params![board_id, label.id],
        )?;
        ensure_changed_one(changed, || {
            KanbanError::NotFound(format!("label {}", label.id))
        })?;
        insert_event(
            &conn,
            &board_id,
            None,
            None,
            "label.deleted",
            actor,
            &json!({
                "label_id": label.id,
                "label": label.name,
                "forced": force,
                "removed_task_bindings": removed_task_bindings,
                "removed_semantics": false,
                "removed_atoms": 0
            })
            .to_string(),
            now,
        )?;
        Ok(DeleteLabelResult {
            label,
            forced: force,
            removed_task_bindings,
            removed_semantics: false,
            removed_atoms: 0,
        })
    })
}

pub fn add_task_label(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    label_name: &str,
) -> Result<TaskRecord> {
    add_task_labels(path, board, actor, task_ref, &[label_name.to_owned()])
}

pub fn add_task_labels(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    label_names: &[String],
) -> Result<TaskRecord> {
    add_task_labels_with_options(path, board, actor, task_ref, label_names, false)
        .map(|result| result.task)
}

pub fn add_task_labels_with_options(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    label_names: &[String],
    create_missing: bool,
) -> Result<AddTaskLabelsResult> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let label_names = normalize_label_names(label_names)?;
    if label_names.is_empty() {
        return Err(KanbanError::InvalidInput("label name is required".into()));
    }
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let task = resolve_task(&conn, &board_id, task_ref)?;
        ensure_task_allows_label_mutation(&conn, &task.id)?;
        let mut created_labels = Vec::new();
        for label_name in &label_names {
            let (label, created) = label_for_task_binding_in_current_tx(
                &conn,
                &task.board_id,
                label_name,
                create_missing,
                now,
            )?;
            if created {
                created_labels.push(label.clone());
            }
            attach_label_in_current_tx(&conn, &task.board_id, actor, &task.id, &label.id, now)?;
        }
        let task = get_task_by_id(&conn, &task.board_id, &task.id)?;
        Ok(AddTaskLabelsResult {
            task,
            created_labels,
        })
    })
}

pub fn add_task_label_by_id(
    path: impl AsRef<Path>,
    actor: &str,
    task_id: &str,
    label_name: &str,
) -> Result<TaskRecord> {
    add_task_labels_by_id(path, actor, task_id, &[label_name.to_owned()])
}

pub fn add_task_labels_by_id(
    path: impl AsRef<Path>,
    actor: &str,
    task_id: &str,
    label_names: &[String],
) -> Result<TaskRecord> {
    add_task_labels_by_id_with_options(path, actor, task_id, label_names, false)
        .map(|result| result.task)
}

pub fn add_task_labels_by_id_with_options(
    path: impl AsRef<Path>,
    actor: &str,
    task_id: &str,
    label_names: &[String],
    create_missing: bool,
) -> Result<AddTaskLabelsResult> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let label_names = normalize_label_names(label_names)?;
    if label_names.is_empty() {
        return Err(KanbanError::InvalidInput("label name is required".into()));
    }
    with_immediate_tx(&conn, || {
        let board_id = active_board_id_for_label_mutation(&conn, task_id)?;
        let task = get_task_by_id(&conn, &board_id, task_id)?;
        let mut created_labels = Vec::new();
        for label_name in &label_names {
            let (label, created) = label_for_task_binding_in_current_tx(
                &conn,
                &task.board_id,
                label_name,
                create_missing,
                now,
            )?;
            if created {
                created_labels.push(label.clone());
            }
            attach_label_in_current_tx(&conn, &task.board_id, actor, &task.id, &label.id, now)?;
        }
        let task = get_task_by_id(&conn, &task.board_id, &task.id)?;
        Ok(AddTaskLabelsResult {
            task,
            created_labels,
        })
    })
}

pub fn bootstrap_task_label(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    input: BootstrapTaskLabel,
) -> Result<BootstrapTaskLabelResult> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let candidate = bootstrap_label_candidate(input)?;
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let task = resolve_task(&conn, &board_id, task_ref)?;
        ensure_task_allows_label_mutation(&conn, &task.id)?;
        let adoption = adopt_label_semantics_candidate_in_current_tx(
            &conn,
            &task.board_id,
            &candidate,
            LabelAdoptionProvenance::DirectBootstrap { actor },
            now,
        )?;
        attach_label_in_current_tx(
            &conn,
            &task.board_id,
            actor,
            &task.id,
            &adoption.label.id,
            now,
        )?;
        Ok(BootstrapTaskLabelResult {
            task: get_task_by_id(&conn, &task.board_id, &task.id)?,
            semantics: adoption.semantics,
        })
    })
}

pub fn bootstrap_task_label_by_id(
    path: impl AsRef<Path>,
    actor: &str,
    task_id: &str,
    input: BootstrapTaskLabel,
) -> Result<BootstrapTaskLabelResult> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let candidate = bootstrap_label_candidate(input)?;
    with_immediate_tx(&conn, || {
        let board_id = active_board_id_for_label_mutation(&conn, task_id)?;
        let task = get_task_by_id(&conn, &board_id, task_id)?;
        let adoption = adopt_label_semantics_candidate_in_current_tx(
            &conn,
            &task.board_id,
            &candidate,
            LabelAdoptionProvenance::DirectBootstrap { actor },
            now,
        )?;
        attach_label_in_current_tx(
            &conn,
            &task.board_id,
            actor,
            &task.id,
            &adoption.label.id,
            now,
        )?;
        Ok(BootstrapTaskLabelResult {
            task: get_task_by_id(&conn, &task.board_id, &task.id)?,
            semantics: adoption.semantics,
        })
    })
}

pub fn bootstrap_task_label_with_staged_verification(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    input: BootstrapTaskLabel,
    store: &(impl LabelAtomVectorStore + ?Sized),
    min_score: f32,
) -> Result<BootstrapTaskLabelVerifiedResult> {
    bootstrap_task_label_with_staged_verification_impl(
        BootstrapTaskLabelStagedVerificationArgs {
            path: path.as_ref(),
            board,
            actor,
            task_ref,
            input,
            store,
            min_score,
        },
        |_| Ok(()),
    )
}

struct BootstrapTaskLabelStagedVerificationArgs<'a, S: LabelAtomVectorStore + ?Sized> {
    path: &'a Path,
    board: &'a str,
    actor: &'a str,
    task_ref: &'a str,
    input: BootstrapTaskLabel,
    store: &'a S,
    min_score: f32,
}

fn bootstrap_task_label_with_staged_verification_impl<S: LabelAtomVectorStore + ?Sized>(
    args: BootstrapTaskLabelStagedVerificationArgs<'_, S>,
    mut stage_hook: impl FnMut(BootstrapStagedVerificationStage) -> Result<()>,
) -> Result<BootstrapTaskLabelVerifiedResult> {
    let BootstrapTaskLabelStagedVerificationArgs {
        path,
        board,
        actor,
        task_ref,
        input,
        store,
        min_score,
    } = args;
    if !(0.0..=1.0).contains(&min_score) {
        return Err(KanbanError::InvalidInput(
            "min_verify_score must be between 0 and 1".to_owned(),
        ));
    }
    let conn = connect_file(path)?;
    let now = SystemClock.now_ms();
    let candidate = bootstrap_label_candidate(input)?;
    let preflight = with_read_tx(&conn, || {
        bootstrap_staged_preflight(
            &conn,
            board,
            task_ref,
            &candidate,
            store.embedding_model(),
            now,
        )
    })?;
    reach_bootstrap_staged_verification_stage(
        BootstrapStagedVerificationStage::BeforeVerify,
        &mut stage_hook,
    )?;
    let staged_store = StagedBootstrapLabelAtomStore::new(store, preflight.atoms)?;
    let suggestions = suggest_task_labels_with(
        path,
        board,
        task_ref,
        &staged_store,
        LabelSuggestionOptions {
            output_limit: MAX_TASK_LIST_LIMIT,
            candidate_limit: DEFAULT_LABEL_SUGGESTION_CANDIDATE_LIMIT,
            atom_limit: DEFAULT_LABEL_SUGGESTION_ATOM_LIMIT,
            max_selected_labels: DEFAULT_LABEL_SUGGESTION_MAX_SELECTED_LABELS,
            min_score: 0.0,
        },
    )?;
    if suggestions.degraded {
        return Err(KanbanError::InvalidInput(format!(
            "label bootstrap verification failed: label suggest degraded ({})",
            suggestions.diagnostics.join(",")
        )));
    }
    let selected = suggestions
        .selected_labels
        .iter()
        .find(|label| label.label_id == preflight.label_id)
        .map(|label| (label.score, "selected_labels"));
    let candidate_hit = suggestions
        .candidates
        .iter()
        .find(|label| label.label_id == preflight.label_id)
        .map(|label| (label.score, "candidates"));
    let Some((score, source)) = selected.or(candidate_hit) else {
        return Err(KanbanError::InvalidInput(format!(
            "label bootstrap verification failed: label {} was not returned by label suggest",
            candidate.name
        )));
    };
    if score < min_score {
        return Err(KanbanError::InvalidInput(format!(
            "label bootstrap verification failed: label {} score {score:.3} is below min_verify_score {min_score:.3}",
            candidate.name
        )));
    }
    let verification = BootstrapTaskLabelVerification {
        label_name: candidate.name.clone(),
        score,
        source: source.to_owned(),
        min_score,
        degraded: false,
        diagnostics: suggestions.diagnostics,
    };
    let verification_context = serde_json::to_string(&json!({
        "bootstrap_verification": {
            "label_name": verification.label_name,
            "score": verification.score,
            "source": verification.source,
            "min_score": verification.min_score,
            "degraded": verification.degraded,
            "diagnostics": verification.diagnostics,
        }
    }))
    .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;

    reach_bootstrap_staged_verification_stage(
        BootstrapStagedVerificationStage::BeforeCommit,
        &mut stage_hook,
    )?;
    let (task, semantics) = with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let task = resolve_task(&conn, &board_id, task_ref)?;
        ensure_task_allows_label_mutation(&conn, &task.id)?;
        let task_hash = suggest_input_hash_for_task(&task);
        let label_state_hash = bootstrap_label_state_hash(&conn, &board_id, &candidate.name)?;
        let board_digest = board_ontology_digest(&conn, &board_id)?;
        if task.id != preflight.task_id || task_hash != preflight.task_suggest_hash {
            return Err(KanbanError::Conflict(
                "label bootstrap verification conflict: task changed before commit".to_owned(),
            ));
        }
        if label_state_hash != preflight.label_state_hash
            || board_digest != preflight.board_ontology_digest
        {
            return Err(KanbanError::Conflict(
                "label bootstrap verification conflict: ontology changed before commit".to_owned(),
            ));
        }
        let adoption = adopt_label_semantics_candidate_with_reserved_label_in_current_tx(
            &conn,
            &board_id,
            &candidate,
            LabelAdoptionProvenance::DirectBootstrap { actor },
            Some(&preflight.label_id),
            now,
            Some(verification_context),
        )?;
        attach_label_in_current_tx(&conn, &board_id, actor, &task.id, &adoption.label.id, now)?;
        reach_bootstrap_staged_verification_stage(
            BootstrapStagedVerificationStage::DuringCommit,
            &mut stage_hook,
        )?;
        Ok((
            get_task_by_id(&conn, &board_id, &task.id)?,
            adoption.semantics,
        ))
    })?;
    reach_bootstrap_staged_verification_stage(
        BootstrapStagedVerificationStage::AfterCommit,
        &mut stage_hook,
    )?;
    Ok(BootstrapTaskLabelVerifiedResult {
        task,
        semantics,
        verification,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BootstrapStagedVerificationStage {
    BeforeVerify,
    BeforeCommit,
    DuringCommit,
    AfterCommit,
}

#[cfg(debug_assertions)]
impl BootstrapStagedVerificationStage {
    fn as_str(self) -> &'static str {
        match self {
            Self::BeforeVerify => "before_verify",
            Self::BeforeCommit => "before_commit",
            Self::DuringCommit => "during_commit",
            Self::AfterCommit => "after_commit",
        }
    }
}

fn reach_bootstrap_staged_verification_stage(
    stage: BootstrapStagedVerificationStage,
    stage_hook: &mut impl FnMut(BootstrapStagedVerificationStage) -> Result<()>,
) -> Result<()> {
    bootstrap_staged_debug_failpoint(stage)?;
    stage_hook(stage)
}

#[cfg(debug_assertions)]
fn bootstrap_staged_debug_failpoint(stage: BootstrapStagedVerificationStage) -> Result<()> {
    if std::env::var("KANBAN_BOOTSTRAP_VERIFY_TEST_FAILPOINT").as_deref() != Ok(stage.as_str()) {
        return Ok(());
    }
    if let Ok(marker) = std::env::var("KANBAN_BOOTSTRAP_VERIFY_TEST_MARKER") {
        std::fs::write(&marker, stage.as_str()).map_err(|err| {
            KanbanError::InvalidInput(format!(
                "bootstrap verification test failpoint marker failed: {err}"
            ))
        })?;
    }
    let sleep_ms = std::env::var("KANBAN_BOOTSTRAP_VERIFY_TEST_SLEEP_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(30_000);
    std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
    Ok(())
}

#[cfg(not(debug_assertions))]
fn bootstrap_staged_debug_failpoint(_stage: BootstrapStagedVerificationStage) -> Result<()> {
    Ok(())
}

struct BootstrapStagedPreflight {
    task_id: String,
    label_id: String,
    task_suggest_hash: String,
    label_state_hash: String,
    board_ontology_digest: String,
    atoms: Vec<LabelAtomVector>,
}

fn bootstrap_staged_preflight(
    conn: &Connection,
    board: &str,
    task_ref: &str,
    candidate: &super::LabelProposalCandidate,
    embedding_model: &str,
    now: i64,
) -> Result<BootstrapStagedPreflight> {
    let board_id = board_id(conn, board)?;
    let task = resolve_task(conn, &board_id, task_ref)?;
    ensure_task_allows_label_mutation(conn, &task.id)?;
    let label = label_by_name(conn, &board_id, &candidate.name)?;
    let label_id = if let Some(label) = &label {
        if count_label_semantics(conn, &board_id, &label.id)? > 0 {
            return Err(KanbanError::InvalidInput(format!(
                "label bootstrap would replace existing semantics for label {}; use a dedicated semantics mutation or proposal adoption path",
                label.name
            )));
        }
        label.id.clone()
    } else {
        new_label_id()
    };
    let mut atoms = label_atom_vectors_for_board(conn, &board_id, embedding_model)?;
    atoms.retain(|atom| atom.label_id != label_id);
    let definition = label_definition_from_bootstrap_candidate(&label_id, candidate);
    atoms.extend(label_atom_vectors_for_definition(
        &definition,
        &board_id,
        &candidate.name,
        embedding_model,
        now,
    ));
    Ok(BootstrapStagedPreflight {
        task_id: task.id.clone(),
        label_id,
        task_suggest_hash: suggest_input_hash_for_task(&task),
        label_state_hash: bootstrap_label_state_hash(conn, &board_id, &candidate.name)?,
        board_ontology_digest: board_ontology_digest(conn, &board_id)?,
        atoms,
    })
}

fn label_definition_from_bootstrap_candidate(
    label_id: &str,
    candidate: &super::LabelProposalCandidate,
) -> LabelDefinition {
    LabelDefinition {
        id: label_id.to_owned(),
        name: candidate.name.clone(),
        description: candidate.description.clone(),
        applies_when: candidate.applies_when.clone(),
        positive_examples: candidate.positive_examples.clone(),
        excludes_when: candidate.excludes_when.clone(),
        negative_examples: candidate.negative_examples.clone(),
    }
}

struct StagedBootstrapLabelAtomStore<'a, S: LabelAtomVectorStore + ?Sized> {
    source: &'a S,
    atoms: Vec<(LabelAtomVector, Vec<f32>)>,
}

impl<'a, S: LabelAtomVectorStore + ?Sized> StagedBootstrapLabelAtomStore<'a, S> {
    fn new(source: &'a S, atoms: Vec<LabelAtomVector>) -> Result<Self> {
        let atoms = atoms
            .into_iter()
            .map(|atom| {
                let vector = source
                    .embed_query_text(&atom.text)
                    .map_err(vector_storage)?;
                Ok((atom, vector))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { source, atoms })
    }
}

impl<S: LabelAtomVectorStore + ?Sized> VectorStoreBackend for StagedBootstrapLabelAtomStore<'_, S> {
    fn embedding_model(&self) -> &str {
        self.source.embedding_model()
    }

    fn status(&self) -> VectorStoreStatus {
        let mut status = self.source.status();
        status.dirty = Some(false);
        status.board_dirty = Some(false);
        status
    }
}

impl<S: LabelAtomVectorStore + ?Sized> kanban_vector::QueryEmbeddingProvider
    for StagedBootstrapLabelAtomStore<'_, S>
{
    fn embed_query_text(&self, text: &str) -> std::result::Result<Vec<f32>, VectorError> {
        self.source.embed_query_text(text)
    }
}

impl<S: LabelAtomVectorStore + ?Sized> LabelAtomVectorStore
    for StagedBootstrapLabelAtomStore<'_, S>
{
    fn query_label_atoms_by_vector(
        &self,
        query: &LabelAtomVectorQuery,
    ) -> std::result::Result<Vec<LabelAtomVectorHit>, VectorError> {
        let mut hits = self
            .atoms
            .iter()
            .filter(|(atom, _vector)| {
                query
                    .board_id
                    .as_ref()
                    .is_none_or(|board_id| &atom.board_id == board_id)
                    && query
                        .embedding_model
                        .as_ref()
                        .is_none_or(|model| &atom.embedding_model == model)
                    && query
                        .polarity
                        .as_ref()
                        .is_none_or(|polarity| &atom.polarity == polarity)
            })
            .filter_map(|(atom, vector)| {
                let similarity = cosine(&query.vector, vector);
                (similarity > 0.0).then_some((similarity, atom, vector))
            })
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| right.0.total_cmp(&left.0));
        Ok(hits
            .into_iter()
            .take(query.limit)
            .map(|(similarity, atom, vector)| LabelAtomVectorHit {
                hit: LabelAtomHit {
                    atom_id: atom.atom_id.clone(),
                    label_id: atom.label_id.clone(),
                    label_name: atom.label_name.clone(),
                    board_id: atom.board_id.clone(),
                    polarity: atom.polarity.clone(),
                    kind: atom.kind.clone(),
                    text: atom.text.clone(),
                    ordinal: atom.ordinal,
                    content_hash: atom.content_hash.clone(),
                    embedding_model: atom.embedding_model.clone(),
                    distance: (1.0 / similarity.max(0.0001)) - 1.0,
                },
                vector: query.include_vector.then(|| vector.clone()),
            })
            .collect())
    }
}

fn cosine(left: &[f32], right: &[f32]) -> f32 {
    let dot = left
        .iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm * right_norm)
    }
}

fn suggest_input_hash_for_task(task: &TaskRecord) -> String {
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

fn bootstrap_label_state_hash(
    conn: &Connection,
    board_id: &str,
    label_name: &str,
) -> Result<String> {
    let label = label_by_name(conn, board_id, label_name)?;
    let value = if let Some(label) = label {
        let semantics_hash =
            match super::label_semantics::get_label_semantics_conn(conn, board_id, &label.id) {
                Ok(semantics) => Some(semantics.semantics_hash),
                Err(KanbanError::NotFound(_)) => None,
                Err(error) => return Err(error),
            };
        json!({
            "state": "present",
            "id": label.id,
            "name": label.name,
            "color": label.color,
            "updated_at": label.updated_at,
            "semantics_hash": semantics_hash,
        })
    } else {
        json!({
            "state": "missing",
            "name": label_name,
        })
    };
    stable_json_hash(&value)
}

fn board_ontology_digest(conn: &Connection, board_id: &str) -> Result<String> {
    let labels = all(
        conn,
        "SELECT id,board_id,name,color,created_at,updated_at FROM labels WHERE board_id=?1 ORDER BY id ASC",
        [board_id],
        label_from_row,
    )?;
    let board_param = vec![rusqlite::types::Value::Text(board_id.to_owned())];
    let semantics = all_values(
        conn,
        "SELECT label_id,description,applies_when,excludes_when,positive_examples,negative_examples,created_at,updated_at \
         FROM label_semantics WHERE board_id=?1 ORDER BY label_id ASC",
        &board_param,
        |row| {
            Ok(json!({
                "label_id": row.get::<_, String>(0)?,
                "description": row.get::<_, Option<String>>(1)?,
                "applies_when": row.get::<_, String>(2)?,
                "excludes_when": row.get::<_, String>(3)?,
                "positive_examples": row.get::<_, String>(4)?,
                "negative_examples": row.get::<_, String>(5)?,
                "created_at": row.get::<_, i64>(6)?,
                "updated_at": row.get::<_, i64>(7)?,
            }))
        },
    )?;
    let atoms = all_values(
        conn,
        "SELECT id,label_id,polarity,kind,text,ordinal,content_hash,created_at,updated_at \
         FROM label_atoms WHERE board_id=?1 ORDER BY label_id ASC, ordinal ASC, id ASC",
        &board_param,
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "label_id": row.get::<_, String>(1)?,
                "polarity": row.get::<_, String>(2)?,
                "kind": row.get::<_, String>(3)?,
                "text": row.get::<_, String>(4)?,
                "ordinal": row.get::<_, i64>(5)?,
                "content_hash": row.get::<_, String>(6)?,
                "created_at": row.get::<_, i64>(7)?,
                "updated_at": row.get::<_, i64>(8)?,
            }))
        },
    )?;
    stable_json_hash(&json!({
        "labels": labels,
        "semantics": semantics,
        "atoms": atoms,
    }))
}

fn stable_json_hash(value: &serde_json::Value) -> Result<String> {
    serde_json::to_string(value)
        .map(|json| stable_hash(&json))
        .map_err(|err| KanbanError::InvalidInput(err.to_string()))
}

fn stable_hash(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub fn remove_task_label(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    label_ref: &str,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let task = resolve_task(&conn, &board_id, task_ref)?;
        ensure_task_allows_label_mutation(&conn, &task.id)?;
        remove_task_label_in_current_tx(&conn, &task.board_id, actor, &task.id, label_ref, now)?;
        get_task_by_id(&conn, &task.board_id, &task.id)
    })
}

pub fn remove_task_label_by_id(
    path: impl AsRef<Path>,
    actor: &str,
    task_id: &str,
    label_ref: &str,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = active_board_id_for_label_mutation(&conn, task_id)?;
        let task = get_task_by_id(&conn, &board_id, task_id)?;
        remove_task_label_in_current_tx(&conn, &task.board_id, actor, &task.id, label_ref, now)?;
        get_task_by_id(&conn, &task.board_id, &task.id)
    })
}

pub fn update_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    patch: TaskPatch,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    validate_retry_policy(patch.max_retries.flatten())?;
    if let Some(priority) = patch.priority {
        validate_priority(priority)?;
    }
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let mut task = resolve_task(&conn, &board_id, task_ref)?;
        let content_recompute_allowed = task.status != TaskStatus::Todo;
        let recompute_needed = patch.scheduled_at.is_some()
            || (content_recompute_allowed
                && (patch.title.is_some() || patch.description.is_some()));
        if patch
            .expected_lock_version
            .is_some_and(|expected| task.lock_version != expected)
        {
            return Err(KanbanError::InvalidInput("lock_version mismatch".into()));
        }
        if let Some(title) = patch.title {
            if title.trim().is_empty() {
                return Err(KanbanError::InvalidInput("title is required".into()));
            }
            task.title = title;
        }
        if let Some(description) = patch.description {
            task.description = description;
        }
        if let Some(assignee) = patch.assignee {
            task.assignee = assignee;
        }
        if let Some(priority) = patch.priority {
            task.priority = priority;
        }
        if let Some(scheduled_at) = patch.scheduled_at {
            task.scheduled_at = scheduled_at;
        }
        if let Some(due_at) = patch.due_at {
            task.due_at = due_at;
        }
        if let Some(metadata_json) = patch.metadata_json {
            if !json_valid(&conn, &metadata_json)? {
                return Err(KanbanError::InvalidInput(
                    "metadata_json must be valid JSON".into(),
                ));
            }
            task.metadata_json = metadata_json;
        }
        if recompute_needed && is_active_recomputable_status(task.status) {
            task.status = recompute_ready_status(&conn, &task, now)?;
        }
        exec_one_named(
            &conn,
            "UPDATE tasks
             SET title=:title,
                 description=:description,
                 status=:status,
                 assignee=:assignee,
                 priority=:priority,
                 scheduled_at=:scheduled_at,
                 due_at=:due_at,
                 metadata_json=:metadata_json,
                 updated_at=:now,
                 lock_version=lock_version+1
             WHERE id=:task_id AND board_id=:board_id",
            named_params! {
                ":title": task.title,
                ":description": task.description,
                ":status": task.status.as_str(),
                ":assignee": task.assignee,
                ":priority": task.priority,
                ":scheduled_at": task.scheduled_at,
                ":due_at": task.due_at,
                ":metadata_json": task.metadata_json,
                ":now": now,
                ":task_id": task.id,
                ":board_id": board_id,
            },
            || KanbanError::InvalidTransition("task update failed".into()),
        )?;
        insert_event(
            &conn,
            &board_id,
            Some(&task.id),
            None,
            "task.updated",
            actor,
            "{}",
            now,
        )?;
        if let Some(max_retries) = patch.max_retries {
            update_retry_policy_in_current_tx(&conn, &board_id, actor, &task.id, max_retries, now)?;
        }
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn specify_task(
    path: impl AsRef<Path>,
    actor: &str,
    task_id: &str,
    description: Option<String>,
    scheduled_at: Option<i64>,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = active_board_id_for_task(&conn, task_id)?;
        let mut task = get_task_by_id(&conn, &board_id, task_id)?;
        if task.status != TaskStatus::Triage {
            return Err(KanbanError::InvalidTransition(format!(
                "cannot specify from {}",
                task.status.as_str()
            )));
        }
        if let Some(description) = description {
            task.description = Some(description);
        }
        if let Some(scheduled_at) = scheduled_at {
            task.scheduled_at = Some(scheduled_at);
        }
        if matches!(
            task.status,
            TaskStatus::Triage | TaskStatus::Todo | TaskStatus::Scheduled | TaskStatus::Ready
        ) {
            task.status = recompute_ready_status(&conn, &task, now)?;
        }
        exec(
            &conn,
            "UPDATE tasks SET description=?1, scheduled_at=?2, status=?3, updated_at=?4, lock_version=lock_version+1 WHERE id=?5 AND board_id=?6",
            params![
                task.description,
                task.scheduled_at,
                task.status.as_str(),
                now,
                task.id,
                board_id
            ],
        )?;
        insert_event(
            &conn,
            &board_id,
            Some(&task.id),
            None,
            "task.specified",
            actor,
            &json!({ "to_status": task.status.as_str() }).to_string(),
            now,
        )?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn list_tasks(
    path: impl AsRef<Path>,
    board: &str,
    statuses: &[TaskStatus],
    include_archived: bool,
) -> Result<Vec<TaskRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let mut tasks = query_tasks(&conn, &board_id)?;
    if !include_archived {
        tasks.retain(|t| t.status != TaskStatus::Archived);
    }
    if !statuses.is_empty() {
        tasks.retain(|t| statuses.contains(&t.status));
    }
    Ok(tasks)
}

pub fn list_tasks_page(
    path: impl AsRef<Path>,
    board: &str,
    options: TaskListOptions,
) -> Result<TaskListPage> {
    validate_page_bounds(options.limit, MAX_TASK_LIST_LIMIT, options.offset)?;
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let (where_sql, params) = task_query_where(&board_id, &options);
    let total_sql = format!("SELECT COUNT(*) FROM tasks {where_sql}");
    let total: i64 = scalar(
        &conn,
        &total_sql,
        rusqlite::params_from_iter(params.iter()),
        |row| row.get(0),
    )?;

    let mut page_params = params;
    page_params.push(Value::Integer(
        options.limit.try_into().expect("validated limit"),
    ));
    page_params.push(Value::Integer(
        options.offset.try_into().expect("validated offset"),
    ));
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks {where_sql} ORDER BY {} LIMIT ? OFFSET ?",
        task_order_by(options.sort)
    );
    let tasks = all_values(&conn, &sql, &page_params, task_from_row)?;
    Ok(TaskListPage {
        tasks,
        total: total as usize,
    })
}

pub fn get_task(path: impl AsRef<Path>, board: &str, task_ref: &str) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    resolve_task(&conn, &board_id, task_ref)
}

pub fn get_task_by_id_global(path: impl AsRef<Path>, task_id: &str) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    get_task_by_id_global_conn(&conn, task_id)
}

pub fn update_task_by_id(
    path: impl AsRef<Path>,
    actor: &str,
    task_id: &str,
    patch: TaskPatch,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = active_board_id_for_task(&conn, task_id)?;
    drop(conn);
    update_task(path, &board_id, actor, task_id, patch)
}

pub fn set_task_retry_policy_by_id(
    path: impl AsRef<Path>,
    actor: &str,
    task_id: &str,
    max_retries: Option<i64>,
) -> Result<TaskRecord> {
    validate_retry_policy(max_retries)?;
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = active_board_id_for_task(&conn, task_id)?;
        update_retry_policy_in_current_tx(&conn, &board_id, actor, task_id, max_retries, now)?;
        get_task_by_id(&conn, &board_id, task_id)
    })
}

fn validate_retry_policy(max_retries: Option<i64>) -> Result<()> {
    if max_retries.is_some_and(|value| value <= 0) {
        return Err(KanbanError::InvalidInput(
            "max_retries must be a positive integer".into(),
        ));
    }
    Ok(())
}

fn update_retry_policy_in_current_tx(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    task_id: &str,
    max_retries: Option<i64>,
    now: i64,
) -> Result<()> {
    let changed = exec(
        conn,
        "UPDATE tasks SET max_retries=?1, updated_at=?2, lock_version=lock_version+1 WHERE id=?3 AND board_id=?4",
        params![max_retries, now, task_id, board_id],
    )?;
    ensure_changed_one(changed, || {
        KanbanError::InvalidTransition("retry policy update failed".into())
    })?;
    insert_event(
        conn,
        board_id,
        Some(task_id),
        None,
        "task.retry_policy.updated",
        actor,
        &json!({ "max_retries": max_retries }).to_string(),
        now,
    )
}

pub(crate) fn task_query_where(board_id: &str, options: &TaskListOptions) -> (String, Vec<Value>) {
    let mut clauses = vec!["WHERE board_id=?".to_owned()];
    let mut params = vec![Value::Text(board_id.to_owned())];
    if !options.include_archived {
        clauses.push("status != 'archived'".to_owned());
    }
    if !options.statuses.is_empty() {
        let placeholders = std::iter::repeat_n("?", options.statuses.len())
            .collect::<Vec<_>>()
            .join(",");
        clauses.push(format!("status IN ({placeholders})"));
        params.extend(
            options
                .statuses
                .iter()
                .map(|status| Value::Text(status.as_str().to_owned())),
        );
    }
    if !options.priorities.is_empty() {
        let placeholders = std::iter::repeat_n("?", options.priorities.len())
            .collect::<Vec<_>>()
            .join(",");
        clauses.push(format!("priority IN ({placeholders})"));
        params.extend(options.priorities.iter().copied().map(Value::Integer));
    }
    for label in options
        .labels
        .iter()
        .map(|label| label.trim())
        .filter(|label| !label.is_empty())
    {
        clauses.push(
            "EXISTS (SELECT 1 FROM task_labels tl JOIN labels l ON l.id=tl.label_id WHERE tl.task_id=tasks.id AND tl.board_id=tasks.board_id AND l.board_id=tasks.board_id AND (l.name=? OR l.id=?))"
                .to_owned(),
        );
        params.push(Value::Text(label.to_owned()));
        params.push(Value::Text(label.to_owned()));
    }
    for filter in &options.plan_filters {
        match filter {
            TaskPlanFilter::PlanNeeded => clauses.push(
                "status NOT IN ('done','archived') AND NOT EXISTS (SELECT 1 FROM task_steps s WHERE s.board_id=tasks.board_id AND s.parent_task_id=tasks.id) AND NOT EXISTS (SELECT 1 FROM task_execution_plans ep WHERE ep.board_id=tasks.board_id AND ep.task_id=tasks.id AND ep.state='not_required')"
                    .to_owned(),
            ),
            TaskPlanFilter::HasSteps => clauses.push(
                "EXISTS (SELECT 1 FROM task_steps s WHERE s.board_id=tasks.board_id AND s.parent_task_id=tasks.id)"
                    .to_owned(),
            ),
            TaskPlanFilter::IncompleteRequiredSteps => clauses.push(
                "EXISTS (SELECT 1 FROM task_steps s WHERE s.board_id=tasks.board_id AND s.parent_task_id=tasks.id AND s.required=1 AND s.status NOT IN ('done','skipped'))"
                    .to_owned(),
            ),
        }
    }
    if let Some(assignee) = options
        .assignee
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        clauses.push("assignee=?".to_owned());
        params.push(Value::Text(assignee.to_owned()));
    }
    if let Some(search) = options
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some((clause, search_params)) = task_ref_filter(search, "tasks.") {
            clauses.push(clause);
            params.extend(search_params);
        } else {
            let needle = format!("%{}%", sqlite_like_literal(&search.to_lowercase()));
            clauses.push(
                "(lower(title) LIKE ? ESCAPE '\\' OR lower(COALESCE(description, '')) LIKE ? ESCAPE '\\')"
                    .to_owned(),
            );
            params.push(Value::Text(needle.clone()));
            params.push(Value::Text(needle));
        }
    }
    (clauses.join(" AND "), params)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskSearchQuery {
    TaskId(String),
    Seq(i64),
    BoardSeq { board: String, seq: i64 },
    RefNoMatch,
    Text,
}

pub(crate) fn classify_task_search_query(search: &str) -> TaskSearchQuery {
    let search = search.trim();
    if search.starts_with("t_") {
        return TaskSearchQuery::TaskId(search.to_owned());
    }
    if let Some((board, seq_ref)) = split_board_seq_ref(search) {
        return parse_task_search_seq(seq_ref)
            .map(|seq| TaskSearchQuery::BoardSeq {
                board: board.to_owned(),
                seq,
            })
            .unwrap_or(TaskSearchQuery::RefNoMatch);
    }
    if search.starts_with('#') {
        return parse_task_search_seq(search)
            .map(TaskSearchQuery::Seq)
            .unwrap_or(TaskSearchQuery::RefNoMatch);
    }
    if search.chars().all(|ch| ch.is_ascii_digit()) {
        return parse_task_search_seq(search)
            .map(TaskSearchQuery::Seq)
            .unwrap_or(TaskSearchQuery::RefNoMatch);
    }
    TaskSearchQuery::Text
}

pub(crate) fn task_ref_filter(search: &str, task_prefix: &str) -> Option<(String, Vec<Value>)> {
    match classify_task_search_query(search) {
        TaskSearchQuery::TaskId(task_id) => {
            Some((format!("{task_prefix}id=?"), vec![Value::Text(task_id)]))
        }
        TaskSearchQuery::Seq(seq) => {
            Some((format!("{task_prefix}seq=?"), vec![Value::Integer(seq)]))
        }
        TaskSearchQuery::BoardSeq { board, seq } => Some((
            format!(
                "{task_prefix}seq=? AND EXISTS (SELECT 1 FROM boards b WHERE b.id={task_prefix}board_id AND (b.slug=? OR b.id=?))"
            ),
            vec![
                Value::Integer(seq),
                Value::Text(board.clone()),
                Value::Text(board),
            ],
        )),
        TaskSearchQuery::RefNoMatch => Some(("0=1".to_owned(), Vec::new())),
        TaskSearchQuery::Text => None,
    }
}

fn parse_task_search_seq(seq_ref: &str) -> Option<i64> {
    let seq = seq_ref.strip_prefix('#').unwrap_or(seq_ref);
    if seq.is_empty() || !seq.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    seq.parse().ok()
}

pub(crate) fn validate_page_bounds(limit: usize, max_limit: usize, offset: usize) -> Result<()> {
    if limit > max_limit {
        return Err(KanbanError::InvalidInput(format!(
            "limit must be <= {max_limit}"
        )));
    }
    if offset > i64::MAX as usize {
        return Err(KanbanError::InvalidInput(format!(
            "offset must be <= {}",
            i64::MAX
        )));
    }
    Ok(())
}

pub(crate) fn sqlite_like_literal(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '%' | '_' | '\\' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

pub(crate) fn task_order_by(sort: TaskListSort) -> &'static str {
    match sort {
        TaskListSort::Seq => "seq ASC",
        TaskListSort::SeqDesc => "seq DESC",
        TaskListSort::Title => "lower(title) ASC, seq ASC",
        TaskListSort::TitleDesc => "lower(title) DESC, seq DESC",
        TaskListSort::Status => {
            "CASE status WHEN 'triage' THEN 10 WHEN 'todo' THEN 20 WHEN 'scheduled' THEN 30 WHEN 'ready' THEN 40 WHEN 'running' THEN 50 WHEN 'blocked' THEN 60 WHEN 'review' THEN 70 WHEN 'done' THEN 80 ELSE 90 END ASC, position ASC, seq ASC"
        }
        TaskListSort::StatusDesc => {
            "CASE status WHEN 'triage' THEN 10 WHEN 'todo' THEN 20 WHEN 'scheduled' THEN 30 WHEN 'ready' THEN 40 WHEN 'running' THEN 50 WHEN 'blocked' THEN 60 WHEN 'review' THEN 70 WHEN 'done' THEN 80 ELSE 90 END DESC, position DESC, seq DESC"
        }
        TaskListSort::Position => "position ASC, created_at ASC, seq ASC",
        TaskListSort::PositionDesc => "position DESC, created_at DESC, seq DESC",
        TaskListSort::Priority => "priority ASC, created_at ASC, seq ASC",
        TaskListSort::PriorityDesc => "priority DESC, created_at DESC, seq DESC",
        TaskListSort::Assignee => "COALESCE(assignee, claim_owner, '') ASC, seq ASC",
        TaskListSort::AssigneeDesc => "COALESCE(assignee, claim_owner, '') DESC, seq DESC",
        TaskListSort::ScheduledAt => {
            "COALESCE(scheduled_at, 9223372036854775807) ASC, created_at ASC, seq ASC"
        }
        TaskListSort::ScheduledAtDesc => {
            "COALESCE(scheduled_at, -9223372036854775808) DESC, created_at DESC, seq DESC"
        }
        TaskListSort::CreatedAt => "created_at ASC, seq ASC",
        TaskListSort::CreatedAtDesc => "created_at DESC, seq DESC",
        TaskListSort::UpdatedAt => "updated_at ASC, seq ASC",
        TaskListSort::UpdatedAtDesc => "updated_at DESC, seq DESC",
        TaskListSort::DueAt => "COALESCE(due_at, 9223372036854775807) ASC, created_at ASC, seq ASC",
        TaskListSort::DueAtDesc => {
            "COALESCE(due_at, -9223372036854775808) DESC, created_at DESC, seq DESC"
        }
    }
}

pub(crate) const TASK_COLUMNS: &str = "id,board_id,(SELECT slug FROM boards WHERE boards.id=tasks.board_id) AS board_slug,((SELECT slug FROM boards WHERE boards.id=tasks.board_id) || '#' || seq) AS task_ref,seq,title,description,status,status_reason,assignee,priority,position,scheduled_at,due_at,created_by,created_at,updated_at,started_at,completed_at,archived_at,claim_token,claim_owner,claim_expires_at,last_heartbeat_at,current_run_id,retry_count,max_retries,result_summary,result_json,metadata_json,lock_version,EXISTS(SELECT 1 FROM task_dependencies d JOIN tasks p ON p.id=d.parent_task_id WHERE d.child_task_id=tasks.id AND p.status NOT IN ('done','archived')) AS dependency_blocked,(SELECT COUNT(*) FROM task_dependencies d JOIN tasks p ON p.id=d.parent_task_id WHERE d.child_task_id=tasks.id AND p.status NOT IN ('done','archived')) AS unfinished_parent_count,CASE WHEN EXISTS(SELECT 1 FROM task_steps s WHERE s.board_id=tasks.board_id AND s.parent_task_id=tasks.id) THEN 'planned' WHEN EXISTS(SELECT 1 FROM task_execution_plans ep WHERE ep.board_id=tasks.board_id AND ep.task_id=tasks.id AND ep.state='not_required') THEN 'not_required' ELSE 'unplanned' END AS execution_plan_state,(SELECT COUNT(*) FROM task_steps s WHERE s.board_id=tasks.board_id AND s.parent_task_id=tasks.id AND s.required=1) AS required_step_count,(SELECT COUNT(*) FROM task_steps s WHERE s.board_id=tasks.board_id AND s.parent_task_id=tasks.id AND s.required=1 AND s.status IN ('done','skipped')) AS completed_required_step_count,(SELECT COUNT(*) FROM task_steps s WHERE s.board_id=tasks.board_id AND s.parent_task_id=tasks.id AND s.required=0) AS optional_step_count,COALESCE((SELECT json_group_array(json_object('id', id, 'board_id', board_id, 'name', name, 'color', color, 'created_at', created_at, 'updated_at', updated_at)) FROM (SELECT l.id, l.board_id, l.name, l.color, l.created_at, l.updated_at FROM task_labels tl JOIN labels l ON l.id=tl.label_id WHERE tl.task_id=tasks.id ORDER BY l.name ASC)), '[]') AS labels_json";

pub(crate) fn query_tasks(conn: &Connection, board_id: &str) -> Result<Vec<TaskRecord>> {
    all(
        conn,
        &format!(
            "SELECT {TASK_COLUMNS} FROM tasks WHERE board_id=?1 ORDER BY CASE status WHEN 'triage' THEN 10 WHEN 'todo' THEN 20 WHEN 'scheduled' THEN 30 WHEN 'ready' THEN 40 WHEN 'running' THEN 50 WHEN 'blocked' THEN 60 WHEN 'review' THEN 70 WHEN 'done' THEN 80 ELSE 90 END, position ASC, priority ASC, created_at ASC"
        ),
        [board_id],
        task_from_row,
    )
}

pub(crate) fn get_task_by_id(
    conn: &Connection,
    board_id: &str,
    task_id: &str,
) -> Result<TaskRecord> {
    required_row(
        conn,
        &format!("SELECT {TASK_COLUMNS} FROM tasks WHERE board_id=?1 AND id=?2"),
        params![board_id, task_id],
        task_from_row,
        || KanbanError::NotFound(format!("task {task_id}")),
    )
}

pub(crate) fn get_task_by_id_global_conn(conn: &Connection, task_id: &str) -> Result<TaskRecord> {
    required_row(
        conn,
        &format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id=?1"),
        [task_id],
        task_from_row,
        || KanbanError::NotFound(format!("task {task_id}")),
    )
}

pub(crate) fn resolve_task(
    conn: &Connection,
    active_board_id: &str,
    task_ref: &str,
) -> Result<TaskRecord> {
    if task_ref.starts_with("t_") {
        return get_task_by_id_global_conn(conn, task_ref);
    }
    if let Some((board_ref, seq_ref)) = split_board_seq_ref(task_ref) {
        let board_id = board_id(conn, board_ref)?;
        let seq = parse_seq_ref(seq_ref)?;
        return get_task_by_seq(conn, &board_id, seq, task_ref);
    }
    let seq = parse_seq_ref(task_ref)?;
    get_task_by_seq(conn, active_board_id, seq, task_ref)
}

pub(crate) fn resolve_task_any(
    conn: &Connection,
    active_board_id: &str,
    task_ref: &str,
) -> Result<TaskRecord> {
    if task_ref.starts_with("t_") {
        return get_task_by_id_global_conn(conn, task_ref);
    }
    if let Some((board_ref, seq_ref)) = split_board_seq_ref(task_ref) {
        let board_id = board_id_any(conn, board_ref)?;
        let seq = parse_seq_ref(seq_ref)?;
        return get_task_by_seq(conn, &board_id, seq, task_ref);
    }
    let seq = parse_seq_ref(task_ref)?;
    get_task_by_seq(conn, active_board_id, seq, task_ref)
}

pub(crate) fn resolve_task_without_active_board(
    conn: &Connection,
    task_ref: &str,
) -> Result<TaskRecord> {
    if task_ref.starts_with("t_") {
        return get_task_by_id_global_conn(conn, task_ref);
    }
    if let Some((board_ref, seq_ref)) = split_board_seq_ref(task_ref) {
        let board_id = board_id_any(conn, board_ref)?;
        let seq = parse_seq_ref(seq_ref)?;
        return get_task_by_seq(conn, &board_id, seq, task_ref);
    }
    Err(KanbanError::InvalidInput(
        "task ref must be a task id or board-qualified ref".into(),
    ))
}

pub(crate) fn split_board_seq_ref(task_ref: &str) -> Option<(&str, &str)> {
    task_ref
        .split_once("/#")
        .or_else(|| task_ref.split_once('#'))
        .filter(|(board, seq)| !board.is_empty() && !seq.is_empty())
}

pub(crate) fn parse_seq_ref(seq_ref: &str) -> Result<i64> {
    let seq = seq_ref.strip_prefix('#').unwrap_or(seq_ref);
    seq.parse()
        .map_err(|_| KanbanError::InvalidInput("invalid task seq".into()))
}

pub(crate) fn get_task_by_seq(
    conn: &Connection,
    board_id: &str,
    seq: i64,
    display_ref: &str,
) -> Result<TaskRecord> {
    required_row(
        conn,
        &format!("SELECT {TASK_COLUMNS} FROM tasks WHERE board_id=?1 AND seq=?2"),
        params![board_id, seq],
        task_from_row,
        || KanbanError::NotFound(format!("task {display_ref}")),
    )
}

pub(crate) fn task_from_row(row: &Row<'_>) -> rusqlite::Result<TaskRecord> {
    let status: String = row.get(7)?;
    let execution_plan_state: String = row.get(33)?;
    let labels_json: String = row.get(37)?;
    let labels: Vec<LabelRecord> = serde_json::from_str(&labels_json)
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    Ok(TaskRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        board_slug: row.get(2)?,
        task_ref: row.get(3)?,
        seq: row.get(4)?,
        title: row.get(5)?,
        description: row.get(6)?,
        status: TaskStatus::try_from(status.as_str())
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        status_reason: row.get(8)?,
        assignee: row.get(9)?,
        priority: row.get(10)?,
        position: row.get(11)?,
        scheduled_at: row.get(12)?,
        due_at: row.get(13)?,
        created_by: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
        started_at: row.get(17)?,
        completed_at: row.get(18)?,
        archived_at: row.get(19)?,
        claim_token: row.get(20)?,
        claim_owner: row.get(21)?,
        claim_expires_at: row.get(22)?,
        last_heartbeat_at: row.get(23)?,
        current_run_id: row.get(24)?,
        retry_count: row.get(25)?,
        max_retries: row.get(26)?,
        result_summary: row.get(27)?,
        result_json: row.get(28)?,
        metadata_json: row.get(29)?,
        lock_version: row.get(30)?,
        dependency_blocked: row.get(31)?,
        unfinished_parent_count: row.get(32)?,
        execution_plan_state: StepPlanState::from_db_str(&execution_plan_state).ok_or_else(
            || {
                rusqlite::Error::InvalidParameterName(format!(
                    "invalid execution plan state: {execution_plan_state}"
                ))
            },
        )?,
        required_step_count: row.get(34)?,
        completed_required_step_count: row.get(35)?,
        optional_step_count: row.get(36)?,
        labels,
    })
}

fn normalize_label_names(labels: &[String]) -> Result<Vec<String>> {
    let mut normalized = Vec::with_capacity(labels.len());
    for label in labels {
        let label = normalize_label_name(label)?;
        if !normalized.contains(&label) {
            normalized.push(label);
        }
    }
    Ok(normalized)
}

fn bootstrap_label_candidate(input: BootstrapTaskLabel) -> Result<super::LabelProposalCandidate> {
    let name = normalize_label_name(&input.name)?;
    let description = normalize_optional_label_semantic(input.description);
    let applies_when = normalize_label_semantic_list(input.applies_when);
    let excludes_when = normalize_label_semantic_list(input.excludes_when);
    let positive_examples = normalize_label_semantic_list(input.positive_examples);
    let negative_examples = normalize_label_semantic_list(input.negative_examples);
    if description.is_none()
        && applies_when.is_empty()
        && excludes_when.is_empty()
        && positive_examples.is_empty()
        && negative_examples.is_empty()
    {
        return Err(KanbanError::InvalidInput(
            "label bootstrap requires description or semantic examples".into(),
        ));
    }
    Ok(super::LabelProposalCandidate {
        name,
        description,
        applies_when,
        excludes_when,
        positive_examples,
        negative_examples,
    })
}

pub(crate) enum LabelAdoptionProvenance<'a> {
    DirectBootstrap {
        actor: &'a str,
    },
    ProposalBootstrap {
        proposal: &'a LabelSemanticProposalRecord,
        options: LabelOntologyProposalBootstrapOptions,
    },
}

pub(crate) struct LabelAdoptionResult {
    pub(crate) label: LabelRecord,
    pub(crate) semantics: LabelSemanticsRecord,
}

pub(crate) fn adopt_label_semantics_candidate_in_current_tx(
    conn: &Connection,
    board_id: &str,
    candidate: &super::LabelProposalCandidate,
    provenance: LabelAdoptionProvenance<'_>,
    now: i64,
) -> Result<LabelAdoptionResult> {
    adopt_label_semantics_candidate_with_reserved_label_in_current_tx(
        conn, board_id, candidate, provenance, None, now, None,
    )
}

fn adopt_label_semantics_candidate_with_reserved_label_in_current_tx(
    conn: &Connection,
    board_id: &str,
    candidate: &super::LabelProposalCandidate,
    provenance: LabelAdoptionProvenance<'_>,
    reserved_label_id: Option<&str>,
    now: i64,
    context_json: Option<String>,
) -> Result<LabelAdoptionResult> {
    let label = ensure_bootstrap_label_writable_in_current_tx(
        conn,
        board_id,
        &candidate.name,
        now,
        reserved_label_id,
    )?;
    let before = label_ontology_semantics_snapshot_in_tx(conn, board_id, &label.id, &label.name)?;
    let before_atoms = super::label_ontology_mutation_atoms(conn, board_id, &label.id)?;
    upsert_label_semantics_candidate_in_tx(conn, board_id, &label.id, &label.name, candidate, now)?;
    mark_label_atom_store_dirty(conn, board_id, now)?;
    match provenance {
        LabelAdoptionProvenance::DirectBootstrap { actor } => {
            record_label_ontology_semantics_mutation_in_tx(
                conn,
                LabelOntologySemanticsMutationInput {
                    board_id,
                    label_id: &label.id,
                    label_name: &label.name,
                    action_type: LabelOntologyActionType::BootstrapLabel,
                    before,
                    before_atoms,
                    include_description_effects: true,
                    options: LabelSemanticsMutationOptions {
                        context_json,
                        ..LabelSemanticsMutationOptions::manual_actor(actor)
                    },
                },
                now,
            )?;
        }
        LabelAdoptionProvenance::ProposalBootstrap { proposal, options } => {
            record_label_ontology_proposal_bootstrap_in_tx(
                conn, proposal, &label.id, options, now,
            )?;
        }
    }
    let semantics = super::label_semantics::get_label_semantics_conn(conn, board_id, &label.id)?;
    Ok(LabelAdoptionResult { label, semantics })
}

fn normalize_optional_label_semantic(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

fn normalize_label_semantic_list(items: Vec<String>) -> Vec<String> {
    items
        .into_iter()
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
        .collect()
}

fn normalize_label_name(label: &str) -> Result<String> {
    let label = label.trim();
    if label.is_empty() {
        return Err(KanbanError::InvalidInput("label name is required".into()));
    }
    Ok(label.to_owned())
}

fn ensure_bootstrap_label_writable_in_current_tx(
    conn: &Connection,
    board_id: &str,
    name: &str,
    now: i64,
    reserved_label_id: Option<&str>,
) -> Result<LabelRecord> {
    let name = normalize_label_name(name)?;
    if let Some(existing) = label_by_name(conn, board_id, &name)? {
        if count_label_semantics(conn, board_id, &existing.id)? > 0 {
            return Err(KanbanError::InvalidInput(format!(
                "label bootstrap would replace existing semantics for label {}; use a dedicated semantics mutation or proposal adoption path",
                existing.name
            )));
        }
        return Ok(existing);
    }
    if let Some(id) = reserved_label_id {
        return ensure_label_with_id_in_current_tx(conn, board_id, id, &name, None, now);
    }
    ensure_label_in_current_tx(conn, board_id, &name, None, now)
}

fn label_for_task_binding_in_current_tx(
    conn: &Connection,
    board_id: &str,
    name: &str,
    create_missing: bool,
    now: i64,
) -> Result<(LabelRecord, bool)> {
    let name = normalize_label_name(name)?;
    if let Some(existing) = label_by_name(conn, board_id, &name)? {
        return Ok((existing, false));
    }
    if !create_missing {
        return Err(KanbanError::InvalidInput(format!(
            "label {name} does not exist; create it first with label create/bootstrap/proposal, or pass --create-missing for an explicit canonical identity"
        )));
    }
    ensure_label_in_current_tx(conn, board_id, &name, None, now).map(|label| (label, true))
}

fn ensure_label_in_current_tx(
    conn: &Connection,
    board_id: &str,
    name: &str,
    color: Option<&str>,
    now: i64,
) -> Result<LabelRecord> {
    let name = normalize_label_name(name)?;
    if let Some(existing) = label_by_name(conn, board_id, &name)? {
        return Ok(existing);
    }
    let id = kanban_core::new_label_id();
    exec(
        conn,
        "INSERT INTO labels(id, board_id, name, color, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![id, board_id, name, color, now],
    )?;
    label_by_id(conn, board_id, &id)?.ok_or_else(|| KanbanError::NotFound(format!("label {id}")))
}

fn ensure_label_with_id_in_current_tx(
    conn: &Connection,
    board_id: &str,
    id: &str,
    name: &str,
    color: Option<&str>,
    now: i64,
) -> Result<LabelRecord> {
    let name = normalize_label_name(name)?;
    if let Some(existing) = label_by_name(conn, board_id, &name)? {
        return Ok(existing);
    }
    exec(
        conn,
        "INSERT INTO labels(id, board_id, name, color, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![id, board_id, name, color, now],
    )?;
    label_by_id(conn, board_id, id)?.ok_or_else(|| KanbanError::NotFound(format!("label {id}")))
}

fn attach_label_in_current_tx(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    task_id: &str,
    label_id: &str,
    now: i64,
) -> Result<()> {
    let changed = exec(
        conn,
        "INSERT OR IGNORE INTO task_labels(board_id, task_id, label_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![board_id, task_id, label_id, now],
    )?;
    if changed > 0 {
        let label = label_by_id(conn, board_id, label_id)?
            .ok_or_else(|| KanbanError::NotFound(format!("label {label_id}")))?;
        insert_event(
            conn,
            board_id,
            Some(task_id),
            None,
            "task.label.added",
            actor,
            &json!({ "label_id": label.id, "label": label.name }).to_string(),
            now,
        )?;
    }
    Ok(())
}

fn list_labels_conn(conn: &Connection, board_id: &str) -> Result<Vec<LabelRecord>> {
    all(
        conn,
        "SELECT id, board_id, name, color, created_at, updated_at FROM labels WHERE board_id=?1 ORDER BY name ASC",
        [board_id],
        label_from_row,
    )
}

fn task_labels_conn(conn: &Connection, board_id: &str, task_id: &str) -> Result<Vec<LabelRecord>> {
    all(
        conn,
        "SELECT l.id, l.board_id, l.name, l.color, l.created_at, l.updated_at FROM task_labels tl JOIN labels l ON l.id=tl.label_id WHERE tl.board_id=?1 AND tl.task_id=?2 ORDER BY l.name ASC",
        params![board_id, task_id],
        label_from_row,
    )
}

fn count_task_label_bindings(conn: &Connection, board_id: &str, label_id: &str) -> Result<i64> {
    scalar(
        conn,
        "SELECT COUNT(*) FROM task_labels WHERE board_id=?1 AND label_id=?2",
        params![board_id, label_id],
        |row| row.get(0),
    )
}

fn count_label_semantics(conn: &Connection, board_id: &str, label_id: &str) -> Result<i64> {
    scalar(
        conn,
        "SELECT COUNT(*) FROM label_semantics WHERE board_id=?1 AND label_id=?2",
        params![board_id, label_id],
        |row| row.get(0),
    )
}

fn count_label_atoms(conn: &Connection, board_id: &str, label_id: &str) -> Result<i64> {
    scalar(
        conn,
        "SELECT COUNT(*) FROM label_atoms WHERE board_id=?1 AND label_id=?2",
        params![board_id, label_id],
        |row| row.get(0),
    )
}

fn resolve_label(conn: &Connection, board_id: &str, label_ref: &str) -> Result<LabelRecord> {
    let label_ref = normalize_label_name(label_ref)?;
    let label = if let Some(label) = label_by_name(conn, board_id, &label_ref)? {
        Some(label)
    } else if label_ref.starts_with("l_") {
        label_by_id(conn, board_id, &label_ref)?
    } else {
        None
    };
    label.ok_or_else(|| KanbanError::NotFound(format!("label {label_ref}")))
}

fn label_by_name(conn: &Connection, board_id: &str, name: &str) -> Result<Option<LabelRecord>> {
    optional(
        conn,
        "SELECT id, board_id, name, color, created_at, updated_at FROM labels WHERE board_id=?1 AND name=?2",
        params![board_id, name],
        label_from_row,
    )
}

fn label_by_id(conn: &Connection, board_id: &str, label_id: &str) -> Result<Option<LabelRecord>> {
    optional(
        conn,
        "SELECT id, board_id, name, color, created_at, updated_at FROM labels WHERE board_id=?1 AND id=?2",
        params![board_id, label_id],
        label_from_row,
    )
}

fn label_from_row(row: &Row<'_>) -> rusqlite::Result<LabelRecord> {
    Ok(LabelRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        name: row.get(2)?,
        color: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

pub(crate) fn active_board_id_for_task(conn: &Connection, task_id: &str) -> Result<String> {
    let board_id = optional(
        conn,
        "SELECT tasks.board_id FROM tasks JOIN boards ON boards.id=tasks.board_id WHERE tasks.id=?1 AND boards.archived_at IS NULL",
        [task_id],
        |r| r.get(0),
    )?;
    board_id.ok_or_else(|| KanbanError::NotFound(format!("task {task_id}")))
}

fn active_board_id_for_label_mutation(conn: &Connection, task_id: &str) -> Result<String> {
    required_row(
        conn,
        "SELECT tasks.board_id FROM tasks JOIN boards ON boards.id=tasks.board_id WHERE tasks.id=?1 AND boards.archived_at IS NULL AND tasks.status != 'archived'",
        [task_id],
        |r| r.get(0),
        || KanbanError::NotFound(format!("task {task_id}")),
    )
}

fn ensure_task_allows_label_mutation(conn: &Connection, task_id: &str) -> Result<()> {
    active_board_id_for_label_mutation(conn, task_id).map(|_| ())
}

fn remove_task_label_in_current_tx(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    task_id: &str,
    label_ref: &str,
    now: i64,
) -> Result<()> {
    let label = resolve_label(conn, board_id, label_ref)?;
    let changed = exec(
        conn,
        "DELETE FROM task_labels WHERE board_id=?1 AND task_id=?2 AND label_id=?3",
        params![board_id, task_id, label.id],
    )?;
    if changed > 0 {
        insert_event(
            conn,
            board_id,
            Some(task_id),
            None,
            "task.label.removed",
            actor,
            &json!({ "label_id": label.id, "label": label.name }).to_string(),
            now,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod bootstrap_staged_verification_tests {
    use super::*;
    use crate::init::init_database;
    use kanban_vector::{
        LabelAtomVectorHit, LabelAtomVectorQuery, QueryEmbeddingProvider, VectorError,
    };
    use std::path::PathBuf;

    struct TempDb {
        _dir: tempfile::TempDir,
        path: PathBuf,
    }

    impl TempDb {
        fn new(name: &str) -> Self {
            let dir = tempfile::Builder::new()
                .prefix(&format!("kb-sqlite-bootstrap-stage-{name}-"))
                .tempdir()
                .expect("tempdir");
            let path = dir.path().join("kb.db");
            Self { _dir: dir, path }
        }
    }

    #[derive(Clone, Copy)]
    struct PassingStore;

    impl VectorStoreBackend for PassingStore {
        fn embedding_model(&self) -> &str {
            "bootstrap-stage-test-model"
        }

        fn status(&self) -> VectorStoreStatus {
            VectorStoreStatus {
                backend: "test-vector".to_owned(),
                enabled: true,
                message: "test vector store; dirty=false last_error=none; board_dirty=false"
                    .to_owned(),
                diagnostics: Vec::new(),
                dirty: Some(false),
                board_dirty: Some(false),
                generation: None,
            }
        }
    }

    impl QueryEmbeddingProvider for PassingStore {
        fn embed_query_text(&self, _text: &str) -> std::result::Result<Vec<f32>, VectorError> {
            Ok(vec![1.0, 0.0, 0.0])
        }
    }

    impl LabelAtomVectorStore for PassingStore {
        fn query_label_atoms_by_vector(
            &self,
            _query: &LabelAtomVectorQuery,
        ) -> std::result::Result<Vec<LabelAtomVectorHit>, VectorError> {
            Ok(Vec::new())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct BootstrapCounts {
        labels: i64,
        label_semantics: i64,
        label_atoms: i64,
        task_labels: i64,
        task_events: i64,
        label_ontology_actions: i64,
        label_ontology_action_atom_effects: i64,
    }

    fn table_count(conn: &Connection, table: &str) -> Result<i64> {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .map_err(|err| KanbanError::Storage(err.to_string()))
    }

    fn bootstrap_counts(conn: &Connection) -> Result<BootstrapCounts> {
        Ok(BootstrapCounts {
            labels: table_count(conn, "labels")?,
            label_semantics: table_count(conn, "label_semantics")?,
            label_atoms: table_count(conn, "label_atoms")?,
            task_labels: table_count(conn, "task_labels")?,
            task_events: table_count(conn, "task_events")?,
            label_ontology_actions: table_count(conn, "label_ontology_actions")?,
            label_ontology_action_atom_effects: table_count(
                conn,
                "label_ontology_action_atom_effects",
            )?,
        })
    }

    fn label_count(conn: &Connection, label_name: &str) -> Result<i64> {
        conn.query_row(
            "SELECT COUNT(*) FROM labels WHERE name=?1",
            [label_name],
            |row| row.get(0),
        )
        .map_err(|err| KanbanError::Storage(err.to_string()))
    }

    fn bootstrap_action_count(conn: &Connection) -> Result<i64> {
        conn.query_row(
            "SELECT COUNT(*) FROM label_ontology_actions WHERE action_type='bootstrap_label'",
            [],
            |row| row.get(0),
        )
        .map_err(|err| KanbanError::Storage(err.to_string()))
    }

    fn bootstrap_effect_count(conn: &Connection) -> Result<i64> {
        conn.query_row(
            "SELECT COUNT(*) FROM label_ontology_action_atom_effects",
            [],
            |row| row.get(0),
        )
        .map_err(|err| KanbanError::Storage(err.to_string()))
    }

    fn bootstrap_input() -> BootstrapTaskLabel {
        BootstrapTaskLabel {
            name: "database".to_owned(),
            description: Some("Database persistence work".to_owned()),
            applies_when: vec!["touches SQLite migrations".to_owned()],
            positive_examples: vec!["new table migration".to_owned()],
            ..BootstrapTaskLabel::default()
        }
    }

    fn bootstrap_target(path: &std::path::Path) -> Result<TaskRecord> {
        init_database(path, "tester")?;
        create_task(
            path,
            "default",
            "tester",
            CreateTask::ready("bootstrap staged verify target"),
        )
    }

    #[test]
    fn staged_verify_rejects_task_change_between_verify_and_commit() -> Result<()> {
        let temp = TempDb::new("task-change-before-commit");
        let task = bootstrap_target(&temp.path)?;
        let conn = connect_file(&temp.path)?;
        let before = bootstrap_counts(&conn)?;
        let hook_path = temp.path.clone();
        let task_id = task.id.clone();
        let mut changed = false;

        let error = bootstrap_task_label_with_staged_verification_impl(
            BootstrapTaskLabelStagedVerificationArgs {
                path: &temp.path,
                board: "default",
                actor: "tester",
                task_ref: &task.id,
                input: bootstrap_input(),
                store: &PassingStore,
                min_score: 0.50,
            },
            |stage| {
                if stage == BootstrapStagedVerificationStage::BeforeCommit && !changed {
                    changed = true;
                    update_task(
                        &hook_path,
                        "default",
                        "tester",
                        &task_id,
                        TaskPatch {
                            title: Some("bootstrap staged verify target changed".to_owned()),
                            ..TaskPatch::default()
                        },
                    )?;
                }
                Ok(())
            },
        )
        .expect_err("task change must conflict");

        assert!(matches!(error, KanbanError::Conflict(_)), "{error}");
        assert_eq!(label_count(&conn, "database")?, 0);
        assert_eq!(
            table_count(&conn, "label_semantics")?,
            before.label_semantics
        );
        assert_eq!(table_count(&conn, "label_atoms")?, before.label_atoms);
        assert_eq!(table_count(&conn, "task_labels")?, before.task_labels);
        assert_eq!(bootstrap_action_count(&conn)?, 0);
        assert_eq!(bootstrap_effect_count(&conn)?, 0);
        Ok(())
    }

    #[test]
    fn staged_verify_rejects_ontology_change_between_verify_and_commit() -> Result<()> {
        let temp = TempDb::new("ontology-change-before-commit");
        let task = bootstrap_target(&temp.path)?;
        let conn = connect_file(&temp.path)?;
        let before = bootstrap_counts(&conn)?;
        let hook_path = temp.path.clone();
        let mut changed = false;

        let error = bootstrap_task_label_with_staged_verification_impl(
            BootstrapTaskLabelStagedVerificationArgs {
                path: &temp.path,
                board: "default",
                actor: "tester",
                task_ref: &task.id,
                input: bootstrap_input(),
                store: &PassingStore,
                min_score: 0.50,
            },
            |stage| {
                if stage == BootstrapStagedVerificationStage::BeforeCommit && !changed {
                    changed = true;
                    create_label(
                        &hook_path,
                        "default",
                        CreateLabel {
                            name: "concurrent-ontology".to_owned(),
                            color: None,
                        },
                    )?;
                }
                Ok(())
            },
        )
        .expect_err("ontology change must conflict");

        assert!(matches!(error, KanbanError::Conflict(_)), "{error}");
        assert_eq!(label_count(&conn, "database")?, 0);
        assert_eq!(
            table_count(&conn, "label_semantics")?,
            before.label_semantics
        );
        assert_eq!(table_count(&conn, "label_atoms")?, before.label_atoms);
        assert_eq!(table_count(&conn, "task_labels")?, before.task_labels);
        assert_eq!(bootstrap_action_count(&conn)?, 0);
        assert_eq!(bootstrap_effect_count(&conn)?, 0);
        Ok(())
    }

    #[test]
    fn staged_verify_rolls_back_if_commit_stage_fails() -> Result<()> {
        let temp = TempDb::new("during-commit-rollback");
        let task = bootstrap_target(&temp.path)?;
        let conn = connect_file(&temp.path)?;
        let before = bootstrap_counts(&conn)?;

        let error = bootstrap_task_label_with_staged_verification_impl(
            BootstrapTaskLabelStagedVerificationArgs {
                path: &temp.path,
                board: "default",
                actor: "tester",
                task_ref: &task.id,
                input: bootstrap_input(),
                store: &PassingStore,
                min_score: 0.50,
            },
            |stage| {
                if stage == BootstrapStagedVerificationStage::DuringCommit {
                    return Err(KanbanError::InvalidInput(
                        "injected during commit failure".to_owned(),
                    ));
                }
                Ok(())
            },
        )
        .expect_err("during commit hook must fail");

        assert!(error.to_string().contains("injected during commit failure"));
        assert_eq!(bootstrap_counts(&conn)?, before);
        assert_eq!(label_count(&conn, "database")?, 0);
        Ok(())
    }
}
