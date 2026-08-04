use std::{collections::BTreeSet, str::FromStr, time::UNIX_EPOCH};

use axum::{
    Json,
    extract::{
        Path, Query, RawQuery, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{HeaderMap, StatusCode},
};
use kanban_application::{
    ClaimTaskCommand, CompleteTaskCommand, CreateTaskCommand, ExecutionPlanRecord,
    ExecutionPlanState, HeartbeatTaskCommand, MarkExecutionPlanNotRequiredCommand,
    PromoteTaskCommand, ReleaseTaskCommand, RunRecord, RunStatus, SubmitReviewTaskCommand,
    TaskListOptions as ApplicationTaskListOptions, TaskListSort as ApplicationTaskListSort,
    TaskPlanFilter as ApplicationTaskPlanFilter, TaskRecord,
};
use kanban_contract::{
    ApiBoard, ApiBoardColumn, ApiClaim, ApiCreateTaskStatus, ApiExecutionPlan,
    ApiExecutionPlanState, ApiRun, ApiRunStatus, ApiTask, ApiTaskPriority, ApiTaskStatus,
    ClaimTaskPath, ClaimTaskRequest, ClaimTaskResponse, CompleteTaskPath, CompleteTaskRequest,
    CompleteTaskResponse, CreateTaskPath, CreateTaskRequest, CreateTaskResponse, GetTaskPath,
    GetTaskQuery, GetTaskResponse, HealthReport, HealthResponse, HeartbeatTaskPath,
    HeartbeatTaskRequest, HeartbeatTaskResponse, ListBoardColumnsResponse, ListBoardsQuery,
    ListBoardsResponse, ListTasksPath, ListTasksQuery, ListTasksResponse,
    MAX_TASK_READ_ASSIGNEE_CHARS, MAX_TASK_READ_LABEL_CHARS, MAX_TASK_READ_LABELS,
    MAX_TASK_READ_LIMIT, MAX_TASK_READ_PLAN_FILTERS, MAX_TASK_READ_PRIORITIES,
    MAX_TASK_READ_Q_CHARS, MAX_TASK_READ_QUERY_BYTES, MAX_TASK_READ_QUERY_PAIRS,
    MAX_TASK_READ_STATUSES, MarkExecutionPlanNotRequiredPath, MarkExecutionPlanNotRequiredRequest,
    MarkExecutionPlanNotRequiredResponse, PromoteTaskPath, PromoteTaskRequest, PromoteTaskResponse,
    ReleaseTaskPath, ReleaseTaskRequest, ReleaseTaskResponse, SubmitReviewTaskPath,
    SubmitReviewTaskRequest, SubmitReviewTaskResponse, TaskReadLabel, TaskReadPlanFilter,
    TaskReadSort, TotalPaginationMeta,
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

pub(crate) async fn list_tasks(
    State(state): State<AppState>,
    Path(ListTasksPath { board }): Path<ListTasksPath>,
    RawQuery(raw_query): RawQuery,
) -> Result<Json<ListTasksResponse>, ApiError> {
    let query = parse_list_tasks_query(raw_query.as_deref())?;
    if !query.label.is_empty() {
        return Err(KanbanError::FeatureNotAvailable(
            "task.list label filters are not available on the single-host path".to_owned(),
        )
        .into());
    }
    let options = ApplicationTaskListOptions {
        statuses: query.status.into_iter().map(task_status).collect(),
        priorities: query
            .priority
            .into_iter()
            .map(|priority| i64::from(priority.get()))
            .collect(),
        plan_filters: query
            .plan_filter
            .into_iter()
            .map(application_plan_filter)
            .collect(),
        assignee: query.assignee,
        query: query.q,
        include_archived: query.include_archived,
        limit: query.limit,
        offset: query.offset,
        sort: application_task_sort(query.sort),
    };
    let page = state.application().list_tasks(&board, options).await?;
    let tasks = page
        .tasks
        .into_iter()
        .map(api_task)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(ListTasksResponse {
        data: tasks,
        meta: TotalPaginationMeta {
            limit: query.limit,
            offset: query.offset,
            total: page.total,
        },
    }))
}

pub(crate) async fn get_task(
    State(state): State<AppState>,
    Path(GetTaskPath { task_id }): Path<GetTaskPath>,
    query: Result<Query<GetTaskQuery>, QueryRejection>,
) -> Result<Json<GetTaskResponse>, ApiError> {
    let Query(query) =
        query.map_err(|error| KanbanError::InvalidInput(format!("invalid query: {error}")))?;
    if let Some(include) = query.include.as_deref() {
        if include == "ontology" {
            return Err(KanbanError::FeatureNotAvailable(
                "task ontology details are not available on the single-host path".to_owned(),
            )
            .into());
        }
        return Err(
            KanbanError::InvalidInput(format!("unsupported task include: {include}")).into(),
        );
    }
    let task = state.application().get_task(&task_id).await?;
    Ok(Json(GetTaskResponse::new(api_task(task)?, None)))
}

pub(crate) async fn mark_execution_plan_not_required(
    State(state): State<AppState>,
    Path(MarkExecutionPlanNotRequiredPath { task_id }): Path<MarkExecutionPlanNotRequiredPath>,
    headers: HeaderMap,
    body: Result<Json<MarkExecutionPlanNotRequiredRequest>, JsonRejection>,
) -> Result<Json<MarkExecutionPlanNotRequiredResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let plan = state
        .application()
        .mark_execution_plan_not_required(MarkExecutionPlanNotRequiredCommand {
            task_id,
            reason: body.reason,
            actor,
        })
        .await?;
    Ok(Json(MarkExecutionPlanNotRequiredResponse {
        data: api_execution_plan(plan),
    }))
}

