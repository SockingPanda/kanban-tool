use crate::error::ApiError;
use kanban_application::{
    CommentAuthorType as ApplicationCommentAuthorType, CommentKind as ApplicationCommentKind,
};
use kanban_core::KanbanError;
use kanban_protocol::ApiComment;

pub(crate) fn api_comment(
    comment: kanban_application::CommentRecord,
) -> Result<ApiComment, ApiError> {
    let metadata = serde_json::from_str(&comment.metadata_json).map_err(|error| {
        KanbanError::Storage(format!("stored comment metadata is invalid JSON: {error}"))
    })?;
    Ok(ApiComment {
        id: comment.id,
        board_id: comment.board_id,
        task_id: comment.task_id,
        author: comment.author,
        author_type: match comment.author_type {
            ApplicationCommentAuthorType::User => kanban_protocol::CommentAuthorType::User,
            ApplicationCommentAuthorType::Agent => kanban_protocol::CommentAuthorType::Agent,
        },
        agent_type: comment.agent_type,
        body: comment.body,
        kind: match comment.kind {
            ApplicationCommentKind::Note => kanban_protocol::CommentKind::Note,
            ApplicationCommentKind::Decision => kanban_protocol::CommentKind::Decision,
            ApplicationCommentKind::Signal => kanban_protocol::CommentKind::Signal,
        },
        metadata,
        created_at: comment.created_at,
    })
}
