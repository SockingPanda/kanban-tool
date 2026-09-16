use super::super::support::request_actor;
use super::api_board;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{CreateBoardRequest, CreateBoardResponse};
use kanban_service::CreateBoardCommand;
pub(crate) async fn create_board(
    state: AppState,
    headers: CallContext,
    body: CreateBoardRequest,
) -> Result<CreateBoardResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let board = state
        .application()
        .create_board(CreateBoardCommand {
            slug: body.slug,
            name: body.name,
            description: body.description,
            actor,
        })
        .await?;
    Ok(CreateBoardResponse {
        data: api_board(board),
    })
}
