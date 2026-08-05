use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use kanban_application::{
    ApplicationStore, CommentAuthorType as ApplicationCommentAuthorType,
    CommentKind as ApplicationCommentKind, CommentRecord as ApplicationComment,
    DependencyEdgeRecord as ApplicationDependencyEdge,
    DependencySnapshotRecord as ApplicationDependencySnapshot,
    ExecutionPlanRecord as ApplicationExecutionPlan, ExecutionPlanState,
    RunRecord as ApplicationRun, RunStatus as ApplicationRunStatus, StepRecord as ApplicationStep,
    TaskRecord as ApplicationTask,
};
use kanban_core::{KanbanError, Result, TaskStatus};
use kanban_store_turso::{
    DependencySnapshotRecord as StoreDependencySnapshot, StoreError,
    TaskExecutionPlanRecord as StoreExecutionPlan, TaskRecord as StoreTask,
    TaskRunRecord as StoreRun, TursoStore,
};

mod operations;

#[derive(Clone)]
pub(crate) struct TursoApplicationStore {
    store: TursoStore,
    run_log_root: Option<Arc<PathBuf>>,
}

impl TursoApplicationStore {
    pub(crate) fn new(store: TursoStore) -> Self {
        Self {
            store,
            run_log_root: None,
        }
    }

    pub(crate) fn with_run_log_root(store: TursoStore, run_log_root: Arc<PathBuf>) -> Self {
        Self {
            store,
            run_log_root: Some(run_log_root),
        }
    }

    pub(crate) fn run_log_root(&self) -> Option<&Path> {
        self.run_log_root.as_deref().map(PathBuf::as_path)
    }
}

impl ApplicationStore for TursoApplicationStore {}

fn store_error(error: StoreError) -> KanbanError {
    match error {
        StoreError::BoardNotFound(selector) => KanbanError::NotFound(format!("board {selector}")),
        StoreError::TaskNotFound(task_id) => KanbanError::NotFound(format!("task {task_id}")),
        StoreError::RunNotFound(run_id) => KanbanError::NotFound(format!("run {run_id}")),
        StoreError::StepNotFound(step_id) => KanbanError::NotFound(format!("step {step_id}")),
        StoreError::DependencyCycle(message) => KanbanError::Conflict(message),
        StoreError::InvalidInput(message) => KanbanError::InvalidInput(message),
        StoreError::InvalidTransition(message) => KanbanError::InvalidTransition(message),
        StoreError::ClaimConflict(message) => {
            KanbanError::InvalidTransition(format!("claim conflict: {message}"))
        }
        StoreError::ClaimTokenMismatch => {
            KanbanError::InvalidTransition("claim token mismatch".to_owned())
        }
        StoreError::StepsIncomplete(message) => KanbanError::StepsIncomplete(message),
        StoreError::IdempotencyConflict {
            board_id,
            key,
            existing_task_id,
        } => KanbanError::IdempotencyConflict(format!(
            "board {board_id}, key {key}, existing task {existing_task_id}"
        )),
        other => KanbanError::Storage(other.to_string()),
    }
}

