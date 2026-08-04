use std::time::UNIX_EPOCH;

use axum::{
    Json,
    extract::{Path, Query, State},
};
use kanban_contract::{
    ApiBoard, ApiBoardColumn, HealthReport, HealthResponse, ListBoardColumnsResponse,
    ListBoardsQuery, ListBoardsResponse,
};

use crate::{error::ApiError, state::AppState};

pub(crate) async fn health(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, ApiError> {
    state.application().health().await?;
    let metadata = tokio::fs::metadata(state.db_path())
        .await
        .map_err(|error| kanban_core::KanbanError::Storage(error.to_string()))?;
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    Ok(Json(HealthResponse::new(HealthReport {
        ok: true,
        db: "turso".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        db_path: state.db_path().display().to_string(),
        db_fingerprint: format!("turso:{}:{modified_ms}", metadata.len()),
    })))
}

pub(crate) async fn list_boards(
    State(state): State<AppState>,
    Query(query): Query<ListBoardsQuery>,
) -> Result<Json<ListBoardsResponse>, ApiError> {
    let data = state
        .application()
        .list_boards(query.include_archived)
        .await?
        .into_iter()
        .map(|board| ApiBoard {
            id: board.id,
            slug: board.slug,
            name: board.name,
            description: board.description,
            created_at: board.created_at,
            updated_at: board.updated_at,
            archived_at: board.archived_at,
        })
        .collect();
    Ok(Json(ListBoardsResponse { data }))
}

pub(crate) async fn list_board_columns(
    State(state): State<AppState>,
    Path(board): Path<String>,
) -> Result<Json<ListBoardColumnsResponse>, ApiError> {
    let data = state
        .application()
        .list_board_columns(&board)
        .await?
        .into_iter()
        .map(|column| ApiBoardColumn {
            id: column.id,
            board_id: column.board_id,
            status: match column.status {
                kanban_core::TaskStatus::Triage => kanban_contract::ApiTaskStatus::Triage,
                kanban_core::TaskStatus::Todo => kanban_contract::ApiTaskStatus::Todo,
                kanban_core::TaskStatus::Scheduled => kanban_contract::ApiTaskStatus::Scheduled,
                kanban_core::TaskStatus::Ready => kanban_contract::ApiTaskStatus::Ready,
                kanban_core::TaskStatus::Running => kanban_contract::ApiTaskStatus::Running,
                kanban_core::TaskStatus::Blocked => kanban_contract::ApiTaskStatus::Blocked,
                kanban_core::TaskStatus::Review => kanban_contract::ApiTaskStatus::Review,
                kanban_core::TaskStatus::Done => kanban_contract::ApiTaskStatus::Done,
                kanban_core::TaskStatus::Archived => kanban_contract::ApiTaskStatus::Archived,
            },
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
