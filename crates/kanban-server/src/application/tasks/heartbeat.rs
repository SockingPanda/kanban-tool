use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{HeartbeatTaskPath, HeartbeatTaskRequest, HeartbeatTaskResponse};
use kanban_service::HeartbeatTaskCommand;
pub(crate) async fn heartbeat_task(
    state: AppState,
    HeartbeatTaskPath { task_id }: HeartbeatTaskPath,
    headers: CallContext,
    body: HeartbeatTaskRequest,
) -> Result<HeartbeatTaskResponse, ApiError> {
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
    Ok(HeartbeatTaskResponse::new(api_task(task)?))
}
