use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
};
use kanban_core::{KanbanError, TaskStatus};

use crate::dto::{ClaimDto, Envelope, RunDto, TaskDto};
use crate::error::{ApiError, extractor_error, invalid_input};
use crate::state::AppState;

use super::shared::{
    ActorBody, ArchiveBody, BlockBody, ClaimBody, HeartbeatBody, ReclaimBody, SpecifyBody,
    TokenBody, actor, metadata_json, optional_json_body,
};

pub(crate) async fn specify_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<SpecifyBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::specify_task(
            state.db_path(),
            &actor,
            &task_id,
            body.description,
            body.scheduled_at,
        )?),
        meta: None,
    }))
}

pub(crate) async fn promote_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<ActorBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let body = optional_json_body(body)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::promote_task(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
        )?),
        meta: None,
    }))
}

pub(crate) async fn claim_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<ClaimBody>, JsonRejection>,
) -> Result<Json<Envelope<ClaimDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    if body.ttl_ms <= 0 {
        return Err(invalid_input("ttl_ms must be positive"));
    }
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    let worker_profile = body.worker_profile.as_deref().unwrap_or("manual");
    let metadata_json = metadata_json(body.metadata)?;
    let claim = kanban_sqlite::claim_task_with_profile_and_metadata(
        state.db_path(),
        &task.board_id,
        &actor,
        &task_id,
        body.ttl_ms,
        worker_profile,
        &metadata_json,
    )?;
    let run = kanban_sqlite::list_runs(state.db_path(), &task.board_id, Some(&task_id))?
        .into_iter()
        .find(|run| run.id == claim.run_id)
        .ok_or_else(|| KanbanError::NotFound(format!("run {}", claim.run_id)))?;
    Ok(Json(Envelope {
        data: ClaimDto {
            claim_token: claim.claim_token,
            claim_expires_at: claim.task.claim_expires_at,
            task: TaskDto::from(claim.task),
            run: RunDto::from(run),
        },
        meta: None,
    }))
}

pub(crate) async fn reclaim_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<ReclaimBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let body = optional_json_body(body)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::reclaim_task_to(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            body.force,
            body.to_status.unwrap_or(TaskStatus::Ready),
            body.reason.as_deref(),
        )?),
        meta: None,
    }))
}

pub(crate) async fn heartbeat_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<HeartbeatBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    if body.ttl_ms <= 0 {
        return Err(invalid_input("ttl_ms must be positive"));
    }
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    if task.status == TaskStatus::Running
        && task.claim_token.as_deref() != Some(body.claim_token.as_str())
    {
        return Err(ApiError(KanbanError::InvalidTransition(
            "claim token mismatch".to_owned(),
        )));
    }
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::heartbeat_task_with_note(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            &body.claim_token,
            body.ttl_ms,
            body.note.as_deref(),
        )?),
        meta: None,
    }))
}

pub(crate) async fn complete_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<TokenBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    let result_json = body.result.map(|value| value.to_string());
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::complete_task_with_summary_and_result(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            body.claim_token.as_deref(),
            body.force,
            body.summary.as_deref(),
            result_json.as_deref(),
        )?),
        meta: None,
    }))
}

pub(crate) async fn submit_review_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<TokenBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    if body.result.is_some() {
        return Err(invalid_input("submit-review result is not supported yet"));
    }
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::submit_review_task_with_summary(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            body.claim_token.as_deref(),
            body.force,
            body.summary.as_deref(),
        )?),
        meta: None,
    }))
}

pub(crate) async fn block_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<BlockBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::block_task(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            &body.reason,
            body.claim_token.as_deref(),
            body.force,
        )?),
        meta: None,
    }))
}

pub(crate) async fn unblock_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<ActorBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let body = optional_json_body(body)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::unblock_task(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
        )?),
        meta: None,
    }))
}

pub(crate) async fn archive_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<ArchiveBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let body = optional_json_body(body)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::archive_task(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            body.force,
        )?),
        meta: None,
    }))
}
