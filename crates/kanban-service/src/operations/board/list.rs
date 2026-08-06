use std::future::Future;

use kanban_core::{Clock, Result};

use crate::{ApplicationHealth, ApplicationService, ApplicationStore, BoardRecord};

pub trait BoardList: ApplicationStore {
    fn list_boards(
        &self,
        include_archived: bool,
    ) -> impl Future<Output = Result<Vec<BoardRecord>>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: BoardList,
    C: Clock,
{
    pub async fn health(&self) -> Result<ApplicationHealth> {
        // 真实的 store query 可以证明已初始化的规范数据库仍可访问，同时不会向 handler
        // 暴露原始连接。
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

    use kanban_core::{Result, TaskStatus};

    use crate::operations::test_support::StubStore;
    use crate::*;

    impl BoardList for StubStore {
        async fn list_boards(&self, include_archived: bool) -> Result<Vec<BoardRecord>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert!(include_archived);
            Ok(vec![kanban_core::Board {
                id: "b_default".into(),
                slug: "default".into(),
                name: "Default".into(),
                description: None,
                created_at: 1,
                updated_at: 1,
                archived_at: None,
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
