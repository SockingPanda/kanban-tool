use kanban_core::Result;

use crate::{ApplicationHealth, ApplicationStore, BoardColumnRecord, BoardRecord};

/// The canonical command/query entry point shared by the HTTP handlers and the
/// in-process dispatcher.
#[derive(Debug, Clone)]
pub struct ApplicationService<S> {
    store: S,
}

impl<S> ApplicationService<S>
where
    S: ApplicationStore,
{
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub async fn health(&self) -> Result<ApplicationHealth> {
        // A real store query proves that the initialized canonical database is
        // still reachable without exposing a raw connection to the handler.
        self.store.list_boards(true).await?;
        Ok(ApplicationHealth { ok: true })
    }

    pub async fn list_boards(&self, include_archived: bool) -> Result<Vec<BoardRecord>> {
        self.store.list_boards(include_archived).await
    }

    pub async fn list_board_columns(&self, board: &str) -> Result<Vec<BoardColumnRecord>> {
        self.store.list_board_columns(board).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use kanban_core::{Board, Result, TaskStatus};

    use super::*;

    #[derive(Clone)]
    struct StubStore {
        calls: Arc<AtomicUsize>,
    }

    impl ApplicationStore for StubStore {
        async fn list_boards(&self, include_archived: bool) -> Result<Vec<BoardRecord>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert!(include_archived);
            Ok(vec![Board {
                id: "b_default".into(),
                slug: "default".into(),
                name: "Default".into(),
                description: None,
                created_at: 1,
                updated_at: 1,
                archived_at: None,
            }])
        }

        async fn list_board_columns(&self, board: &str) -> Result<Vec<BoardColumnRecord>> {
            assert_eq!(board, "default");
            Ok(vec![BoardColumnRecord {
                id: "col_default_todo".into(),
                board_id: "b_default".into(),
                status: TaskStatus::Todo,
                title: "Todo".into(),
                position: 20,
                hidden: false,
                wip_limit: None,
                created_at: 1,
                updated_at: 1,
            }])
        }
    }

    #[tokio::test]
    async fn health_and_board_queries_share_the_application_store() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = ApplicationService::new(StubStore {
            calls: calls.clone(),
        });

        assert!(service.health().await.unwrap().ok);
        assert_eq!(service.list_boards(true).await.unwrap().len(), 1);
        assert_eq!(
            service.list_board_columns("default").await.unwrap()[0].status,
            TaskStatus::Todo
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
