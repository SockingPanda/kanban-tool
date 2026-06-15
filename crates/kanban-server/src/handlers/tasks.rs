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

use crate::dto::{Envelope, LabelDto, TaskDto};
use crate::error::{ApiError, extractor_error, invalid_input, validate_page_bounds};
use crate::state::AppState;

use super::shared::{
    actor, metadata_json, parse_priority_filters, parse_status_filters, parse_task_sort,
    patch_from_value, retry_policy_from_value,
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
}

#[derive(Debug, Deserialize)]
pub(crate) struct LabelSuggestionQuery {
    #[serde(default = "default_label_suggestion_limit")]
    limit: usize,
    #[serde(default = "default_label_suggestion_atom_limit")]
    atom_limit: usize,
    #[serde(default = "default_label_suggestion_min_score")]
    min_score: f32,
}

fn default_limit() -> usize {
    100
}

fn default_label_suggestion_limit() -> usize {
    kanban_sqlite::LabelSuggestionOptions::default().limit
}

fn default_label_suggestion_atom_limit() -> usize {
    kanban_sqlite::LabelSuggestionOptions::default().atom_limit
}

fn default_label_suggestion_min_score() -> f32 {
    kanban_sqlite::LabelSuggestionOptions::default().min_score
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateTaskBody {
    title: String,
    description: Option<String>,
    status: Option<TaskStatus>,
    assignee: Option<String>,
    #[serde(default = "kanban_sqlite::default_priority")]
    priority: i64,
    scheduled_at: Option<i64>,
    due_at: Option<i64>,
    max_retries: Option<i64>,
    metadata: Option<serde_json::Value>,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    depends_on: Vec<String>,
    actor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateLabelBody {
    name: String,
    color: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AddTaskLabelBody {
    name: String,
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
    let statuses = parse_status_filters(raw_query.as_deref())?;
    let priorities = parse_priority_filters(raw_query.as_deref())?;
    let labels = parse_label_filters(raw_query.as_deref())?;
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
            priorities,
            labels,
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

pub(crate) fn parse_label_filters(raw_query: Option<&str>) -> Result<Vec<String>, ApiError> {
    let Some(raw_query) = raw_query else {
        return Ok(Vec::new());
    };
    let pairs = serde_urlencoded::from_str::<Vec<(String, String)>>(raw_query)
        .map_err(|error| invalid_input(error.to_string()))?;
    Ok(pairs
        .into_iter()
        .filter_map(|(key, value)| {
            (key == "label")
                .then(|| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        })
        .collect())
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
        max_retries: body.max_retries,
        metadata_json: metadata_json(body.metadata)?,
    };
    let task = kanban_sqlite::create_task_with_labels_and_dependencies(
        state.db_path(),
        &board,
        &actor,
        input,
        &body.labels,
        &body.depends_on,
    )?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: TaskDto::from(task),
            meta: None,
        }),
    ))
}

pub(crate) async fn list_board_labels(
    State(state): State<AppState>,
    Path(board): Path<String>,
) -> Result<Json<Envelope<Vec<LabelDto>>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::list_labels(state.db_path(), &board)?
            .into_iter()
            .map(LabelDto::from)
            .collect(),
        meta: None,
    }))
}

pub(crate) async fn create_board_label(
    State(state): State<AppState>,
    Path(board): Path<String>,
    body: Result<Json<CreateLabelBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Envelope<LabelDto>>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let label = kanban_sqlite::create_label(
        state.db_path(),
        &board,
        kanban_sqlite::CreateLabel {
            name: body.name,
            color: body.color,
        },
    )?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: LabelDto::from(label),
            meta: None,
        }),
    ))
}

pub(crate) async fn list_task_labels(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Envelope<Vec<LabelDto>>>, ApiError> {
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(Envelope {
        data: task.labels.into_iter().map(LabelDto::from).collect(),
        meta: None,
    }))
}

pub(crate) async fn suggest_task_labels(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    query: Result<Query<LabelSuggestionQuery>, QueryRejection>,
) -> Result<Json<Envelope<kanban_sqlite::LabelSuggestionResult>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(query.limit, kanban_sqlite::MAX_TASK_LIST_LIMIT, 0)?;
    validate_page_bounds(query.atom_limit, kanban_sqlite::MAX_TASK_LIST_LIMIT, 0)?;
    let options = kanban_sqlite::LabelSuggestionOptions {
        limit: query.limit,
        atom_limit: query.atom_limit,
        min_score: query.min_score,
    };
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    let result = suggest_task_labels_for_state(&state, &task.board_slug, &task_id, options)?;
    Ok(Json(Envelope {
        data: result,
        meta: None,
    }))
}

pub(crate) async fn add_task_label(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<AddTaskLabelBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Envelope<TaskDto>>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::add_task_label_by_id(state.db_path(), &actor, &task_id, &body.name)?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: TaskDto::from(task),
            meta: None,
        }),
    ))
}

fn suggest_task_labels_for_state(
    state: &AppState,
    board: &str,
    task_id: &str,
    options: kanban_sqlite::LabelSuggestionOptions,
) -> Result<kanban_sqlite::LabelSuggestionResult, ApiError> {
    #[cfg(feature = "vector-lancedb")]
    {
        if let Some(store) = super::shared::configured_lancedb_store(state)? {
            return kanban_sqlite::suggest_task_labels_with(
                state.db_path(),
                board,
                task_id,
                &store,
                options,
            )
            .map_err(ApiError::from);
        }
    }
    kanban_sqlite::suggest_task_labels(state.db_path(), board, task_id, options)
        .map_err(ApiError::from)
}

pub(crate) async fn remove_task_label(
    State(state): State<AppState>,
    Path((task_id, label_ref)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let actor = actor(None, &headers, &state);
    let task =
        kanban_sqlite::remove_task_label_by_id(state.db_path(), &actor, &task_id, &label_ref)?;
    Ok(Json(Envelope {
        data: TaskDto::from(task),
        meta: None,
    }))
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
    let mut patch = patch_from_value(object)?;
    patch.max_retries = retry_policy;
    let task = kanban_sqlite::update_task_by_id(state.db_path(), &actor, &task_id, patch)?;
    Ok(Json(Envelope {
        data: TaskDto::from(task),
        meta: None,
    }))
}
