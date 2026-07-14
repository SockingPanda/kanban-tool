use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
};
use kanban_contract::{
    ArchiveTaskPath, ArchiveTaskRequest, ArchiveTaskResponse, BlockTaskPath, BlockTaskRequest,
    BlockTaskResponse, ClaimTaskPath, ClaimTaskRequest, ClaimTaskResponse, CompleteTaskPath,
    CompleteTaskRequest, CompleteTaskResponse, DataEnvelope, HeartbeatTaskPath,
    HeartbeatTaskRequest, HeartbeatTaskResponse, PromoteTaskPath, PromoteTaskRequest,
    PromoteTaskResponse, ReclaimTargetStatus, ReclaimTaskPath, ReclaimTaskRequest,
    ReclaimTaskResponse, ReopenTaskPath, ReopenTaskRequest, ReopenTaskResponse, SpecifyTaskPath,
    SpecifyTaskRequest, SpecifyTaskResponse, SubmitReviewTaskPath, SubmitReviewTaskRequest,
    SubmitReviewTaskResponse, UnblockTaskPath, UnblockTaskRequest, UnblockTaskResponse,
};
use kanban_core::{KanbanError, TaskStatus};

use crate::dto::api_task_from_record;
use crate::error::{ApiError, extractor_error, invalid_input};
use crate::handlers::runs::api_run;
use crate::state::AppState;
use kanban_contract::ApiClaim;

use super::shared::{actor, metadata_json, optional_json_body};

pub(crate) async fn specify_task(
    State(state): State<AppState>,
    Path(SpecifyTaskPath { task_id }): Path<SpecifyTaskPath>,
    headers: HeaderMap,
    body: Result<Json<SpecifyTaskRequest>, JsonRejection>,
) -> Result<Json<SpecifyTaskResponse>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    Ok(Json(DataEnvelope::new(api_task_from_record(
        kanban_sqlite::api::specify_task(
            state.db_path(),
            &actor,
            &task_id,
            body.description,
            body.scheduled_at,
        )?,
    )?)))
}

pub(crate) async fn promote_task(
    State(state): State<AppState>,
    Path(PromoteTaskPath { task_id }): Path<PromoteTaskPath>,
    headers: HeaderMap,
    body: Result<Json<PromoteTaskRequest>, JsonRejection>,
) -> Result<Json<PromoteTaskResponse>, ApiError> {
    let body = optional_json_body(body)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(DataEnvelope::new(api_task_from_record(
        kanban_sqlite::api::promote_task(state.db_path(), &task.board_id, &actor, &task_id)?,
    )?)))
}

pub(crate) async fn claim_task(
    State(state): State<AppState>,
    Path(ClaimTaskPath { task_id }): Path<ClaimTaskPath>,
    headers: HeaderMap,
    body: Result<Json<ClaimTaskRequest>, JsonRejection>,
) -> Result<Json<ClaimTaskResponse>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    if body.ttl_ms <= 0 {
        return Err(invalid_input("ttl_ms must be positive"));
    }
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &task_id)?;
    let worker_profile = body.worker_profile.as_deref().unwrap_or("manual");
    let metadata_json = metadata_json(body.metadata)?;
    let claim = kanban_sqlite::api::claim_task_with_profile_and_metadata(
        state.db_path(),
        &task.board_id,
        &actor,
        &task_id,
        body.ttl_ms,
        worker_profile,
        &metadata_json,
    )?;
    let run = kanban_sqlite::api::list_runs(state.db_path(), &task.board_id, Some(&task_id))?
        .into_iter()
        .find(|run| run.id == claim.run_id)
        .ok_or_else(|| KanbanError::NotFound(format!("run {}", claim.run_id)))?;
    Ok(Json(DataEnvelope::new(ApiClaim {
        claim_token: claim.claim_token,
        claim_expires_at: claim.task.claim_expires_at,
        task: api_task_from_record(claim.task)?,
        run: api_run(run)?,
    })))
}

pub(crate) async fn reclaim_task(
    State(state): State<AppState>,
    Path(ReclaimTaskPath { task_id }): Path<ReclaimTaskPath>,
    headers: HeaderMap,
    body: Result<Json<ReclaimTaskRequest>, JsonRejection>,
) -> Result<Json<ReclaimTaskResponse>, ApiError> {
    let body = optional_json_body(body)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let to_status = match body.to_status.unwrap_or(ReclaimTargetStatus::Ready) {
        ReclaimTargetStatus::Ready => TaskStatus::Ready,
        ReclaimTargetStatus::Blocked => TaskStatus::Blocked,
    };
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(DataEnvelope::new(api_task_from_record(
        kanban_sqlite::api::reclaim_task_to(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            body.force,
            to_status,
            body.reason.as_deref(),
        )?,
    )?)))
}

