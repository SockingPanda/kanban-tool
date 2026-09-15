use super::super::support::api_run;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{ListRunsPath, ListRunsResponse};
pub(crate) async fn list_runs(
    state: AppState,
    ListRunsPath { task_id }: ListRunsPath,
) -> Result<ListRunsResponse, ApiError> {
    let runs = state.application().list_runs(&task_id).await?;
    let data = runs
        .into_iter()
        .map(api_run)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(ListRunsResponse { data })
}
