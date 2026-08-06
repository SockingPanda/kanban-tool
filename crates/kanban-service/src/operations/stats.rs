use kanban_core::{Clock, KanbanError, Result, TaskStatus};

use crate::KanbanService;

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

impl<C> KanbanService<C>
where
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
        self.application
            .store
            .store
            .get_stats(board, generated_at)
            .await
            .map_err(crate::adapter::store_error)
            .and_then(application_stats)
    }
}

fn application_stats(stats: crate::domain::QueueStatsRecord) -> Result<QueueStatsRecord> {
    Ok(QueueStatsRecord {
        board_id: stats.board_id,
        generated_at: stats.generated_at,
        status_counts: stats
            .status_counts
            .into_iter()
            .map(application_status_count)
            .collect::<Result<Vec<_>>>()?,
        stale_claims: stats
            .stale_claims
            .into_iter()
            .map(application_stale_claim)
            .collect(),
        blocked_reasons: stats
            .blocked_reasons
            .into_iter()
            .map(application_blocked_reason)
            .collect(),
        unplanned_active_tasks: stats.unplanned_active_tasks,
        active_parents_with_incomplete_required_steps: stats
            .active_parents_with_incomplete_required_steps,
    })
}

fn application_status_count(value: crate::domain::StatusCountRecord) -> Result<StatusCountRecord> {
    let status = value.status.parse::<TaskStatus>().map_err(|error| {
        KanbanError::Storage(format!("stored stats status is invalid: {error}"))
    })?;
    Ok(StatusCountRecord {
        status,
        count: value.count,
    })
}

fn application_stale_claim(value: crate::domain::StaleClaimRecord) -> StaleClaimRecord {
    StaleClaimRecord {
        task_id: value.task_id,
        seq: value.seq,
        title: value.title,
        claim_owner: value.claim_owner,
        claim_expires_at: value.claim_expires_at,
        last_heartbeat_at: value.last_heartbeat_at,
        current_run_id: value.current_run_id,
        retry_count: value.retry_count,
        max_retries: value.max_retries,
    }
}

fn application_blocked_reason(
    value: crate::domain::BlockedReasonCountRecord,
) -> BlockedReasonCountRecord {
    BlockedReasonCountRecord {
        reason: value.reason,
        count: value.count,
    }
}

#[cfg(test)]
mod tests {
    use kanban_core::KanbanError;

    use crate::operations::test_support::FixedClock;
    use crate::*;

    async fn service(name: &str) -> (tempfile::TempDir, KanbanService<FixedClock>) {
        let (directory, store, _path) = crate::test_support::store(name).await;
        store.initialize().await.expect("initialize");
        (
            directory,
            KanbanService::with_clock(TursoApplicationStore::new(store), FixedClock(100)),
        )
    }

    #[tokio::test]
    async fn stats_trims_board_and_uses_application_clock() {
        let (_directory, service) = service("stats-service").await;
        let stats = service.get_stats(" default ").await.unwrap();
        assert_eq!(stats.board_id, "b_default");
        assert_eq!(stats.generated_at, 100);
    }

    #[tokio::test]
    async fn stats_rejects_empty_board_without_calling_store() {
        let (_directory, service) = service("stats-service-errors").await;
        let error = service.get_stats("  ").await.unwrap_err();
        assert!(
            matches!(error, KanbanError::InvalidInput(message) if message == "board is required")
        );
    }
}
