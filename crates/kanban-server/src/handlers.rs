use std::time::UNIX_EPOCH;

use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
};
use kanban_application::{CreateTaskCommand, ExecutionPlanState, TaskRecord};
use kanban_contract::{
    ApiBoard, ApiBoardColumn, ApiCreateTaskStatus, ApiExecutionPlanState, ApiTask, ApiTaskPriority,
    ApiTaskStatus, CreateTaskPath, CreateTaskRequest, CreateTaskResponse, HealthReport,
    HealthResponse, ListBoardColumnsResponse, ListBoardsQuery, ListBoardsResponse,
};
use kanban_core::{KanbanError, TaskStatus, new_task_id};

use crate::{error::ApiError, state::AppState};

pub(crate) async fn health(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, ApiError> {
    state.application().health().await?;
    let metadata = tokio::fs::metadata(state.db_path())
        .await
        .map_err(|error| kanban_core::KanbanError::Storage(error.to_string()))?;
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    Ok(Json(HealthResponse::new(HealthReport {
        ok: true,
        db: "turso".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        db_path: state.db_path().display().to_string(),
        db_fingerprint: format!("turso:{}:{modified_ms}", metadata.len()),
    })))
}

pub(crate) async fn list_boards(
    State(state): State<AppState>,
    Query(query): Query<ListBoardsQuery>,
) -> Result<Json<ListBoardsResponse>, ApiError> {
    let data = state
        .application()
        .list_boards(query.include_archived)
        .await?
        .into_iter()
        .map(|board| ApiBoard {
            id: board.id,
            slug: board.slug,
            name: board.name,
            description: board.description,
            created_at: board.created_at,
            updated_at: board.updated_at,
            archived_at: board.archived_at,
        })
        .collect();
    Ok(Json(ListBoardsResponse { data }))
}

pub(crate) async fn list_board_columns(
    State(state): State<AppState>,
    Path(board): Path<String>,
) -> Result<Json<ListBoardColumnsResponse>, ApiError> {
    let data = state
        .application()
        .list_board_columns(&board)
        .await?
        .into_iter()
        .map(|column| ApiBoardColumn {
            id: column.id,
            board_id: column.board_id,
            status: match column.status {
                kanban_core::TaskStatus::Triage => kanban_contract::ApiTaskStatus::Triage,
                kanban_core::TaskStatus::Todo => kanban_contract::ApiTaskStatus::Todo,
                kanban_core::TaskStatus::Scheduled => kanban_contract::ApiTaskStatus::Scheduled,
                kanban_core::TaskStatus::Ready => kanban_contract::ApiTaskStatus::Ready,
                kanban_core::TaskStatus::Running => kanban_contract::ApiTaskStatus::Running,
                kanban_core::TaskStatus::Blocked => kanban_contract::ApiTaskStatus::Blocked,
                kanban_core::TaskStatus::Review => kanban_contract::ApiTaskStatus::Review,
                kanban_core::TaskStatus::Done => kanban_contract::ApiTaskStatus::Done,
                kanban_core::TaskStatus::Archived => kanban_contract::ApiTaskStatus::Archived,
            },
            title: column.title,
            position: column.position,
            hidden: column.hidden,
            wip_limit: column.wip_limit,
            created_at: column.created_at,
            updated_at: column.updated_at,
        })
        .collect();
    Ok(Json(ListBoardColumnsResponse { data }))
}

pub(crate) async fn create_task(
    State(state): State<AppState>,
    Path(CreateTaskPath { board }): Path<CreateTaskPath>,
    headers: HeaderMap,
    body: Result<Json<CreateTaskRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CreateTaskResponse>), ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    if !body.labels.is_empty() || !body.depends_on.is_empty() {
        return Err(KanbanError::FeatureNotAvailable(
            "task.create labels and dependencies are not available on the single-host path"
                .to_owned(),
        )
        .into());
    }
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .create_task(CreateTaskCommand {
            task_id: body.task_id.unwrap_or_else(new_task_id),
            board,
            idempotency_key: body.idempotency_key,
            title: body.title,
            description: body.description,
            requested_status: body.status.map(create_status),
            assignee: body.assignee,
            priority: body.priority,
            scheduled_at: body.scheduled_at,
            due_at: body.due_at,
            max_retries: body.max_retries,
            metadata: body.metadata.unwrap_or_default(),
            actor,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(CreateTaskResponse {
            data: api_task(task)?,
        }),
    ))
}

