use super::api_board;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{GetBoardPath, GetBoardResponse};
pub(crate) async fn get_board(
    state: AppState,
    GetBoardPath { board }: GetBoardPath,
) -> Result<GetBoardResponse, ApiError> {
    let board = state.application().get_board(&board).await?;
    Ok(GetBoardResponse {
        data: api_board(board),
    })
}
