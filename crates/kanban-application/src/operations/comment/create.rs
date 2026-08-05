use std::future::Future;

use kanban_core::{Clock, KanbanError, Result, new_event_id, new_typed_id};

use crate::{ApplicationService, ApplicationStore, CommentAuthorType, CommentKind, CommentRecord};

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

pub trait CommentCreate: ApplicationStore {
    fn create_comment(
        &self,
        task_id: &str,
        input: CreateCommentRecord,
    ) -> impl Future<Output = Result<CommentRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: CommentCreate,
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
                CreateCommentRecord {
                    id: new_typed_id("c"),
                    idempotency_key,
                    author: author.to_owned(),
                    author_type: command.author_type,
                    agent_type,
                    body: body.to_owned(),
                    kind: command.kind,
                    metadata_json,
                    event_id: new_event_id(),
                    created_at: self.clock.now_ms(),
                },
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, atomic::AtomicUsize};

    use kanban_core::{KanbanError, Result};

    use crate::operations::test_support::{FixedClock, StubStore};
    use crate::*;

    impl CommentCreate for StubStore {
        async fn create_comment(
            &self,
            task_id: &str,
            input: CreateCommentRecord,
        ) -> Result<CommentRecord> {
            assert_eq!(task_id, "t_comment");
            assert!(input.id.starts_with("c_"));
            assert!(input.event_id.starts_with("e_"));
            assert_eq!(input.created_at, 100);
            Ok(CommentRecord {
                id: input.id,
                board_id: "b_default".into(),
                task_id: task_id.into(),
                author: input.author,
                author_type: input.author_type,
                agent_type: input.agent_type,
                body: input.body,
                kind: input.kind,
                metadata_json: input.metadata_json,
                created_at: input.created_at,
            })
        }
    }

    #[tokio::test]
    async fn create_comment_canonicalizes_input_and_rejects_signal_kind() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let comment = service
            .create_comment(CreateCommentCommand {
                task_id: " t_comment ".into(),
                idempotency_key: Some(" comment-1 ".into()),
                author: " alice ".into(),
                author_type: CommentAuthorType::User,
                agent_type: None,
                body: " note ".into(),
                kind: CommentKind::Note,
                metadata: BTreeMap::from([(String::from("source"), serde_json::json!("test"))]),
            })
            .await
            .unwrap();
        assert_eq!(comment.author, "alice");
        assert_eq!(comment.body, "note");
        assert_eq!(comment.kind, CommentKind::Note);
        assert_eq!(comment.metadata_json, r#"{"source":"test"}"#);

        let error = service
            .create_comment(CreateCommentCommand {
                task_id: "t_comment".into(),
                idempotency_key: Some("signal-1".into()),
                author: "alice".into(),
                author_type: CommentAuthorType::User,
                agent_type: None,
                body: "signal".into(),
                kind: CommentKind::Signal,
                metadata: BTreeMap::new(),
            })
            .await
            .expect_err("signal comments are deferred");
        assert!(matches!(error, KanbanError::FeatureNotAvailable(_)));
    }
}
