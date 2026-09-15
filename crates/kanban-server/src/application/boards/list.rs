use super::api_board;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{ListBoardsQuery, ListBoardsResponse};
pub(crate) async fn list_boards(
    state: AppState,
    query: ListBoardsQuery,
) -> Result<ListBoardsResponse, ApiError> {
    let data = state
        .application()
        .list_boards(query.include_archived)
        .await?
        .into_iter()
        .map(api_board)
        .collect();
    Ok(ListBoardsResponse { data })
}
