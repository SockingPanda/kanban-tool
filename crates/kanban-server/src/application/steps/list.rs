use super::support::*;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{ListStepsPath, ListStepsResponse};
pub(crate) async fn list_steps(
    state: AppState,
    ListStepsPath { task_id }: ListStepsPath,
) -> Result<ListStepsResponse, ApiError> {
    let steps = state.application().list_steps(&task_id).await?;
    Ok(ListStepsResponse {
        data: api_task_steps(steps)?,
    })
}