pub(crate) async fn promote_task(
    State(state): State<AppState>,
    Path(PromoteTaskPath { task_id }): Path<PromoteTaskPath>,
    headers: HeaderMap,
    body: Result<Json<PromoteTaskRequest>, JsonRejection>,
) -> Result<Json<PromoteTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .promote_task(PromoteTaskCommand { task_id, actor })
        .await?;
    Ok(Json(PromoteTaskResponse::new(api_task(task)?)))
}

pub(crate) async fn claim_task(
    State(state): State<AppState>,
    Path(ClaimTaskPath { task_id }): Path<ClaimTaskPath>,
    headers: HeaderMap,
    body: Result<Json<ClaimTaskRequest>, JsonRejection>,
) -> Result<Json<ClaimTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let claim = state
        .application()
        .claim_task(ClaimTaskCommand {
            task_id,
            actor,
            ttl_ms: body.ttl_ms,
            worker_profile: body.worker_profile,
            metadata: body.metadata.unwrap_or_else(|| serde_json::json!({})),
        })
        .await?;
    Ok(Json(ClaimTaskResponse::new(ApiClaim {
        task: api_task(claim.task)?,
        run: api_run(claim.run)?,
        claim_token: claim.claim_token,
        claim_expires_at: Some(claim.claim_expires_at),
    })))
}

pub(crate) async fn heartbeat_task(
    State(state): State<AppState>,
    Path(HeartbeatTaskPath { task_id }): Path<HeartbeatTaskPath>,
    headers: HeaderMap,
    body: Result<Json<HeartbeatTaskRequest>, JsonRejection>,
) -> Result<Json<HeartbeatTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .heartbeat_task(HeartbeatTaskCommand {
            task_id,
            actor,
            claim_token: body.claim_token,
            ttl_ms: body.ttl_ms,
            note: body.note,
        })
        .await?;
    Ok(Json(HeartbeatTaskResponse::new(api_task(task)?)))
}

pub(crate) async fn release_task(
    State(state): State<AppState>,
    Path(ReleaseTaskPath { task_id }): Path<ReleaseTaskPath>,
    headers: HeaderMap,
    body: Result<Json<ReleaseTaskRequest>, JsonRejection>,
) -> Result<Json<ReleaseTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .release_task(ReleaseTaskCommand {
            task_id,
            actor,
            claim_token: body.claim_token,
        })
        .await?;
    Ok(Json(ReleaseTaskResponse::new(api_task(task)?)))
}

pub(crate) async fn submit_review_task(
    State(state): State<AppState>,
    Path(SubmitReviewTaskPath { task_id }): Path<SubmitReviewTaskPath>,
    headers: HeaderMap,
    body: Result<Json<SubmitReviewTaskRequest>, JsonRejection>,
) -> Result<Json<SubmitReviewTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .submit_review_task(SubmitReviewTaskCommand {
            task_id,
            actor,
            claim_token: body.claim_token,
            force: body.force,
            summary: body.summary,
        })
        .await?;
    Ok(Json(SubmitReviewTaskResponse::new(api_task(task)?)))
}

