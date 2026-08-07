use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use kanban_protocol::{ListCommentsPath, ListCommentsResponse};

pub(crate) async fn list_comments(
    State(state): State<AppState>,
    Path(ListCommentsPath { task_id }): Path<ListCommentsPath>,
) -> Result<Json<ListCommentsResponse>, ApiError> {
    let data = state
        .application()
        .list_comments(&task_id)
        .await?
        .into_iter()
        .map(api_comment)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(ListCommentsResponse { data }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        crate::http::operations::registered_path(
            kanban_protocol::HttpMethod::Get,
            "/api/v1/tasks/:task_id/comments",
        ),
        get(list_comments),
    )
}
#[cfg(test)]
mod tests {}
