use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;

use crate::dto::Envelope;
use crate::error::{ApiError, extractor_error};
use crate::state::AppState;

use super::shared::{ActorBody, actor, optional_json_body};

pub(crate) async fn list_boards(
    State(state): State<AppState>,
) -> Result<Json<Envelope<Vec<kanban_sqlite::api::BoardRecord>>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::api::list_boards(
            state.db_path(),
            kanban_sqlite::api::BoardListOptions::default(),
        )?,
        meta: None,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateBoardBody {
    slug: String,
    name: String,
    description: Option<String>,
    actor: Option<String>,
}

pub(crate) async fn create_board(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<CreateBoardBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Envelope<kanban_sqlite::api::BoardRecord>>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let board = kanban_sqlite::api::create_board(
        state.db_path(),
        &actor,
        kanban_sqlite::api::CreateBoard {
            slug: body.slug,
            name: body.name,
            description: body.description,
        },
    )?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: board,
            meta: None,
        }),
    ))
}

pub(crate) async fn get_board(
    State(state): State<AppState>,
    Path(board): Path<String>,
) -> Result<Json<Envelope<kanban_sqlite::api::BoardRecord>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::api::get_board(state.db_path(), &board)?,
        meta: None,
    }))
}

pub(crate) async fn archive_board(
    State(state): State<AppState>,
    Path(board): Path<String>,
    headers: HeaderMap,
    body: Result<Json<ActorBody>, JsonRejection>,
) -> Result<Json<Envelope<kanban_sqlite::api::BoardRecord>>, ApiError> {
    let body = optional_json_body(body)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    Ok(Json(Envelope {
        data: kanban_sqlite::api::archive_board(state.db_path(), &board, &actor)?,
        meta: None,
    }))
}

pub(crate) async fn list_board_columns(
    State(state): State<AppState>,
    Path(board): Path<String>,
) -> Result<Json<Envelope<Vec<kanban_sqlite::api::BoardColumnRecord>>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::api::list_board_columns(state.db_path(), &board)?,
        meta: None,
    }))
}