fn request_actor(
    body_actor: Option<&str>,
    headers: &HeaderMap,
    default_actor: &str,
) -> Result<String, ApiError> {
    let actor = match body_actor {
        Some(actor) => actor,
        None => headers
            .get("x-kb-actor")
            .map(|value| {
                value.to_str().map_err(|_| {
                    KanbanError::InvalidInput("x-kb-actor must contain valid text".to_owned())
                })
            })
            .transpose()?
            .unwrap_or(default_actor),
    };
    let actor = actor.trim();
    if actor.is_empty() {
        return Err(KanbanError::InvalidInput("actor is required".to_owned()).into());
    }
    Ok(actor.to_owned())
}

fn create_status(status: ApiCreateTaskStatus) -> TaskStatus {
    match status {
        ApiCreateTaskStatus::Triage => TaskStatus::Triage,
        ApiCreateTaskStatus::Todo => TaskStatus::Todo,
        ApiCreateTaskStatus::Scheduled => TaskStatus::Scheduled,
        ApiCreateTaskStatus::Ready => TaskStatus::Ready,
    }
}

fn api_task(task: TaskRecord) -> Result<ApiTask, ApiError> {
    let priority = ApiTaskPriority::try_from(task.priority).map_err(|priority| {
        KanbanError::Storage(format!("stored task priority is outside 0..=3: {priority}"))
    })?;
    let metadata = serde_json::from_str(&task.metadata_json).map_err(|error| {
        KanbanError::Storage(format!("stored task metadata is invalid JSON: {error}"))
    })?;
    let result = task
        .result_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|error| {
            KanbanError::Storage(format!("stored task result is invalid JSON: {error}"))
        })?;
    Ok(ApiTask {
        id: task.id,
        board_id: task.board_id,
        board_slug: task.board_slug,
        task_ref: task.task_ref,
        seq: task.seq,
        title: task.title,
        description: task.description,
        status: api_task_status(task.status),
        status_reason: task.status_reason,
        assignee: task.assignee,
        priority,
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
        result,
        metadata,
        lock_version: task.lock_version,
        dependency_blocked: task.dependency_blocked,
        unfinished_parent_count: task.unfinished_parent_count,
        execution_plan_state: match task.execution_plan_state {
            ExecutionPlanState::Unplanned => ApiExecutionPlanState::Unplanned,
            ExecutionPlanState::Planned => ApiExecutionPlanState::Planned,
            ExecutionPlanState::NotRequired => ApiExecutionPlanState::NotRequired,
        },
        required_step_count: task.required_step_count,
        completed_required_step_count: task.completed_required_step_count,
        optional_step_count: task.optional_step_count,
        labels: Vec::new(),
    })
}

fn api_task_status(status: TaskStatus) -> ApiTaskStatus {
    match status {
        TaskStatus::Triage => ApiTaskStatus::Triage,
        TaskStatus::Todo => ApiTaskStatus::Todo,
        TaskStatus::Scheduled => ApiTaskStatus::Scheduled,
        TaskStatus::Ready => ApiTaskStatus::Ready,
        TaskStatus::Running => ApiTaskStatus::Running,
        TaskStatus::Blocked => ApiTaskStatus::Blocked,
        TaskStatus::Review => ApiTaskStatus::Review,
        TaskStatus::Done => ApiTaskStatus::Done,
        TaskStatus::Archived => ApiTaskStatus::Archived,
    }
}
