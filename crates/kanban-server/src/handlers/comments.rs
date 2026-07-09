use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
};

use crate::dto::{CommentDto, Envelope};
use crate::error::{ApiError, extractor_error};
use crate::state::AppState;

use super::shared::{CommentBody, actor, metadata_json};

pub(crate) async fn list_comments(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Envelope<Vec<CommentDto>>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::api::list_comments(state.db_path(), &task_id)?
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
    let metadata_json = metadata_json(body.metadata)?;
    let comment = kanban_sqlite::api::create_comment_with_options(
        state.db_path(),
        &task_id,
        kanban_sqlite::api::CreateComment {
            author: actor,
            body: body.body,
            kind: body.kind,
            author_type: body.author_type,
            agent_type: body.agent_type,
            metadata_json: Some(metadata_json),
        },
    )?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: CommentDto::from(comment),
            meta: None,
        }),
    ))
}
