use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use kanban_protocol::{ApiBoardColumn, ListBoardColumnsResponse};

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
                kanban_core::TaskStatus::Triage => kanban_protocol::ApiTaskStatus::Triage,
                kanban_core::TaskStatus::Todo => kanban_protocol::ApiTaskStatus::Todo,
                kanban_core::TaskStatus::Scheduled => kanban_protocol::ApiTaskStatus::Scheduled,
                kanban_core::TaskStatus::Ready => kanban_protocol::ApiTaskStatus::Ready,
                kanban_core::TaskStatus::Running => kanban_protocol::ApiTaskStatus::Running,
                kanban_core::TaskStatus::Blocked => kanban_protocol::ApiTaskStatus::Blocked,
                kanban_core::TaskStatus::Review => kanban_protocol::ApiTaskStatus::Review,
                kanban_core::TaskStatus::Done => kanban_protocol::ApiTaskStatus::Done,
                kanban_core::TaskStatus::Archived => kanban_protocol::ApiTaskStatus::Archived,
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

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/boards/:board/columns", get(list_board_columns))
}

#[cfg(test)]
mod tests {}
