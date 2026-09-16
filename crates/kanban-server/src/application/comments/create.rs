use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{CreateCommentPath, CreateCommentRequest, CreateCommentResponse};
use kanban_service::KanbanError;
use kanban_service::{
    CommentAuthorType as ApplicationCommentAuthorType, CommentKind as ApplicationCommentKind,
    CreateCommentCommand,
};
pub(crate) async fn create_comment(
    state: AppState,
    CreateCommentPath { task_id }: CreateCommentPath,
    headers: CallContext,
    body: CreateCommentRequest,
) -> Result<CreateCommentResponse, ApiError> {
    let actor = request_actor(body.author.as_deref(), &headers, state.default_actor())?;
    let metadata = body.metadata.unwrap_or_else(|| serde_json::json!({}));
    let metadata = metadata.as_object().cloned().ok_or_else(|| {
        ApiError(KanbanError::InvalidInput(
            "metadata 必须是 JSON 对象".to_owned(),
        ))
    })?;
    let comment = state
        .application()
        .create_comment(CreateCommentCommand {
            task_id,
            idempotency_key: body.idempotency_key,
            author: actor,
            author_type: body
                .author_type
                .map(|value| match value {
                    kanban_protocol::CommentAuthorType::User => ApplicationCommentAuthorType::User,
                    kanban_protocol::CommentAuthorType::Agent => {
                        ApplicationCommentAuthorType::Agent
                    }
                })
                .unwrap_or(ApplicationCommentAuthorType::User),
            agent_type: body.agent_type,
            body: body.body,
            kind: body
                .kind
                .map(|value| match value {
                    kanban_protocol::CommentKind::Note => ApplicationCommentKind::Note,
                    kanban_protocol::CommentKind::Decision => ApplicationCommentKind::Decision,
                    kanban_protocol::CommentKind::Signal => ApplicationCommentKind::Signal,
                })
                .unwrap_or(ApplicationCommentKind::Note),
            metadata: metadata.into_iter().collect(),
        })
        .await?;
    Ok(CreateCommentResponse {
        data: api_comment(comment)?,
    })
}
