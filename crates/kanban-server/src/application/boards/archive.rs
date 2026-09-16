use super::super::support::request_actor;
use super::api_board;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{ArchiveBoardPath, ArchiveBoardRequest, ArchiveBoardResponse};
use kanban_service::ArchiveBoardCommand;
pub(crate) async fn archive_board(
    state: AppState,
    ArchiveBoardPath { board }: ArchiveBoardPath,
    headers: CallContext,
    body: ArchiveBoardRequest,
) -> Result<ArchiveBoardResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let board = state
        .application()
        .archive_board(ArchiveBoardCommand { board, actor })
        .await?;
    Ok(ArchiveBoardResponse {
        data: api_board(board),
    })
}
