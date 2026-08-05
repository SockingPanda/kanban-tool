use super::super::support::request_actor;
use super::api_board;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_application::ArchiveBoardCommand;
use kanban_contract::{ArchiveBoardPath, ArchiveBoardRequest, ArchiveBoardResponse};
use kanban_core::KanbanError;

pub(crate) async fn archive_board(
    State(state): State<AppState>,
    Path(ArchiveBoardPath { board }): Path<ArchiveBoardPath>,
    headers: HeaderMap,
    body: Result<Json<ArchiveBoardRequest>, JsonRejection>,
) -> Result<Json<ArchiveBoardResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let board = state
        .application()
        .archive_board(ArchiveBoardCommand { board, actor })
        .await?;
    Ok(Json(ArchiveBoardResponse {
        data: api_board(board),
    }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/boards/:board/archive", post(archive_board))
}
