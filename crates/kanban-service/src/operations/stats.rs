use std::future::Future;

use kanban_core::{Clock, KanbanError, Result, TaskStatus};

use crate::{ApplicationService, ApplicationStore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusCountRecord {
    pub status: TaskStatus,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleClaimRecord {
    pub task_id: String,
    pub seq: i64,
    pub title: String,
    pub claim_owner: Option<String>,
    pub claim_expires_at: Option<i64>,
    pub last_heartbeat_at: Option<i64>,
    pub current_run_id: Option<String>,
    pub retry_count: i64,
    pub max_retries: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedReasonCountRecord {
    pub reason: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueStatsRecord {
    pub board_id: String,
    pub generated_at: i64,
    pub status_counts: Vec<StatusCountRecord>,
    pub stale_claims: Vec<StaleClaimRecord>,
    pub blocked_reasons: Vec<BlockedReasonCountRecord>,
    pub unplanned_active_tasks: i64,
    pub active_parents_with_incomplete_required_steps: i64,
}

pub trait StatsQuery: ApplicationStore {
    fn get_stats(
        &self,
        board: &str,
        generated_at: i64,
    ) -> impl Future<Output = Result<QueueStatsRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: StatsQuery,
    C: Clock,
{
    pub async fn get_stats(&self, board: &str) -> Result<QueueStatsRecord> {
        let board = board.trim();
        if board.is_empty() {
            return Err(KanbanError::InvalidInput("board is required".to_owned()));
        }
        let generated_at = self.clock.now_ms();
        if generated_at < 0 {
            return Err(KanbanError::InvalidInput(
                "generated_at must be non-negative".to_owned(),
            ));
        }
        self.store.get_stats(board, generated_at).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use kanban_core::{KanbanError, Result};

    use crate::operations::test_support::{FixedClock, StubStore};
    use crate::*;

    impl StatsQuery for StubStore {
        async fn get_stats(&self, board: &str, generated_at: i64) -> Result<QueueStatsRecord> {
            assert_eq!(board, "default");
            assert_eq!(generated_at, 100);
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(QueueStatsRecord {
                board_id: "b_default".into(),
                generated_at,
                status_counts: Vec::new(),
                stale_claims: Vec::new(),
                blocked_reasons: Vec::new(),
                unplanned_active_tasks: 0,
                active_parents_with_incomplete_required_steps: 0,
            })
        }
    }

    fn service(calls: Arc<AtomicUsize>) -> ApplicationService<StubStore, FixedClock> {
        ApplicationService::with_clock(StubStore { calls }, FixedClock(100))
    }

    #[tokio::test]
    async fn stats_trims_board_and_uses_application_clock() {
        let calls = Arc::new(AtomicUsize::new(0));
        let stats = service(calls.clone()).get_stats(" default ").await.unwrap();
        assert_eq!(stats.board_id, "b_default");
        assert_eq!(stats.generated_at, 100);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn stats_rejects_empty_board_without_calling_store() {
        let calls = Arc::new(AtomicUsize::new(0));
        let error = service(calls.clone()).get_stats("  ").await.unwrap_err();
        assert!(
            matches!(error, KanbanError::InvalidInput(message) if message == "board is required")
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}
