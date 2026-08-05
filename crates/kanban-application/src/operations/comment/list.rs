use std::future::Future;

use kanban_core::{Clock, KanbanError, Result};

use crate::{ApplicationService, ApplicationStore, CommentRecord};

pub trait CommentList: ApplicationStore {
    fn list_comments(
        &self,
        task_id: &str,
    ) -> impl Future<Output = Result<Vec<CommentRecord>>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: CommentList,
    C: Clock,
{
    pub async fn list_comments(&self, task_id: &str) -> Result<Vec<CommentRecord>> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        self.store.list_comments(task_id).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicUsize};

    use kanban_core::{KanbanError, Result};

    use crate::operations::test_support::{FixedClock, StubStore};
    use crate::*;

    impl CommentList for StubStore {
        async fn list_comments(&self, task_id: &str) -> Result<Vec<CommentRecord>> {
            assert_eq!(task_id, "t_comment");
            Ok(vec![CommentRecord {
                id: "c_comment".into(),
                board_id: "b_default".into(),
                task_id: task_id.into(),
                author: "alice".into(),
                author_type: CommentAuthorType::User,
                agent_type: None,
                body: "handoff".into(),
                kind: CommentKind::Note,
                metadata_json: r#"{"source":"test"}"#.into(),
                created_at: 100,
            }])
        }
    }

    #[tokio::test]
    async fn list_comments_requires_global_task_id_and_preserves_history() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let comments = service.list_comments(" t_comment ").await.unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].task_id, "t_comment");
        assert_eq!(comments[0].metadata_json, r#"{"source":"test"}"#);

        let error = service
            .list_comments("default#1")
            .await
            .expect_err("board-local selectors must be resolved by the client");
        assert!(matches!(error, KanbanError::InvalidInput(message) if message.contains("global")));
    }
}
