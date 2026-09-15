use super::support::*;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{ListCommentsPath, ListCommentsResponse};
pub(crate) async fn list_comments(
    state: AppState,
    ListCommentsPath { task_id }: ListCommentsPath,
) -> Result<ListCommentsResponse, ApiError> {
    let data = state
        .application()
        .list_comments(&task_id)
        .await?
        .into_iter()
        .map(api_comment)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ListCommentsResponse { data })
}
