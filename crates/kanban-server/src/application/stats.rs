use crate::{error::ApiError, state::AppState};
use kanban_protocol::{
    ApiTaskStatus, BlockedReasonCount, BoardQuery, DataEnvelope, QueueStats, StaleClaim,
    StatsResponse, StatusCount,
};
use kanban_service::TaskStatus;
pub(crate) async fn stats(state: AppState, query: BoardQuery) -> Result<StatsResponse, ApiError> {
    let value = state.application().get_stats(&query.board).await?;
    Ok(DataEnvelope::new(QueueStats {
        board_id: value.board_id,
        generated_at: value.generated_at,
        status_counts: value
            .status_counts
            .into_iter()
            .map(|count| StatusCount {
                status: api_task_status(count.status),
                count: count.count,
            })
            .collect(),
        stale_claims: value
            .stale_claims
            .into_iter()
            .map(|claim| StaleClaim {
                task_id: claim.task_id,
                seq: claim.seq,
                title: claim.title,
                claim_owner: claim.claim_owner,
                claim_expires_at: claim.claim_expires_at,
                last_heartbeat_at: claim.last_heartbeat_at,
                current_run_id: claim.current_run_id,
                retry_count: claim.retry_count,
                max_retries: claim.max_retries,
            })
            .collect(),
        blocked_reasons: value
            .blocked_reasons
            .into_iter()
            .map(|reason| BlockedReasonCount {
                reason: reason.reason,
                count: reason.count,
            })
            .collect(),
        unplanned_active_tasks: value.unplanned_active_tasks,
        active_parents_with_incomplete_required_steps: value
            .active_parents_with_incomplete_required_steps,
    }))
}
fn api_task_status(status: TaskStatus) -> ApiTaskStatus {
    match status {
        TaskStatus::Triage => ApiTaskStatus::Triage,
        TaskStatus::Todo => ApiTaskStatus::Todo,
        TaskStatus::Scheduled => ApiTaskStatus::Scheduled,
        TaskStatus::Ready => ApiTaskStatus::Ready,
        TaskStatus::Running => ApiTaskStatus::Running,
        TaskStatus::Blocked => ApiTaskStatus::Blocked,
        TaskStatus::Review => ApiTaskStatus::Review,
        TaskStatus::Done => ApiTaskStatus::Done,
        TaskStatus::Archived => ApiTaskStatus::Archived,
    }
}
