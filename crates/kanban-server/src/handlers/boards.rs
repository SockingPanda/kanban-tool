use axum::{
    Json,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{HeaderMap, StatusCode},
};
use kanban_contract::{
    ApiBoard, ApiBoardColumn, ArchiveBoardPath, ArchiveBoardRequest, ArchiveBoardResponse,
    CreateBoardRequest, CreateBoardResponse, GetBoardPath, GetBoardResponse, ListBoardColumnsPath,
    ListBoardColumnsResponse, ListBoardsQuery, ListBoardsResponse,
};

use super::shared::{actor, optional_json_body};
use crate::error::{ApiError, extractor_error};
use crate::state::AppState;

pub(crate) async fn list_boards(
    State(state): State<AppState>,
    query: Result<Query<ListBoardsQuery>, QueryRejection>,
) -> Result<Json<ListBoardsResponse>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let boards = kanban_sqlite::api::list_boards(
        state.db_path(),
        kanban_sqlite::api::BoardListOptions {
            include_archived: query.include_archived,
        },
    )?;
    Ok(Json(ListBoardsResponse {
        data: boards.into_iter().map(api_board).collect(),
    }))
}

pub(crate) async fn create_board(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<CreateBoardRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CreateBoardResponse>), ApiError> {
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
        Json(CreateBoardResponse {
            data: api_board(board),
        }),
    ))
}

pub(crate) async fn get_board(
    State(state): State<AppState>,
    Path(path): Path<GetBoardPath>,
) -> Result<Json<GetBoardResponse>, ApiError> {
    let board = kanban_sqlite::api::get_board(state.db_path(), &path.board)?;
    Ok(Json(GetBoardResponse {
        data: api_board(board),
    }))
}

pub(crate) async fn archive_board(
    State(state): State<AppState>,
    Path(path): Path<ArchiveBoardPath>,
    headers: HeaderMap,
    body: Result<Json<ArchiveBoardRequest>, JsonRejection>,
) -> Result<Json<ArchiveBoardResponse>, ApiError> {
    let body = optional_json_body(body)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let board = kanban_sqlite::api::archive_board(state.db_path(), &path.board, &actor)?;
    Ok(Json(ArchiveBoardResponse {
        data: api_board(board),
    }))
}

fn api_board(board: kanban_sqlite::api::BoardRecord) -> ApiBoard {
    ApiBoard {
        id: board.id,
        slug: board.slug,
        name: board.name,
        description: board.description,
        created_at: board.created_at,
        updated_at: board.updated_at,
        archived_at: board.archived_at,
    }
}

pub(crate) async fn list_board_columns(
    State(state): State<AppState>,
    Path(path): Path<ListBoardColumnsPath>,
) -> Result<Json<ListBoardColumnsResponse>, ApiError> {
    let data = kanban_sqlite::api::list_board_columns(state.db_path(), &path.board)?
        .into_iter()
        .map(|column| ApiBoardColumn {
            id: column.id,
            board_id: column.board_id,
            status: crate::dto::api_task_status_from_core(column.status),
            title: column.title,
            position: column.position,
            hidden: column.hidden,
            wip_limit: column.wip_limit,
            created_at: column.created_at,
            updated_at: column.updated_at,
        })
        .collect();
    Ok(Json(ListBoardColumnsResponse { data }))
}
