use axum::{
    Json,
    extract::{
        Path, Query, RawQuery, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{HeaderMap, StatusCode},
};
use kanban_core::TaskStatus;
use serde::Deserialize;
use serde_json::json;

use crate::dto::{Envelope, TaskDto};
use crate::error::{ApiError, extractor_error, invalid_input, validate_page_bounds};
use crate::state::AppState;

use super::shared::{
    actor, metadata_json, parse_status_filters, parse_task_sort, patch_from_value,
    retry_policy_from_value,
};

#[derive(Debug, Deserialize)]
pub(crate) struct TaskListQuery {
    #[serde(default)]
    include_archived: bool,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
    #[serde(default)]
    assignee: Option<String>,
    q: Option<String>,
    search: Option<String>,
    sort: Option<String>,
    label: Option<String>,
}

fn default_limit() -> usize {
    100
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateTaskBody {
    title: String,
    description: Option<String>,
    status: Option<TaskStatus>,
    assignee: Option<String>,
    #[serde(default)]
    priority: i64,
    scheduled_at: Option<i64>,
    due_at: Option<i64>,
    max_retries: Option<i64>,
    metadata: Option<serde_json::Value>,
    #[serde(default)]
    depends_on: Vec<String>,
    actor: Option<String>,
}

pub(crate) async fn list_tasks(
    State(state): State<AppState>,
    Path(board): Path<String>,
    RawQuery(raw_query): RawQuery,
    query: Result<Query<TaskListQuery>, QueryRejection>,
) -> Result<Json<Envelope<Vec<TaskDto>>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(
        query.limit,
        kanban_sqlite::MAX_TASK_LIST_LIMIT,
        query.offset,
    )?;
    if query
        .label
        .as_deref()
        .is_some_and(|label| !label.trim().is_empty())
    {
        return Err(invalid_input("label filter is not supported yet"));
    }
    let statuses = parse_status_filters(raw_query.as_deref())?;
    let assignee = query
        .assignee
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let search = query
        .q
        .as_deref()
        .or(query.search.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let sort = parse_task_sort(query.sort.as_deref())?;
    let page = kanban_sqlite::list_tasks_page(
        state.db_path(),
        &board,
        kanban_sqlite::TaskListOptions {
            statuses,
            include_archived: query.include_archived,
            assignee,
            search,
            sort,
            limit: query.limit,
            offset: query.offset,
        },
    )?;
    let tasks = page.tasks.into_iter().map(TaskDto::from).collect();
    Ok(Json(Envelope {
        data: tasks,
        meta: Some(json!({ "limit": query.limit, "offset": query.offset, "total": page.total })),
    }))
}

pub(crate) async fn create_task(
    State(state): State<AppState>,
    Path(board): Path<String>,
    headers: HeaderMap,
    body: Result<Json<CreateTaskBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Envelope<TaskDto>>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let input = kanban_sqlite::CreateTask {
        title: body.title,
        description: body.description,
        status: body.status,
        assignee: body.assignee,
        priority: body.priority,
        scheduled_at: body.scheduled_at,
        due_at: body.due_at,
        metadata_json: metadata_json(body.metadata)?,
    };
    let mut task = kanban_sqlite::create_task_with_dependencies(
        state.db_path(),
        &board,
        &actor,
        input,
        &body.depends_on,
    )?;
    if body.max_retries.is_some() {
        task = kanban_sqlite::set_task_retry_policy_by_id(
            state.db_path(),
            &actor,
            &task.id,
            body.max_retries,
        )?;
    }
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: TaskDto::from(task),
            meta: None,
        }),
    ))
}

pub(crate) async fn get_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::get_task_by_id_global(
            state.db_path(),
            &task_id,
        )?),
        meta: None,
    }))
}

pub(crate) async fn update_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<serde_json::Value>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let object = body
        .as_object()
        .ok_or_else(|| invalid_input("request body must be a JSON object"))?;
    for forbidden in ["status", "claim_token", "current_run_id", "completed_at"] {
        if object.contains_key(forbidden) {
            return Err(invalid_input(format!("{forbidden} cannot be patched")));
        }
    }
    let body_actor = object.get("actor").and_then(|value| value.as_str());
    let actor = actor(body_actor, &headers, &state);
    let retry_policy = retry_policy_from_value(object)?;
    let patch = patch_from_value(object)?;
    let mut task = kanban_sqlite::update_task_by_id(state.db_path(), &actor, &task_id, patch)?;
    if let Some(max_retries) = retry_policy {
        task = kanban_sqlite::set_task_retry_policy_by_id(
            state.db_path(),
            &actor,
            &task_id,
            max_retries,
        )?;
    }
    Ok(Json(Envelope {
        data: TaskDto::from(task),
        meta: None,
    }))
}
