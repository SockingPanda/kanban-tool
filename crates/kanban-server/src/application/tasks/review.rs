use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{SubmitReviewTaskPath, SubmitReviewTaskRequest, SubmitReviewTaskResponse};
use kanban_service::SubmitReviewTaskCommand;
pub(crate) async fn submit_review_task(
    state: AppState,
    SubmitReviewTaskPath { task_id }: SubmitReviewTaskPath,
    headers: CallContext,
    body: SubmitReviewTaskRequest,
) -> Result<SubmitReviewTaskResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .submit_review_task(SubmitReviewTaskCommand {
            task_id,
            actor,
            claim_token: body.claim_token,
            force: body.force,
            summary: body.summary,
        })
        .await?;
    Ok(SubmitReviewTaskResponse::new(api_task(task)?))
}
