use kanban_application::{
    ApplicationStore, BlockTaskRecord as ApplicationBlockTask,
    BoardColumnRecord as ApplicationBoardColumn, BoardRecord, ClaimRecord as ApplicationClaim,
    ClaimTaskRecord as ApplicationClaimTask, CompleteTaskRecord as ApplicationCompleteTask,
    CreateTaskRecord as ApplicationCreateTask, ExecutionPlanRecord as ApplicationExecutionPlan,
    ExecutionPlanState, HeartbeatTaskRecord as ApplicationHeartbeatTask,
    MarkExecutionPlanNotRequiredRecord as ApplicationMarkExecutionPlanNotRequired,
    PromoteTaskRecord as ApplicationPromoteTask, ReleaseTaskRecord as ApplicationReleaseTask,
    RunRecord as ApplicationRun, RunStatus as ApplicationRunStatus,
    SubmitReviewTaskRecord as ApplicationSubmitReviewTask,
    TaskListOptions as ApplicationTaskListOptions, TaskListPage as ApplicationTaskListPage,
    TaskListSort as ApplicationTaskListSort, TaskPlanFilter as ApplicationTaskPlanFilter,
    TaskRecord as ApplicationTask,
};
use kanban_core::{Board, KanbanError, Result, TaskStatus};
use kanban_store_turso::{
    BlockTaskInput as StoreBlockTask, ClaimTaskInput as StoreClaimTask,
    ClaimTaskRecord as StoreClaim, CompleteTaskInput as StoreCompleteTask,
    CreateTaskInput as StoreCreateTask, HeartbeatTaskInput as StoreHeartbeatTask,
    MarkExecutionPlanNotRequiredInput as StoreMarkExecutionPlanNotRequired,
    PromoteTaskInput as StorePromoteTask, ReleaseTaskInput as StoreReleaseTask, StoreError,
    SubmitReviewTaskInput as StoreSubmitReviewTask, TaskExecutionPlanRecord as StoreExecutionPlan,
    TaskListOptions as StoreTaskListOptions, TaskListSort as StoreTaskListSort,
    TaskPlanFilter as StoreTaskPlanFilter, TaskRecord as StoreTask, TaskRunRecord as StoreRun,
    TursoStore,
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

    async fn list_tasks(
        &self,
        board: &str,
        options: ApplicationTaskListOptions,
    ) -> Result<ApplicationTaskListPage> {
        let page = self
            .store
            .list_tasks(
                board,
                StoreTaskListOptions {
                    statuses: options
                        .statuses
                        .into_iter()
                        .map(|status| status.as_str().to_owned())
                        .collect(),
                    priorities: options.priorities,
                    include_archived: options.include_archived,
                    assignee: options.assignee,
                    q: options.query,
                    plan_filters: options
                        .plan_filters
                        .into_iter()
                        .map(store_plan_filter)
                        .collect(),
                    sort: store_task_sort(options.sort),
                    limit: options.limit,
                    offset: options.offset,
                },
            )
            .await
            .map_err(store_error)?;
        Ok(ApplicationTaskListPage {
            tasks: page
                .tasks
                .into_iter()
                .map(application_task)
                .collect::<Result<Vec<_>>>()?,
            total: page.total,
        })
    }

    async fn get_task(&self, task_id: &str) -> Result<ApplicationTask> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn mark_execution_plan_not_required(
        &self,
        task_id: &str,
        input: ApplicationMarkExecutionPlanNotRequired,
    ) -> Result<ApplicationExecutionPlan> {
        self.store
            .mark_execution_plan_not_required(
                task_id,
                StoreMarkExecutionPlanNotRequired {
                    reason: input.reason,
                    actor: input.actor,
                    event_id: input.event_id,
                    updated_at: input.updated_at,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_execution_plan)
    }

    async fn promote_task(
        &self,
        task_id: &str,
        input: ApplicationPromoteTask,
    ) -> Result<ApplicationTask> {
        self.store
            .promote_task(
                task_id,
                StorePromoteTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    event_id: input.event_id,
                    updated_at: input.updated_at,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn claim_task(
        &self,
        task_id: &str,
        input: ApplicationClaimTask,
    ) -> Result<ApplicationClaim> {
        self.store
            .claim_task(
                task_id,
                StoreClaimTask {
                    expected_lock_version: input.expected_lock_version,
                    owner: input.actor,
                    claim_token: input.claim_token,
                    run_id: input.run_id,
                    event_id: input.event_id,
                    worker_profile: input.worker_profile,
                    metadata_json: input.metadata_json,
                    now: input.now,
                    claim_expires_at: input.claim_expires_at,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_claim)
    }

    async fn heartbeat_task(
        &self,
        task_id: &str,
        input: ApplicationHeartbeatTask,
    ) -> Result<ApplicationTask> {
        self.store
            .heartbeat_task(
                task_id,
                StoreHeartbeatTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    claim_token: input.claim_token,
                    event_id: input.event_id,
                    note: input.note,
                    now: input.now,
                    claim_expires_at: input.claim_expires_at,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn release_task(
        &self,
        task_id: &str,
        input: ApplicationReleaseTask,
    ) -> Result<ApplicationTask> {
        self.store
            .release_task(
                task_id,
                StoreReleaseTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    claim_token: input.claim_token,
                    event_id: input.event_id,
                    now: input.now,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn submit_review_task(
        &self,
        task_id: &str,
        input: ApplicationSubmitReviewTask,
    ) -> Result<ApplicationTask> {
        self.store
            .submit_review_task(
                task_id,
                StoreSubmitReviewTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    claim_token: input.claim_token,
                    force: input.force,
                    summary: input.summary,
                    event_id: input.event_id,
                    now: input.now,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn complete_task(
        &self,
        task_id: &str,
        input: ApplicationCompleteTask,
    ) -> Result<ApplicationTask> {
        self.store
            .complete_task(
                task_id,
                StoreCompleteTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    claim_token: input.claim_token,
                    force: input.force,
                    summary: input.summary,
                    result_json: input.result_json,
                    event_id: input.event_id,
                    now: input.now,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn block_task(
        &self,
        task_id: &str,
        input: ApplicationBlockTask,
    ) -> Result<ApplicationTask> {
        self.store
            .block_task(
                task_id,
                StoreBlockTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    reason: input.reason,
                    claim_token: input.claim_token,
                    force: input.force,
                    event_id: input.event_id,
                    now: input.now,
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
        StoreError::TaskNotFound(task_id) => KanbanError::NotFound(format!("task {task_id}")),
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

fn application_claim(claim: StoreClaim) -> Result<ApplicationClaim> {
    Ok(ApplicationClaim {
        task: application_task(claim.task)?,
        run: application_run(claim.run)?,
        claim_token: claim.claim_token,
        claim_expires_at: claim.claim_expires_at,
    })
}

fn application_run(run: StoreRun) -> Result<ApplicationRun> {
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

fn store_plan_filter(filter: ApplicationTaskPlanFilter) -> StoreTaskPlanFilter {
    match filter {
        ApplicationTaskPlanFilter::PlanNeeded => StoreTaskPlanFilter::PlanNeeded,
        ApplicationTaskPlanFilter::HasSteps => StoreTaskPlanFilter::HasSteps,
        ApplicationTaskPlanFilter::IncompleteRequiredSteps => {
            StoreTaskPlanFilter::IncompleteRequiredSteps
        }
    }
}

fn store_task_sort(sort: ApplicationTaskListSort) -> StoreTaskListSort {
    match sort {
        ApplicationTaskListSort::Seq => StoreTaskListSort::Seq,
        ApplicationTaskListSort::SeqDesc => StoreTaskListSort::SeqDesc,
        ApplicationTaskListSort::Title => StoreTaskListSort::Title,
        ApplicationTaskListSort::TitleDesc => StoreTaskListSort::TitleDesc,
        ApplicationTaskListSort::Status => StoreTaskListSort::Status,
        ApplicationTaskListSort::StatusDesc => StoreTaskListSort::StatusDesc,
        ApplicationTaskListSort::Position => StoreTaskListSort::Position,
        ApplicationTaskListSort::PositionDesc => StoreTaskListSort::PositionDesc,
        ApplicationTaskListSort::Priority => StoreTaskListSort::Priority,
        ApplicationTaskListSort::PriorityDesc => StoreTaskListSort::PriorityDesc,
        ApplicationTaskListSort::Assignee => StoreTaskListSort::Assignee,
        ApplicationTaskListSort::AssigneeDesc => StoreTaskListSort::AssigneeDesc,
        ApplicationTaskListSort::ScheduledAt => StoreTaskListSort::ScheduledAt,
        ApplicationTaskListSort::ScheduledAtDesc => StoreTaskListSort::ScheduledAtDesc,
        ApplicationTaskListSort::DueAt => StoreTaskListSort::DueAt,
        ApplicationTaskListSort::DueAtDesc => StoreTaskListSort::DueAtDesc,
        ApplicationTaskListSort::CreatedAt => StoreTaskListSort::CreatedAt,
        ApplicationTaskListSort::CreatedAtDesc => StoreTaskListSort::CreatedAtDesc,
        ApplicationTaskListSort::UpdatedAt => StoreTaskListSort::UpdatedAt,
        ApplicationTaskListSort::UpdatedAtDesc => StoreTaskListSort::UpdatedAtDesc,
    }
}