pub(crate) async fn complete_task(
    State(state): State<AppState>,
    Path(CompleteTaskPath { task_id }): Path<CompleteTaskPath>,
    headers: HeaderMap,
    body: Result<Json<CompleteTaskRequest>, JsonRejection>,
) -> Result<Json<CompleteTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .complete_task(CompleteTaskCommand {
            task_id,
            actor,
            claim_token: body.claim_token,
            force: body.force,
            summary: body.summary,
            result: body.result,
        })
        .await?;
    Ok(Json(CompleteTaskResponse::new(api_task(task)?)))
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

fn task_status(status: ApiTaskStatus) -> TaskStatus {
    match status {
        ApiTaskStatus::Triage => TaskStatus::Triage,
        ApiTaskStatus::Todo => TaskStatus::Todo,
        ApiTaskStatus::Scheduled => TaskStatus::Scheduled,
        ApiTaskStatus::Ready => TaskStatus::Ready,
        ApiTaskStatus::Running => TaskStatus::Running,
        ApiTaskStatus::Blocked => TaskStatus::Blocked,
        ApiTaskStatus::Review => TaskStatus::Review,
        ApiTaskStatus::Done => TaskStatus::Done,
        ApiTaskStatus::Archived => TaskStatus::Archived,
    }
}

fn application_plan_filter(filter: TaskReadPlanFilter) -> ApplicationTaskPlanFilter {
    match filter {
        TaskReadPlanFilter::PlanNeeded => ApplicationTaskPlanFilter::PlanNeeded,
        TaskReadPlanFilter::HasSteps => ApplicationTaskPlanFilter::HasSteps,
        TaskReadPlanFilter::IncompleteRequiredSteps => {
            ApplicationTaskPlanFilter::IncompleteRequiredSteps
        }
    }
}

fn application_task_sort(sort: TaskReadSort) -> ApplicationTaskListSort {
    match sort {
        TaskReadSort::Seq => ApplicationTaskListSort::Seq,
        TaskReadSort::SeqDesc => ApplicationTaskListSort::SeqDesc,
        TaskReadSort::Title => ApplicationTaskListSort::Title,
        TaskReadSort::TitleDesc => ApplicationTaskListSort::TitleDesc,
        TaskReadSort::Status => ApplicationTaskListSort::Status,
        TaskReadSort::StatusDesc => ApplicationTaskListSort::StatusDesc,
        TaskReadSort::Position => ApplicationTaskListSort::Position,
        TaskReadSort::PositionDesc => ApplicationTaskListSort::PositionDesc,
        TaskReadSort::Priority => ApplicationTaskListSort::Priority,
        TaskReadSort::PriorityDesc => ApplicationTaskListSort::PriorityDesc,
        TaskReadSort::Assignee => ApplicationTaskListSort::Assignee,
        TaskReadSort::AssigneeDesc => ApplicationTaskListSort::AssigneeDesc,
        TaskReadSort::ScheduledAt => ApplicationTaskListSort::ScheduledAt,
        TaskReadSort::ScheduledAtDesc => ApplicationTaskListSort::ScheduledAtDesc,
        TaskReadSort::DueAt => ApplicationTaskListSort::DueAt,
        TaskReadSort::DueAtDesc => ApplicationTaskListSort::DueAtDesc,
        TaskReadSort::CreatedAt => ApplicationTaskListSort::CreatedAt,
        TaskReadSort::CreatedAtDesc => ApplicationTaskListSort::CreatedAtDesc,
        TaskReadSort::UpdatedAt => ApplicationTaskListSort::UpdatedAt,
        TaskReadSort::UpdatedAtDesc => ApplicationTaskListSort::UpdatedAtDesc,
    }
}

