use super::super::support::request_actor;
use super::api_board;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    routing::post,
};
use kanban_application::CreateBoardCommand;
use kanban_core::KanbanError;
use kanban_protocol::{CreateBoardRequest, CreateBoardResponse};

pub(crate) async fn create_board(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<CreateBoardRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CreateBoardResponse>), ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("JSON 请求体无效：{error}")))?;
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
    Ok((
        StatusCode::CREATED,
        Json(CreateBoardResponse {
            data: api_board(board),
        }),
    ))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/boards", post(create_board))
}
