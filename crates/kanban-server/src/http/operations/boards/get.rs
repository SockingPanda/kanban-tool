use super::api_board;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use kanban_protocol::{GetBoardPath, GetBoardResponse};

pub(crate) async fn get_board(
    State(state): State<AppState>,
    Path(GetBoardPath { board }): Path<GetBoardPath>,
) -> Result<Json<GetBoardResponse>, ApiError> {
    let board = state.application().get_board(&board).await?;
    Ok(Json(GetBoardResponse {
        data: api_board(board),
    }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        crate::http::operations::registered_path(
            kanban_protocol::HttpMethod::Get,
            "/api/v1/boards/:board",
        ),
        get(get_board),
    )
}
