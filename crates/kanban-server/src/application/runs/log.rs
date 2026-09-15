use crate::{error::ApiError, state::AppState};
use kanban_protocol::{ApiRunLog, GetRunLogPath, GetRunLogResponse};
pub(crate) async fn get_run_log(
    state: AppState,
    GetRunLogPath { run_id }: GetRunLogPath,
) -> Result<GetRunLogResponse, ApiError> {
    let log = state.application().get_run_log(&run_id).await?;
    Ok(GetRunLogResponse {
        data: ApiRunLog {
            run_id: log.run_id,
            content: log.content,
            truncated: log.truncated,
        },
    })
}