pub(crate) async fn reopen_task(
    State(state): State<AppState>,
    Path(ReopenTaskPath { task_id }): Path<ReopenTaskPath>,
    headers: HeaderMap,
    body: Result<Json<ReopenTaskRequest>, JsonRejection>,
) -> Result<Json<ReopenTaskResponse>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    if body.reason.trim().is_empty() {
        Err(invalid_input("reopen reason is required"))?;
    }
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(DataEnvelope::new(api_task_from_record(
        kanban_sqlite::api::reopen_task(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            &body.reason,
        )?,
    )?)))
}

pub(crate) async fn heartbeat_task(
    State(state): State<AppState>,
    Path(HeartbeatTaskPath { task_id }): Path<HeartbeatTaskPath>,
    headers: HeaderMap,
    body: Result<Json<HeartbeatTaskRequest>, JsonRejection>,
) -> Result<Json<HeartbeatTaskResponse>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    if body.ttl_ms <= 0 {
        return Err(invalid_input("ttl_ms must be positive"));
    }
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &task_id)?;
    if task.status == TaskStatus::Running
        && task.claim_token.as_deref() != Some(body.claim_token.as_str())
    {
        return Err(ApiError(KanbanError::InvalidTransition(
            "claim token mismatch".to_owned(),
        )));
    }
    Ok(Json(DataEnvelope::new(api_task_from_record(
        kanban_sqlite::api::heartbeat_task_with_note(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            &body.claim_token,
            body.ttl_ms,
            body.note.as_deref(),
        )?,
    )?)))
}

pub(crate) async fn complete_task(
    State(state): State<AppState>,
    Path(CompleteTaskPath { task_id }): Path<CompleteTaskPath>,
    headers: HeaderMap,
    body: Result<Json<CompleteTaskRequest>, JsonRejection>,
) -> Result<Json<CompleteTaskResponse>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &task_id)?;
    let result_json = body.result.map(|value| value.to_string());
    Ok(Json(DataEnvelope::new(api_task_from_record(
        kanban_sqlite::api::complete_task_with_summary_and_result(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            body.claim_token.as_deref(),
            body.force,
            body.summary.as_deref(),
            result_json.as_deref(),
        )?,
    )?)))
}

pub(crate) async fn submit_review_task(
    State(state): State<AppState>,
    Path(SubmitReviewTaskPath { task_id }): Path<SubmitReviewTaskPath>,
    headers: HeaderMap,
    body: Result<Json<SubmitReviewTaskRequest>, JsonRejection>,
) -> Result<Json<SubmitReviewTaskResponse>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(DataEnvelope::new(api_task_from_record(
        kanban_sqlite::api::submit_review_task_with_summary(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            body.claim_token.as_deref(),
            body.force,
            body.summary.as_deref(),
        )?,
    )?)))
}

pub(crate) async fn block_task(
    State(state): State<AppState>,
    Path(BlockTaskPath { task_id }): Path<BlockTaskPath>,
    headers: HeaderMap,
    body: Result<Json<BlockTaskRequest>, JsonRejection>,
) -> Result<Json<BlockTaskResponse>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(DataEnvelope::new(api_task_from_record(
        kanban_sqlite::api::block_task(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            &body.reason,
            body.claim_token.as_deref(),
            body.force,
        )?,
    )?)))
}

pub(crate) async fn unblock_task(
    State(state): State<AppState>,
    Path(UnblockTaskPath { task_id }): Path<UnblockTaskPath>,
    headers: HeaderMap,
    body: Result<Json<UnblockTaskRequest>, JsonRejection>,
) -> Result<Json<UnblockTaskResponse>, ApiError> {
    let body = optional_json_body(body)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(DataEnvelope::new(api_task_from_record(
        kanban_sqlite::api::unblock_task(state.db_path(), &task.board_id, &actor, &task_id)?,
    )?)))
}

pub(crate) async fn archive_task(
    State(state): State<AppState>,
    Path(ArchiveTaskPath { task_id }): Path<ArchiveTaskPath>,
    headers: HeaderMap,
    body: Result<Json<ArchiveTaskRequest>, JsonRejection>,
) -> Result<Json<ArchiveTaskResponse>, ApiError> {
    let body = optional_json_body(body)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(DataEnvelope::new(api_task_from_record(
        kanban_sqlite::api::archive_task(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            body.force,
        )?,
    )?)))
}