fn parse_list_tasks_query(raw_query: Option<&str>) -> Result<ListTasksQuery, ApiError> {
    let mut query = ListTasksQuery::default();
    let mut scalar_parameters = BTreeSet::new();
    let Some(raw_query) = raw_query else {
        return Ok(query);
    };
    if raw_query.is_empty() {
        return Ok(query);
    }
    if raw_query.len() > MAX_TASK_READ_QUERY_BYTES {
        return Err(KanbanError::InvalidInput(format!(
            "task-read raw query exceeds {MAX_TASK_READ_QUERY_BYTES} bytes"
        ))
        .into());
    }
    let pairs = raw_query.split('&').collect::<Vec<_>>();
    if pairs.len() > MAX_TASK_READ_QUERY_PAIRS {
        return Err(KanbanError::InvalidInput(format!(
            "task-read query exceeds {MAX_TASK_READ_QUERY_PAIRS} parameter pairs"
        ))
        .into());
    }
    for pair in pairs {
        let (encoded_key, encoded_value) = pair.split_once('=').unwrap_or((pair, ""));
        let key = decode_query_component(encoded_key)?;
        let value = decode_query_component(encoded_value)?;
        match key.as_str() {
            "status" => {
                let status = ApiTaskStatus::from_str(value.trim()).map_err(|()| {
                    KanbanError::InvalidInput(format!("unknown status filter: {value}"))
                })?;
                push_repeated(&mut query.status, status, "status", MAX_TASK_READ_STATUSES)?;
            }
            "priority" => {
                let priority = value
                    .trim()
                    .parse::<i64>()
                    .ok()
                    .and_then(|value| ApiTaskPriority::try_from(value).ok())
                    .ok_or_else(|| {
                        KanbanError::InvalidInput("priority must be between 0 and 3".to_owned())
                    })?;
                push_repeated(
                    &mut query.priority,
                    priority,
                    "priority",
                    MAX_TASK_READ_PRIORITIES,
                )?;
            }
            "label" => {
                if value.chars().count() > MAX_TASK_READ_LABEL_CHARS {
                    return Err(KanbanError::InvalidInput(format!(
                        "label exceeds {MAX_TASK_READ_LABEL_CHARS} characters"
                    ))
                    .into());
                }
                let label = TaskReadLabel::new(value).ok_or_else(|| {
                    KanbanError::InvalidInput(
                        "label must contain a non-whitespace character".to_owned(),
                    )
                })?;
                push_repeated(&mut query.label, label, "label", MAX_TASK_READ_LABELS)?;
            }
            "plan_filter" => {
                let filter = TaskReadPlanFilter::from_str(value.trim()).map_err(|()| {
                    KanbanError::InvalidInput(format!("unknown plan_filter: {value}"))
                })?;
                push_repeated(
                    &mut query.plan_filter,
                    filter,
                    "plan_filter",
                    MAX_TASK_READ_PLAN_FILTERS,
                )?;
            }
            "assignee" => {
                scalar(&mut scalar_parameters, "assignee")?;
                query.assignee = bounded_optional(value, "assignee", MAX_TASK_READ_ASSIGNEE_CHARS)?;
            }
            "q" => {
                scalar(&mut scalar_parameters, "q")?;
                query.q = bounded_optional(value, "q", MAX_TASK_READ_Q_CHARS)?;
            }
            "include_archived" => {
                scalar(&mut scalar_parameters, "include_archived")?;
                query.include_archived = value.parse::<bool>().map_err(|_| {
                    KanbanError::InvalidInput(format!("invalid include_archived: {value}"))
                })?;
            }
            "limit" => {
                scalar(&mut scalar_parameters, "limit")?;
                query.limit = value
                    .parse::<usize>()
                    .map_err(|_| KanbanError::InvalidInput(format!("invalid limit: {value}")))?;
                if query.limit > MAX_TASK_READ_LIMIT {
                    return Err(KanbanError::InvalidInput(format!(
                        "limit must be <= {MAX_TASK_READ_LIMIT}"
                    ))
                    .into());
                }
            }
            "offset" => {
                scalar(&mut scalar_parameters, "offset")?;
                query.offset = value
                    .parse::<usize>()
                    .map_err(|_| KanbanError::InvalidInput(format!("invalid offset: {value}")))?;
                if query.offset > i64::MAX as usize {
                    return Err(KanbanError::InvalidInput(
                        "offset exceeds the supported range".to_owned(),
                    )
                    .into());
                }
            }
            "sort" => {
                scalar(&mut scalar_parameters, "sort")?;
                query.sort = TaskReadSort::from_str(value.trim()).map_err(|()| {
                    KanbanError::InvalidInput(format!("unsupported sort: {value}"))
                })?;
            }
            _ => {
                return Err(KanbanError::InvalidInput(format!(
                    "unknown task-read query parameter: {key}"
                ))
                .into());
            }
        }
    }
    Ok(query)
}

