use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;

use crate::dto::{Envelope, TaskExecutionPlanDto, TaskStepsDto};
use crate::error::{ApiError, extractor_error};
use crate::state::AppState;

use super::shared::actor;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateStepBody {
    title: String,
    body: Option<String>,
    #[serde(alias = "linked_task_id")]
    linked_task_ref: Option<String>,
    position: Option<i64>,
    #[serde(default = "default_required")]
    required: bool,
    actor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateStepBody {
    title: Option<String>,
    body: Option<String>,
    #[serde(alias = "linked_task_id")]
    linked_task_ref: Option<String>,
    #[serde(default)]
    unlink_task: bool,
    position: Option<i64>,
    required: Option<bool>,
    actor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResolveStepDoneBody {
    note: String,
    actor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResolveStepReasonBody {
    reason: String,
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

pub(crate) async fn list_steps(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Envelope<TaskStepsDto>>, ApiError> {
    Ok(Json(Envelope {
        data: steps_dto(&state, &task_id)?,
        meta: None,
    }))
}

pub(crate) async fn create_step(
    State(state): State<AppState>,
    Path(parent_task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<CreateStepBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Envelope<TaskStepsDto>>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let parent = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &parent_task_id)?;
    kanban_sqlite::api::create_step(
        state.db_path(),
        &parent.board_id,
        &actor,
        &parent_task_id,
        kanban_sqlite::api::CreateStepInput {
            title: body.title,
            body: body.body,
            linked_task_ref: body.linked_task_ref,
            position: body.position,
            required: body.required,
        },
    )?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: steps_dto(&state, &parent_task_id)?,
            meta: None,
        }),
    ))
}

pub(crate) async fn update_step(
    State(state): State<AppState>,
    Path((parent_task_id, step_id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Result<Json<UpdateStepBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskStepsDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let parent = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &parent_task_id)?;
    kanban_sqlite::api::update_step(
        state.db_path(),
        &parent.board_id,
        &actor,
        &parent_task_id,
        &step_id,
        kanban_sqlite::api::UpdateStepInput {
            title: body.title,
            body: body.body.map(Some),
            linked_task_ref: body.linked_task_ref,
            unlink_task: body.unlink_task,
            position: body.position,
            required: body.required,
        },
    )?;
    Ok(Json(Envelope {
        data: steps_dto(&state, &parent_task_id)?,
        meta: None,
    }))
}

pub(crate) async fn remove_step(
    State(state): State<AppState>,
    Path((parent_task_id, step_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<Envelope<TaskStepsDto>>, ApiError> {
    let actor = actor(None, &headers, &state);
    let parent = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &parent_task_id)?;
    kanban_sqlite::api::remove_step(
        state.db_path(),
        &parent.board_id,
        &actor,
        &parent_task_id,
        &step_id,
    )?;
    Ok(Json(Envelope {
        data: steps_dto(&state, &parent_task_id)?,
        meta: None,
    }))
}

pub(crate) async fn complete_step(
    State(state): State<AppState>,
    Path((parent_task_id, step_id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Result<Json<ResolveStepDoneBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskStepsDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let parent = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &parent_task_id)?;
    kanban_sqlite::api::complete_step(
        state.db_path(),
        &parent.board_id,
        &actor,
        &parent_task_id,
        &step_id,
        &body.note,
    )?;
    Ok(Json(Envelope {
        data: steps_dto(&state, &parent_task_id)?,
        meta: None,
    }))
}

pub(crate) async fn skip_step(
    State(state): State<AppState>,
    Path((parent_task_id, step_id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Result<Json<ResolveStepReasonBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskStepsDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let parent = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &parent_task_id)?;
    kanban_sqlite::api::skip_step(
        state.db_path(),
        &parent.board_id,
        &actor,
        &parent_task_id,
        &step_id,
        &body.reason,
    )?;
    Ok(Json(Envelope {
        data: steps_dto(&state, &parent_task_id)?,
        meta: None,
    }))
}

pub(crate) async fn reopen_step(
    State(state): State<AppState>,
    Path((parent_task_id, step_id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Result<Json<ResolveStepReasonBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskStepsDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let parent = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &parent_task_id)?;
    kanban_sqlite::api::reopen_step(
        state.db_path(),
        &parent.board_id,
        &actor,
        &parent_task_id,
        &step_id,
        &body.reason,
    )?;
    Ok(Json(Envelope {
        data: steps_dto(&state, &parent_task_id)?,
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
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &task_id)?;
    let plan = kanban_sqlite::api::mark_execution_plan_not_required(
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

fn steps_dto(state: &AppState, task_id: &str) -> Result<TaskStepsDto, ApiError> {
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), task_id)?;
    let steps = kanban_sqlite::api::list_steps(state.db_path(), &task.board_id, task_id)?
        .into_iter()
        .map(crate::dto::TaskStepDto::from)
        .collect();
    let execution_plan =
        kanban_sqlite::api::execution_plan(state.db_path(), &task.board_id, task_id)?;
    Ok(TaskStepsDto {
        task_id: task.id,
        steps,
        execution_plan: TaskExecutionPlanDto::from(execution_plan),
    })
}
