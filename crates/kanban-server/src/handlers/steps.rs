use super::shared::actor;
use crate::error::{ApiError, extractor_error};
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
};
use kanban_contract::{
    ApiExecutionPlan, ApiStepStatus, ApiTaskStep, ApiTaskSteps, CompleteStepPath,
    CompleteStepRequest, CompleteStepResponse, CreateStepPath, CreateStepRequest,
    CreateStepResponse, ListStepsPath, ListStepsResponse, MarkExecutionPlanNotRequiredPath,
    MarkExecutionPlanNotRequiredRequest, MarkExecutionPlanNotRequiredResponse, RemoveStepPath,
    RemoveStepResponse, ReopenStepPath, ReopenStepRequest, ReopenStepResponse, SkipStepPath,
    SkipStepRequest, SkipStepResponse, UpdateStepPath, UpdateStepRequest, UpdateStepResponse,
};
pub(crate) async fn list_steps(
    State(state): State<AppState>,
    Path(path): Path<ListStepsPath>,
) -> Result<Json<ListStepsResponse>, ApiError> {
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &path.task_id)?;
    let steps = kanban_sqlite::api::list_steps(state.db_path(), &task.board_id, &path.task_id)?;
    let execution_plan =
        kanban_sqlite::api::execution_plan(state.db_path(), &task.board_id, &path.task_id)?;
    Ok(Json(ListStepsResponse {
        data: steps_data(task, steps, execution_plan)?,
    }))
}
pub(crate) async fn create_step(
    State(state): State<AppState>,
    Path(path): Path<CreateStepPath>,
    headers: HeaderMap,
    body: Result<Json<CreateStepRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CreateStepResponse>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let parent = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &path.task_id)?;
    kanban_sqlite::api::create_step(
        state.db_path(),
        &parent.board_id,
        &actor,
        &path.task_id,
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
        Json(CreateStepResponse {
            data: steps_dto(&state, &path.task_id)?,
        }),
    ))
}
pub(crate) async fn update_step(
    State(state): State<AppState>,
    Path(path): Path<UpdateStepPath>,
    headers: HeaderMap,
    body: Result<Json<UpdateStepRequest>, JsonRejection>,
) -> Result<Json<UpdateStepResponse>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let parent = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &path.task_id)?;
    kanban_sqlite::api::update_step(
        state.db_path(),
        &parent.board_id,
        &actor,
        &path.task_id,
        &path.step_id,
        kanban_sqlite::api::UpdateStepInput {
            title: body.title,
            body: body.body.map(Some),
            linked_task_ref: body.linked_task_ref,
            unlink_task: body.unlink_task,
            position: body.position,
            required: body.required,
        },
    )?;
    Ok(Json(UpdateStepResponse {
        data: steps_dto(&state, &path.task_id)?,
    }))
}
pub(crate) async fn remove_step(
    State(state): State<AppState>,
    Path(path): Path<RemoveStepPath>,
    headers: HeaderMap,
) -> Result<Json<RemoveStepResponse>, ApiError> {
    let actor = actor(None, &headers, &state);
    let parent = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &path.task_id)?;
    kanban_sqlite::api::remove_step(
        state.db_path(),
        &parent.board_id,
        &actor,
        &path.task_id,
        &path.step_id,
    )?;
    Ok(Json(RemoveStepResponse {
        data: steps_dto(&state, &path.task_id)?,
    }))
}
pub(crate) async fn complete_step(
    State(state): State<AppState>,
    Path(path): Path<CompleteStepPath>,
    headers: HeaderMap,
    body: Result<Json<CompleteStepRequest>, JsonRejection>,
) -> Result<Json<CompleteStepResponse>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let parent = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &path.task_id)?;
    kanban_sqlite::api::complete_step(
        state.db_path(),
        &parent.board_id,
        &actor,
        &path.task_id,
        &path.step_id,
        &body.note,
    )?;
    Ok(Json(CompleteStepResponse {
        data: steps_dto(&state, &path.task_id)?,
    }))
}
pub(crate) async fn skip_step(
    State(state): State<AppState>,
    Path(path): Path<SkipStepPath>,
    headers: HeaderMap,
    body: Result<Json<SkipStepRequest>, JsonRejection>,
) -> Result<Json<SkipStepResponse>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let parent = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &path.task_id)?;
    kanban_sqlite::api::skip_step(
        state.db_path(),
        &parent.board_id,
        &actor,
        &path.task_id,
        &path.step_id,
        &body.reason,
    )?;
    Ok(Json(SkipStepResponse {
        data: steps_dto(&state, &path.task_id)?,
    }))
}
pub(crate) async fn reopen_step(
    State(state): State<AppState>,
    Path(path): Path<ReopenStepPath>,
    headers: HeaderMap,
    body: Result<Json<ReopenStepRequest>, JsonRejection>,
) -> Result<Json<ReopenStepResponse>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let parent = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &path.task_id)?;
    kanban_sqlite::api::reopen_step(
        state.db_path(),
        &parent.board_id,
        &actor,
        &path.task_id,
        &path.step_id,
        &body.reason,
    )?;
    Ok(Json(ReopenStepResponse {
        data: steps_dto(&state, &path.task_id)?,
    }))
}
pub(crate) async fn mark_execution_plan_not_required(
    State(state): State<AppState>,
    Path(path): Path<MarkExecutionPlanNotRequiredPath>,
    headers: HeaderMap,
    body: Result<Json<MarkExecutionPlanNotRequiredRequest>, JsonRejection>,
) -> Result<Json<MarkExecutionPlanNotRequiredResponse>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &path.task_id)?;
    let plan = kanban_sqlite::api::mark_execution_plan_not_required(
        state.db_path(),
        &task.board_id,
        &actor,
        &path.task_id,
        &body.reason,
    )?;
    Ok(Json(MarkExecutionPlanNotRequiredResponse {
        data: api_execution_plan(plan),
    }))
}
fn api_step(step: kanban_sqlite::api::TaskStepRecord) -> Result<ApiTaskStep, ApiError> {
    let status = match step.status {
        kanban_sqlite::api::StepStatus::Todo => ApiStepStatus::Todo,
        kanban_sqlite::api::StepStatus::Done => ApiStepStatus::Done,
        kanban_sqlite::api::StepStatus::Skipped => ApiStepStatus::Skipped,
    };
    Ok(ApiTaskStep {
        id: step.id,
        parent_task_id: step.parent_task_id,
        title: step.title,
        body: step.body,
        linked_task: step
            .linked_task
            .map(crate::dto::api_task_from_record)
            .transpose()?,
        position: step.position,
        required: step.required,
        status,
        resolution_note: step.resolution_note,
        resolved_by: step.resolved_by,
        resolved_at: step.resolved_at,
        created_by: step.created_by,
        created_at: step.created_at,
        updated_by: step.updated_by,
        updated_at: step.updated_at,
    })
}
fn api_execution_plan(plan: kanban_sqlite::api::TaskExecutionPlanRecord) -> ApiExecutionPlan {
    ApiExecutionPlan {
        board_id: plan.board_id,
        task_id: plan.task_id,
        state: crate::dto::api_execution_plan_state_from_record(plan.state),
        reason: plan.reason,
        updated_by: plan.updated_by,
        updated_at: plan.updated_at,
    }
}
fn steps_dto(state: &AppState, task_id: &str) -> Result<ApiTaskSteps, ApiError> {
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), task_id)?;
    let steps = kanban_sqlite::api::list_steps(state.db_path(), &task.board_id, task_id)?;
    let execution_plan =
        kanban_sqlite::api::execution_plan(state.db_path(), &task.board_id, task_id)?;
    steps_data(task, steps, execution_plan)
}
fn steps_data(
    task: kanban_sqlite::api::TaskRecord,
    steps: Vec<kanban_sqlite::api::TaskStepRecord>,
    execution_plan: kanban_sqlite::api::TaskExecutionPlanRecord,
) -> Result<ApiTaskSteps, ApiError> {
    let steps = steps
        .into_iter()
        .map(api_step)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ApiTaskSteps {
        task_id: task.id,
        steps,
        execution_plan: api_execution_plan(execution_plan),
    })
}
