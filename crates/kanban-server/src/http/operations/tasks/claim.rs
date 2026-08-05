use super::super::support::{api_run, request_actor};
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_application::ClaimTaskCommand;
use kanban_contract::{ApiClaim, ClaimTaskPath, ClaimTaskRequest, ClaimTaskResponse};
use kanban_core::KanbanError;

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

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/tasks/:task_id/transitions/claim", post(claim_task))
}

#[cfg(test)]
mod tests {}
