use kanban_core::{Clock, Result};

use crate::{ApplicationHealth, ApplicationService, ApplicationStore, BoardRecord};

impl<S, C> ApplicationService<S, C>
where
    S: ApplicationStore,
    C: Clock,
{
    pub async fn health(&self) -> Result<ApplicationHealth> {
        // A real store query proves that the initialized canonical database is
        // still reachable without exposing a raw connection to the handler.
        self.store.list_boards(true).await?;
        Ok(ApplicationHealth { ok: true })
    }

    pub async fn list_boards(&self, include_archived: bool) -> Result<Vec<BoardRecord>> {
        self.store.list_boards(include_archived).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use kanban_core::TaskStatus;

    use crate::operations::test_support::StubStore;
    use crate::*;
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
