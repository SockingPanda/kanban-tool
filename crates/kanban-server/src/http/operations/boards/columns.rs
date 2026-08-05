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
                kanban_service::TaskStatus::Triage => kanban_protocol::ApiTaskStatus::Triage,
                kanban_service::TaskStatus::Todo => kanban_protocol::ApiTaskStatus::Todo,
                kanban_service::TaskStatus::Scheduled => kanban_protocol::ApiTaskStatus::Scheduled,
                kanban_service::TaskStatus::Ready => kanban_protocol::ApiTaskStatus::Ready,
                kanban_service::TaskStatus::Running => kanban_protocol::ApiTaskStatus::Running,
                kanban_service::TaskStatus::Blocked => kanban_protocol::ApiTaskStatus::Blocked,
                kanban_service::TaskStatus::Review => kanban_protocol::ApiTaskStatus::Review,
                kanban_service::TaskStatus::Done => kanban_protocol::ApiTaskStatus::Done,
                kanban_service::TaskStatus::Archived => kanban_protocol::ApiTaskStatus::Archived,
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
