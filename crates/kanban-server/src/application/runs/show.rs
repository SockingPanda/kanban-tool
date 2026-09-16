use super::super::support::api_run;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{GetRunPath, GetRunResponse};
pub(crate) async fn get_run(
    state: AppState,
    GetRunPath { run_id }: GetRunPath,
) -> Result<GetRunResponse, ApiError> {
    let run = state.application().get_run(&run_id).await?;
    Ok(GetRunResponse {
        data: api_run(run)?,
    })
}