fn decode_query_component(encoded: &str) -> Result<String, ApiError> {
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' => {
                let high = bytes
                    .get(index + 1)
                    .and_then(|byte| hex_value(*byte))
                    .ok_or_else(|| {
                        KanbanError::InvalidInput("malformed percent-encoding in query".to_owned())
                    })?;
                let low = bytes
                    .get(index + 2)
                    .and_then(|byte| hex_value(*byte))
                    .ok_or_else(|| {
                        KanbanError::InvalidInput("malformed percent-encoding in query".to_owned())
                    })?;
                decoded.push((high << 4) | low);
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded)
        .map_err(|_| KanbanError::InvalidInput("query is not valid UTF-8".to_owned()).into())
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn scalar(seen: &mut BTreeSet<&'static str>, name: &'static str) -> Result<(), ApiError> {
    if seen.insert(name) {
        Ok(())
    } else {
        Err(KanbanError::InvalidInput(format!("duplicate scalar query parameter: {name}")).into())
    }
}

fn push_repeated<T: PartialEq>(
    values: &mut Vec<T>,
    value: T,
    name: &'static str,
    maximum: usize,
) -> Result<(), ApiError> {
    if values.len() >= maximum {
        return Err(KanbanError::InvalidInput(format!(
            "too many {name} query parameters: maximum is {maximum}"
        ))
        .into());
    }
    if values.contains(&value) {
        return Err(KanbanError::InvalidInput(format!(
            "duplicate repeated query parameter value: {name}"
        ))
        .into());
    }
    values.push(value);
    Ok(())
}

fn bounded_optional(
    value: String,
    name: &'static str,
    maximum_chars: usize,
) -> Result<Option<String>, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > maximum_chars {
        return Err(KanbanError::InvalidInput(format!(
            "{name} exceeds {maximum_chars} characters"
        ))
        .into());
    }
    Ok(Some(value.to_owned()))
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

fn api_run(run: RunRecord) -> Result<ApiRun, ApiError> {
    let metadata = serde_json::from_str(&run.metadata_json).map_err(|error| {
        KanbanError::Storage(format!("stored run metadata is invalid JSON: {error}"))
    })?;
    Ok(ApiRun {
        id: run.id,
        task_id: run.task_id,
        status: match run.status {
            RunStatus::Running => ApiRunStatus::Running,
            RunStatus::Succeeded => ApiRunStatus::Succeeded,
            RunStatus::Failed => ApiRunStatus::Failed,
            RunStatus::Canceled => ApiRunStatus::Canceled,
            RunStatus::Expired => ApiRunStatus::Expired,
        },
        worker_profile: run.worker_profile,
        worker_pid: run.worker_pid,
        claim_owner: run.claim_owner,
        started_at: run.started_at,
        finished_at: run.finished_at,
        exit_code: run.exit_code,
        summary: run.summary,
        error: run.error,
        has_log: run.log_path.is_some(),
        metadata,
    })
}

fn api_execution_plan(plan: ExecutionPlanRecord) -> ApiExecutionPlan {
    ApiExecutionPlan {
        board_id: plan.board_id,
        task_id: plan.task_id,
        state: match plan.state {
            ExecutionPlanState::Unplanned => ApiExecutionPlanState::Unplanned,
            ExecutionPlanState::Planned => ApiExecutionPlanState::Planned,
            ExecutionPlanState::NotRequired => ApiExecutionPlanState::NotRequired,
        },
        reason: plan.reason,
        updated_by: plan.updated_by,
        updated_at: plan.updated_at,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_list_query_parses_repeated_filters_and_encoded_search() {
        let query = parse_list_tasks_query(Some(
            "status=ready&status=blocked&priority=0&priority=2&q=a%20%26%20b&limit=25&offset=50&sort=-updated_at",
        ))
        .unwrap();
        assert_eq!(
            query.status,
            vec![ApiTaskStatus::Ready, ApiTaskStatus::Blocked]
        );
        assert_eq!(
            query
                .priority
                .into_iter()
                .map(ApiTaskPriority::get)
                .collect::<Vec<_>>(),
            vec![0, 2]
        );
        assert_eq!(query.q.as_deref(), Some("a & b"));
        assert_eq!(query.limit, 25);
        assert_eq!(query.offset, 50);
        assert_eq!(query.sort, TaskReadSort::UpdatedAtDesc);
    }

    #[test]
    fn task_list_query_rejects_duplicate_and_unknown_parameters() {
        assert!(
            parse_list_tasks_query(Some("limit=10&limit=20"))
                .unwrap_err()
                .0
                .to_string()
                .contains("duplicate")
        );
        assert!(parse_list_tasks_query(Some("future=true")).is_err());
        assert!(parse_list_tasks_query(Some("q=%ZZ")).is_err());
    }
}
