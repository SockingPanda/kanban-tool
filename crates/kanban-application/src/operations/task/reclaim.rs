use kanban_core::{
    Clock, KanbanError, ReadinessFacts, Result, TaskStatus, new_event_id, recompute_ready_status,
    retry_decision,
};

use crate::{ApplicationService, ApplicationStore, ExecutionPlanState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReclaimExpiredTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub event_id: String,
    pub target_status: TaskStatus,
    pub retry_count: i64,
    pub reason: String,
    pub now: i64,
}

impl<S, C> ApplicationService<S, C>
where
    S: ApplicationStore,
    C: Clock,
{
    /// Reclaim only expired running leases for the in-process dispatcher.
    pub async fn reclaim_expired(&self, board: &str, actor: &str) -> Result<usize> {
        let board = board.trim();
        if board.is_empty() {
            return Err(KanbanError::InvalidInput("board is required".to_owned()));
        }
        let actor = actor.trim();
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("actor is required".to_owned()));
        }
        let _mutation = self.mutation_gate.lock().await;
        let now = self.clock.now_ms();
        let expired = self.store.list_expired_claims(board, now).await?;
        let mut reclaimed = 0;
        for task in expired {
            let decision = retry_decision(task.retry_count, task.max_retries, TaskStatus::Ready);
            let mut target_status = decision.status;
            if target_status == TaskStatus::Ready {
                target_status = recompute_ready_status(
                    ReadinessFacts {
                        title: &task.title,
                        description: task.description.as_deref(),
                        scheduled_at: task.scheduled_at,
                        dependencies_done: !task.dependency_blocked,
                    },
                    now,
                );
                let has_executable_plan = task.execution_plan_state
                    == ExecutionPlanState::NotRequired
                    || task.required_step_count > 0
                    || task.optional_step_count > 0;
                if target_status == TaskStatus::Ready && !has_executable_plan {
                    target_status = TaskStatus::Todo;
                }
            }
            let reason = if decision.max_retries_reached {
                "max retries reached"
            } else {
                "claim expired"
            };
            if self
                .store
                .reclaim_expired_task(
                    &task.id,
                    ReclaimExpiredTaskRecord {
                        expected_lock_version: task.lock_version,
                        actor: actor.to_owned(),
                        event_id: new_event_id(),
                        target_status,
                        retry_count: decision.retry_count,
                        reason: reason.to_owned(),
                        now,
                    },
                )
                .await?
                .is_some()
            {
                reclaimed += 1;
            }
        }
        Ok(reclaimed)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicUsize};

    use kanban_core::KanbanError;

    use crate::operations::test_support::{FixedClock, StubStore};
    use crate::*;
    #[tokio::test]
    async fn reclaim_expired_uses_canonical_retry_and_readiness_decision() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        assert_eq!(
            service
                .reclaim_expired(" default ", " dispatcher ")
                .await
                .unwrap(),
            2
        );
        assert!(matches!(
            service.reclaim_expired("", "dispatcher").await,
            Err(KanbanError::InvalidInput(_))
        ));
        assert!(matches!(
            service.reclaim_expired("default", " ").await,
            Err(KanbanError::InvalidInput(_))
        ));
    }
}
