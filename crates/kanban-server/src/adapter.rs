use kanban_application::{
    ApplicationStore, BoardColumnRecord as ApplicationBoardColumn, BoardRecord,
    CreateTaskRecord as ApplicationCreateTask, ExecutionPlanState, TaskRecord as ApplicationTask,
};
use kanban_core::{Board, KanbanError, Result, TaskStatus};
use kanban_store_turso::{
    CreateTaskInput as StoreCreateTask, StoreError, TaskRecord as StoreTask, TursoStore,
};

#[derive(Clone)]
pub(crate) struct TursoApplicationStore {
    store: TursoStore,
}

impl TursoApplicationStore {
    pub(crate) fn new(store: TursoStore) -> Self {
        Self { store }
    }
}

impl ApplicationStore for TursoApplicationStore {
    async fn list_boards(&self, include_archived: bool) -> Result<Vec<BoardRecord>> {
        self.store
            .list_boards(include_archived)
            .await
            .map(|boards| {
                boards
                    .into_iter()
                    .map(|board| Board {
                        id: board.id,
                        slug: board.slug,
                        name: board.name,
                        description: board.description,
                        created_at: board.created_at,
                        updated_at: board.updated_at,
                        archived_at: board.archived_at,
                    })
                    .collect()
            })
            .map_err(store_error)
    }

    async fn list_board_columns(&self, board: &str) -> Result<Vec<ApplicationBoardColumn>> {
        self.store
            .list_board_columns(board)
            .await
            .map_err(store_error)?
            .into_iter()
            .map(|column| {
                Ok(ApplicationBoardColumn {
                    id: column.id,
                    board_id: column.board_id,
                    status: column.status.parse::<TaskStatus>()?,
                    title: column.title,
                    position: column.position,
                    hidden: column.hidden,
                    wip_limit: column.wip_limit,
                    created_at: column.created_at,
                    updated_at: column.updated_at,
                })
            })
            .collect()
    }

    async fn create_task(
        &self,
        board: &str,
        input: ApplicationCreateTask,
    ) -> Result<ApplicationTask> {
        self.store
            .create_task(
                board,
                StoreCreateTask {
                    id: input.id,
                    idempotency_key: input.idempotency_key,
                    title: input.title,
                    status: input.status.as_str().to_owned(),
                    description: input.description,
                    assignee: input.assignee,
                    priority: input.priority,
                    scheduled_at: input.scheduled_at,
                    due_at: input.due_at,
                    max_retries: input.max_retries,
                    metadata_json: input.metadata_json,
                    created_by: input.created_by,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }
}

fn store_error(error: StoreError) -> KanbanError {
    match error {
        StoreError::BoardNotFound(selector) => KanbanError::NotFound(format!("board {selector}")),
        StoreError::InvalidInput(message) => KanbanError::InvalidInput(message),
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

fn application_task(task: StoreTask) -> Result<ApplicationTask> {
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
        dependency_blocked: false,
        unfinished_parent_count: 0,
        execution_plan_state: ExecutionPlanState::Unplanned,
        required_step_count: 0,
        completed_required_step_count: 0,
        optional_step_count: 0,
    })
}
