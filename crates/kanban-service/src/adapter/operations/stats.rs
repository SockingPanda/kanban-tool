use crate::{
    BlockedReasonCountRecord as ApplicationBlockedReasonCount,
    QueueStatsRecord as ApplicationQueueStats, StaleClaimRecord as ApplicationStaleClaim,
    StatsQuery, StatusCountRecord as ApplicationStatusCount,
};
use kanban_core::{KanbanError, Result, TaskStatus};
use crate::domain::{
    BlockedReasonCountRecord as StoreBlockedReasonCount, QueueStatsRecord as StoreQueueStats,
    StaleClaimRecord as StoreStaleClaim, StatusCountRecord as StoreStatusCount,
};

use crate::adapter::{TursoApplicationStore, store_error};

impl StatsQuery for TursoApplicationStore {
    async fn get_stats(&self, board: &str, generated_at: i64) -> Result<ApplicationQueueStats> {
        self.store
            .get_stats(board, generated_at)
            .await
            .map_err(store_error)
            .and_then(application_stats)
    }
}

fn application_stats(stats: StoreQueueStats) -> Result<ApplicationQueueStats> {
    Ok(ApplicationQueueStats {
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

fn application_status_count(value: StoreStatusCount) -> Result<ApplicationStatusCount> {
    let status = value.status.parse::<TaskStatus>().map_err(|error| {
        KanbanError::Storage(format!("stored stats status is invalid: {error}"))
    })?;
    Ok(ApplicationStatusCount {
        status,
        count: value.count,
    })
}

fn application_stale_claim(value: StoreStaleClaim) -> ApplicationStaleClaim {
    ApplicationStaleClaim {
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

fn application_blocked_reason(value: StoreBlockedReasonCount) -> ApplicationBlockedReasonCount {
    ApplicationBlockedReasonCount {
        reason: value.reason,
        count: value.count,
    }
}
