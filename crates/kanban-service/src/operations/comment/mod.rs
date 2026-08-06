mod create;
mod list;

pub use create::{CreateCommentCommand, CreateCommentRecord};

pub(crate) fn application_comment(
    comment: crate::domain::CommentRecord,
) -> crate::Result<crate::CommentRecord> {
    let author_type = match comment.author_type.as_str() {
        "user" => crate::CommentAuthorType::User,
        "agent" => crate::CommentAuthorType::Agent,
        other => {
            return Err(crate::KanbanError::Storage(format!(
                "stored comment author_type is invalid: {other}"
            )));
        }
    };
    let kind = match comment.kind.as_str() {
        "note" => crate::CommentKind::Note,
        "decision" => crate::CommentKind::Decision,
        "signal" => crate::CommentKind::Signal,
        other => {
            return Err(crate::KanbanError::Storage(format!(
                "stored comment kind is invalid: {other}"
            )));
        }
    };
    Ok(crate::CommentRecord {
        id: comment.id,
        board_id: comment.board_id,
        task_id: comment.task_id,
        author: comment.author,
        author_type,
        agent_type: comment.agent_type,
        body: comment.body,
        kind,
        metadata_json: comment.metadata_json,
        created_at: comment.created_at,
    })
}
