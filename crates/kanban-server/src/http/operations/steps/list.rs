use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use kanban_protocol::{ListStepsPath, ListStepsResponse};

pub(crate) async fn list_steps(
    State(state): State<AppState>,
    Path(ListStepsPath { task_id }): Path<ListStepsPath>,
) -> Result<Json<ListStepsResponse>, ApiError> {
    let steps = state.application().list_steps(&task_id).await?;
    Ok(Json(ListStepsResponse {
        data: api_task_steps(steps)?,
    }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        crate::http::operations::registered_path(
            kanban_protocol::HttpMethod::Get,
            "/api/v1/tasks/:task_id/steps",
        ),
        get(list_steps),
    )
}
#[cfg(test)]
mod tests {}
