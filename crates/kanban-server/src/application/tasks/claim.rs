use super::super::support::{api_run, request_actor};
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{ApiClaim, ClaimTaskPath, ClaimTaskRequest, ClaimTaskResponse};
use kanban_service::ClaimTaskCommand;
pub(crate) async fn claim_task(
    state: AppState,
    ClaimTaskPath { task_id }: ClaimTaskPath,
    headers: CallContext,
    body: ClaimTaskRequest,
) -> Result<ClaimTaskResponse, ApiError> {
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
    Ok(ClaimTaskResponse::new(ApiClaim {
        task: api_task(claim.task)?,
        run: api_run(claim.run)?,
        claim_token: claim.claim_token,
        claim_expires_at: Some(claim.claim_expires_at),
    }))
}