fn application_dependency_snapshot(
    snapshot: StoreDependencySnapshot,
) -> Result<ApplicationDependencySnapshot> {
    Ok(ApplicationDependencySnapshot {
        task: application_task(snapshot.task)?,
        parents: snapshot
            .parents
            .into_iter()
            .map(application_task)
            .collect::<Result<Vec<_>>>()?,
        children: snapshot
            .children
            .into_iter()
            .map(application_task)
            .collect::<Result<Vec<_>>>()?,
        edges: snapshot
            .edges
            .into_iter()
            .map(|edge| {
                Ok(ApplicationDependencyEdge {
                    parent: application_task(edge.parent)?,
                    child: application_task(edge.child)?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
    })
}

fn application_task(task: StoreTask) -> Result<ApplicationTask> {
    let execution_plan_state = match task.execution_plan_state.as_str() {
        "unplanned" => ExecutionPlanState::Unplanned,
        "planned" => ExecutionPlanState::Planned,
        "not_required" => ExecutionPlanState::NotRequired,
        other => {
            return Err(KanbanError::Storage(format!(
                "stored execution plan state is invalid: {other}"
            )));
        }
    };
    Ok(ApplicationTask {
        id: task.id,
        board_id: task.board_id,
        board_slug: task.board_slug,
        task_ref: task.task_ref,
        seq: task.seq,
        title: task.title,
        description: task.description,
        status: task.status.parse::<TaskStatus>()?,
        status_reason: task.status_reason,
        assignee: task.assignee,
        priority: task.priority,
        position: task.position,
        scheduled_at: task.scheduled_at,
        due_at: task.due_at,
        created_by: task.created_by,
        created_at: task.created_at,
        updated_at: task.updated_at,
        started_at: task.started_at,
        completed_at: task.completed_at,
        archived_at: task.archived_at,
        has_claim_token: task.claim_token.is_some(),
        claim_owner: task.claim_owner,
        claim_expires_at: task.claim_expires_at,
        last_heartbeat_at: task.last_heartbeat_at,
        current_run_id: task.current_run_id,
        retry_count: task.retry_count,
        max_retries: task.max_retries,
        result_summary: task.result_summary,
        result_json: task.result_json,
        metadata_json: task.metadata_json,
        lock_version: task.lock_version,
        dependency_blocked: task.dependency_blocked,
        unfinished_parent_count: task.unfinished_parent_count,
        execution_plan_state,
        required_step_count: task.required_step_count,
        completed_required_step_count: task.completed_required_step_count,
        optional_step_count: task.optional_step_count,
    })
}

fn application_execution_plan(plan: StoreExecutionPlan) -> Result<ApplicationExecutionPlan> {
    let state = match plan.state.as_str() {
        "unplanned" => ExecutionPlanState::Unplanned,
        "planned" => ExecutionPlanState::Planned,
        "not_required" => ExecutionPlanState::NotRequired,
        other => {
            return Err(KanbanError::Storage(format!(
                "stored execution plan state is invalid: {other}"
            )));
        }
    };
    Ok(ApplicationExecutionPlan {
        board_id: plan.board_id,
        task_id: plan.task_id,
        state,
        reason: plan.reason,
        updated_by: plan.updated_by,
        updated_at: plan.updated_at,
    })
}

pub(crate) fn application_run(run: StoreRun) -> Result<ApplicationRun> {
    let status = match run.status.as_str() {
        "running" => ApplicationRunStatus::Running,
        "succeeded" => ApplicationRunStatus::Succeeded,
        "failed" => ApplicationRunStatus::Failed,
        "canceled" => ApplicationRunStatus::Canceled,
        "expired" => ApplicationRunStatus::Expired,
        other => {
            return Err(KanbanError::Storage(format!(
                "stored run status is invalid: {other}"
            )));
        }
    };
    Ok(ApplicationRun {
        id: run.id,
        board_id: run.board_id,
        task_id: run.task_id,
        status,
        worker_profile: run.worker_profile,
        worker_pid: run.worker_pid,
        claim_owner: run.claim_owner,
        claim_expires_at: run.claim_expires_at,
        started_at: run.started_at,
        last_heartbeat_at: run.last_heartbeat_at,
        finished_at: run.finished_at,
        exit_code: run.exit_code,
        summary: run.summary,
        error: run.error,
        log_path: run.log_path,
        metadata_json: run.metadata_json,
    })
}

fn application_comment(comment: kanban_store_turso::CommentRecord) -> Result<ApplicationComment> {
    let author_type = match comment.author_type.as_str() {
        "user" => ApplicationCommentAuthorType::User,
        "agent" => ApplicationCommentAuthorType::Agent,
        other => {
            return Err(KanbanError::Storage(format!(
                "stored comment author_type is invalid: {other}"
            )));
        }
    };
    let kind = match comment.kind.as_str() {
        "note" => ApplicationCommentKind::Note,
        "decision" => ApplicationCommentKind::Decision,
        "signal" => ApplicationCommentKind::Signal,
        other => {
            return Err(KanbanError::Storage(format!(
                "stored comment kind is invalid: {other}"
            )));
        }
    };
    Ok(ApplicationComment {
        id: comment.id,
        board_id: comment.board_id,
        task_id: comment.task_id,
        author: comment.author,
        author_type,
        agent_type: comment.agent_type,
        body: comment.body,
        kind,
        metadata_json: comment.metadata_json,
        created_at: comment.created_at,
    })
}

fn application_step(step: kanban_store_turso::TaskStepRecord) -> Result<ApplicationStep> {
    Ok(ApplicationStep {
        id: step.id,
        parent_task_id: step.parent_task_id,
        title: step.title,
        body: step.body,
        linked_task: step.linked_task.map(application_task).transpose()?,
        position: step.position,
        required: step.required,
        status: step.status,
        resolution_note: step.resolution_note,
        resolved_by: step.resolved_by,
        resolved_at: step.resolved_at,
        created_by: step.created_by,
        created_at: step.created_at,
        updated_by: step.updated_by,
        updated_at: step.updated_at,
    })
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use kanban_store_turso::TursoStore;
    use tempfile::tempdir;

    use super::TursoApplicationStore;

    #[tokio::test]
    async fn run_log_root_constructor_preserves_optional_root() {
        let directory = tempdir().expect("temporary database directory");
        let store = TursoStore::open(directory.path().join("kanban.db"))
            .await
            .expect("open store");

        assert_eq!(
            TursoApplicationStore::new(store.clone()).run_log_root(),
            None
        );

        let root = Arc::new(PathBuf::from("runs"));
        let configured = TursoApplicationStore::with_run_log_root(store, root.clone());
        assert_eq!(configured.run_log_root(), Some(root.as_path()));
    }
}
