use super::shared::{actor, metadata_json};
use crate::error::{ApiError, extractor_error, invalid_input};
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
};
use kanban_contract::{
    ApiComment, CommentAuthorType, CommentKind, CreateCommentPath, CreateCommentRequest,
    CreateCommentResponse, ListCommentsPath, ListCommentsResponse,
};

fn api_comment(comment: kanban_sqlite::api::CommentRecord) -> Result<ApiComment, ApiError> {
    let author_type = match comment.author_type.as_str() {
        "user" => CommentAuthorType::User,
        "agent" => CommentAuthorType::Agent,
        _ => return Err(invalid_input("invalid stored comment author_type")),
    };
    let kind = match comment.kind.as_str() {
        "note" => CommentKind::Note,
        "decision" => CommentKind::Decision,
        "signal" => CommentKind::Signal,
        _ => return Err(invalid_input("invalid stored comment kind")),
    };
    let metadata_value: serde_json::Value =
        serde_json::from_str(&comment.metadata_json).map_err(|error| {
            ApiError(kanban_core::KanbanError::Storage(format!(
                "comment {} has invalid metadata_json: {error}",
                comment.id
            )))
        })?;
    let metadata = serde_json::from_value(metadata_value).map_err(|error| {
        ApiError(kanban_core::KanbanError::Storage(format!(
            "comment {} has invalid metadata_json: {error}",
            comment.id
        )))
    })?;
    Ok(ApiComment {
        id: comment.id,
        board_id: comment.board_id,
        task_id: comment.task_id,
        author: comment.author,
        author_type,
        agent_type: comment.agent_type,
        body: comment.body,
        kind,
        metadata,
        created_at: comment.created_at,
    })
}

pub(crate) async fn list_comments(
    State(state): State<AppState>,
    Path(path): Path<ListCommentsPath>,
) -> Result<Json<ListCommentsResponse>, ApiError> {
    let data = kanban_sqlite::api::list_comments(state.db_path(), &path.task_id)?
        .into_iter()
        .map(api_comment)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(ListCommentsResponse { data }))
}

pub(crate) async fn create_comment(
    State(state): State<AppState>,
    Path(path): Path<CreateCommentPath>,
    headers: HeaderMap,
    body: Result<Json<CreateCommentRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CreateCommentResponse>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let author = actor(body.author.as_deref(), &headers, &state);
    let metadata_json = metadata_json(body.metadata)?;
    let comment = kanban_sqlite::api::create_comment_with_options(
        state.db_path(),
        &path.task_id,
        kanban_sqlite::api::CreateComment {
            author,
            body: body.body,
            kind: body.kind.map(|value| value.as_str().to_owned()),
            author_type: body.author_type.map(|value| value.as_str().to_owned()),
            agent_type: body.agent_type,
            metadata_json: Some(metadata_json),
        },
    )?;
    Ok((
        StatusCode::CREATED,
        Json(CreateCommentResponse {
            data: api_comment(comment)?,
        }),
    ))
}
