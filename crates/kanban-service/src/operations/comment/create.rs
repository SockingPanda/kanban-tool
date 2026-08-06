use kanban_core::{Clock, KanbanError, Result, new_event_id, new_typed_id};

use crate::{CommentAuthorType, CommentKind, CommentRecord, KanbanService};

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct CreateCommentCommand {
    pub task_id: String,
    pub idempotency_key: Option<String>,
    pub author: String,
    pub author_type: CommentAuthorType,
    pub agent_type: Option<String>,
    pub body: String,
    pub kind: CommentKind,
    pub metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateCommentRecord {
    pub id: String,
    pub idempotency_key: Option<String>,
    pub author: String,
    pub author_type: CommentAuthorType,
    pub agent_type: Option<String>,
    pub body: String,
    pub kind: CommentKind,
    pub metadata_json: String,
    pub event_id: String,
    pub created_at: i64,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn create_comment(&self, command: CreateCommentCommand) -> Result<CommentRecord> {
        let task_id = command.task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        if command.kind == CommentKind::Signal {
            return Err(KanbanError::FeatureNotAvailable(
                "signal comments are not available on the single-host path".to_owned(),
            ));
        }
        let author = command.author.trim();
        if author.is_empty() {
            return Err(KanbanError::InvalidInput(
                "comment author is required".to_owned(),
            ));
        }
        let body = command.body.trim();
        if body.is_empty() {
            return Err(KanbanError::InvalidInput(
                "comment body is required".to_owned(),
            ));
        }
        if command
            .agent_type
            .as_deref()
            .is_some_and(|agent_type| !agent_type.trim().is_empty())
            && command.author_type != CommentAuthorType::Agent
        {
            return Err(KanbanError::InvalidInput(
                "comment agent_type is only allowed when author_type is agent".to_owned(),
            ));
        }
        if command
            .idempotency_key
            .as_deref()
            .is_some_and(|key| key.trim().is_empty())
        {
            return Err(KanbanError::InvalidInput(
                "idempotency_key must not be empty".to_owned(),
            ));
        }
        let metadata_json = serde_json::to_string(&command.metadata)
            .map_err(|error| KanbanError::InvalidInput(format!("invalid metadata: {error}")))?;
        let idempotency_key = command
            .idempotency_key
            .map(|key| key.trim().to_owned())
            .filter(|key| !key.is_empty());
        let agent_type = command
            .agent_type
            .map(|agent_type| agent_type.trim().to_owned())
            .filter(|agent_type| !agent_type.is_empty());
        let _mutation = self.mutation_gate.lock().await;
        self.store
            .create_comment(
                task_id,
                crate::CreateCommentInput {
                    id: new_typed_id("c"),
                    idempotency_key,
                    author: author.to_owned(),
                    author_type: command.author_type.as_str().to_owned(),
                    agent_type,
                    body: body.to_owned(),
                    kind: command.kind.as_str().to_owned(),
                    metadata_json,
                    event_id: new_event_id(),
                    created_at: self.clock.now_ms(),
                },
            )
            .await
            .map_err(crate::error::store_error)
            .and_then(super::application_comment)
    }
}
