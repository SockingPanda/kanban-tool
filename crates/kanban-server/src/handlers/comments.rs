use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
};

use crate::dto::{CommentDto, Envelope};
use crate::error::{ApiError, extractor_error};
use crate::state::AppState;

use super::shared::{CommentBody, actor};

pub(crate) async fn list_comments(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Envelope<Vec<CommentDto>>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::list_comments(state.db_path(), &task_id)?
            .into_iter()
            .map(CommentDto::from)
            .collect(),
        meta: None,
    }))
}

pub(crate) async fn create_comment(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<CommentBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Envelope<CommentDto>>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.author.as_deref(), &headers, &state);
    let comment = kanban_sqlite::create_comment(
        state.db_path(),
        &task_id,
        &actor,
        &body.body,
        body.kind.as_deref(),
    )?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: CommentDto::from(comment),
            meta: None,
        }),
    ))
}
