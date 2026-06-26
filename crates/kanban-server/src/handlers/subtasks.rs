use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
};
use kanban_core::TaskStatus;
use serde::Deserialize;

use crate::dto::{Envelope, TaskExecutionPlanDto, TaskSubtasksDto};
use crate::error::{ApiError, extractor_error};
use crate::state::AppState;

use super::shared::{actor, metadata_json};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateSubtaskBody {
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
    position: Option<i64>,
    #[serde(default = "default_required")]
    required: bool,
    actor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AttachSubtaskBody {
    child_task_id: String,
    position: Option<i64>,
    #[serde(default = "default_required")]
    required: bool,
    actor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateSubtaskBody {
    position: Option<i64>,
    required: Option<bool>,
    actor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MarkExecutionPlanNotRequiredBody {
    reason: String,
    actor: Option<String>,
}

fn default_required() -> bool {
    true
}

pub(crate) async fn list_subtasks(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Envelope<TaskSubtasksDto>>, ApiError> {
    Ok(Json(Envelope {
        data: subtasks_dto(&state, &task_id)?,
        meta: None,
    }))
}

pub(crate) async fn create_subtask(
    State(state): State<AppState>,
    Path(parent_task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<CreateSubtaskBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Envelope<TaskSubtasksDto>>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let parent = kanban_sqlite::get_task_by_id_global(state.db_path(), &parent_task_id)?;
    kanban_sqlite::create_subtask(
        state.db_path(),
        &parent.board_id,
        &actor,
        &parent_task_id,
        kanban_sqlite::CreateSubtaskInput {
            task: kanban_sqlite::CreateTask {
                title: body.title,
                description: body.description,
                status: body.status,
                assignee: body.assignee,
                priority: body.priority,
                scheduled_at: body.scheduled_at,
                due_at: body.due_at,
                max_retries: body.max_retries,
                metadata_json: metadata_json(body.metadata)?,
            },
            position: body.position,
            required: body.required,
        },
    )?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: subtasks_dto(&state, &parent_task_id)?,
            meta: None,
        }),
    ))
}

pub(crate) async fn attach_subtask(
    State(state): State<AppState>,
    Path(parent_task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<AttachSubtaskBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Envelope<TaskSubtasksDto>>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let parent = kanban_sqlite::get_task_by_id_global(state.db_path(), &parent_task_id)?;
    let child = kanban_sqlite::get_task_by_id_global(state.db_path(), &body.child_task_id)?;
    if child.board_id != parent.board_id {
        return Err(crate::error::invalid_input(
            "cross-board subtask child is not allowed".to_owned(),
        ));
    }
    kanban_sqlite::attach_subtask(
        state.db_path(),
        &parent.board_id,
        &actor,
        &parent_task_id,
        kanban_sqlite::AttachSubtaskInput {
            child_ref: child.id,
            position: body.position,
            required: body.required,
        },
    )?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: subtasks_dto(&state, &parent_task_id)?,
            meta: None,
        }),
    ))
}

pub(crate) async fn update_subtask(
    State(state): State<AppState>,
    Path((parent_task_id, child_task_id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Result<Json<UpdateSubtaskBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskSubtasksDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let parent = kanban_sqlite::get_task_by_id_global(state.db_path(), &parent_task_id)?;
    kanban_sqlite::update_subtask(
        state.db_path(),
        &parent.board_id,
        &actor,
        &parent_task_id,
        &child_task_id,
        kanban_sqlite::UpdateSubtaskInput {
            position: body.position,
            required: body.required,
        },
    )?;
    Ok(Json(Envelope {
        data: subtasks_dto(&state, &parent_task_id)?,
        meta: None,
    }))
}

pub(crate) async fn remove_subtask(
    State(state): State<AppState>,
    Path((parent_task_id, child_task_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<Envelope<TaskSubtasksDto>>, ApiError> {
    let actor = actor(None, &headers, &state);
    let parent = kanban_sqlite::get_task_by_id_global(state.db_path(), &parent_task_id)?;
    kanban_sqlite::detach_subtask(
        state.db_path(),
        &parent.board_id,
        &actor,
        &parent_task_id,
        &child_task_id,
    )?;
    Ok(Json(Envelope {
        data: subtasks_dto(&state, &parent_task_id)?,
        meta: None,
    }))
}

pub(crate) async fn mark_execution_plan_not_required(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<MarkExecutionPlanNotRequiredBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskExecutionPlanDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    let plan = kanban_sqlite::mark_execution_plan_not_required(
        state.db_path(),
        &task.board_id,
        &actor,
        &task_id,
        &body.reason,
    )?;
    Ok(Json(Envelope {
        data: TaskExecutionPlanDto::from(plan),
        meta: None,
    }))
}

fn subtasks_dto(state: &AppState, task_id: &str) -> Result<TaskSubtasksDto, ApiError> {
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), task_id)?;
    let subtasks = kanban_sqlite::list_subtasks(state.db_path(), &task.board_id, task_id)?
        .into_iter()
        .map(crate::dto::TaskSubtaskDto::from)
        .collect();
    let execution_plan = kanban_sqlite::execution_plan(state.db_path(), &task.board_id, task_id)?;
    Ok(TaskSubtasksDto {
        task_id: task.id,
        subtasks,
        execution_plan: TaskExecutionPlanDto::from(execution_plan),
    })
}
