use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{
    MarkExecutionPlanNotRequiredPath, MarkExecutionPlanNotRequiredRequest,
    MarkExecutionPlanNotRequiredResponse,
};
use kanban_service::MarkExecutionPlanNotRequiredCommand;
pub(crate) async fn mark_execution_plan_not_required(
    state: AppState,
    MarkExecutionPlanNotRequiredPath { task_id }: MarkExecutionPlanNotRequiredPath,
    headers: CallContext,
    body: MarkExecutionPlanNotRequiredRequest,
) -> Result<MarkExecutionPlanNotRequiredResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let plan = state
        .application()
        .mark_execution_plan_not_required(MarkExecutionPlanNotRequiredCommand {
            task_id,
            reason: body.reason,
            actor,
        })
        .await?;
    Ok(MarkExecutionPlanNotRequiredResponse {
        data: api_execution_plan(plan),
    })
}
