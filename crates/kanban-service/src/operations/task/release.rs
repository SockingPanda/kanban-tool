use std::future::Future;

use kanban_core::{
    Clock, KanbanError, ReadinessFacts, Result, TaskStatus, new_event_id, recompute_ready_status,
    running_claim_is_present,
};

use crate::{ApplicationService, ApplicationStore, ExecutionPlanState, TaskRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub claim_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub claim_token: String,
    pub event_id: String,
    pub now: i64,
}

pub trait TaskRelease: ApplicationStore {
    fn get_task(&self, task_id: &str) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn release_task(
        &self,
        task_id: &str,
        input: ReleaseTaskRecord,
    ) -> impl Future<Output = Result<TaskRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: TaskRelease,
    C: Clock,
{
    pub async fn release_task(&self, command: ReleaseTaskCommand) -> Result<TaskRecord> {
        let task_id = command.task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        let actor = command.actor.trim();
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("actor is required".to_owned()));
        }
        if command.claim_token.trim().is_empty() {
            return Err(KanbanError::InvalidInput(
                "claim_token is required".to_owned(),
            ));
        }
        let _mutation = self.mutation_gate.lock().await;
        let task = self.store.get_task(task_id).await?;
        if !running_claim_is_present(
            task.status,
            task.has_claim_token,
            task.current_run_id.is_some(),
        ) {
            return Err(KanbanError::InvalidTransition(
                "release requires an active running claim".to_owned(),
            ));
        }
        if task.claim_owner.as_deref() != Some(actor) {
            return Err(KanbanError::InvalidTransition(
                "claim owner mismatch".to_owned(),
            ));
        }
        if task.execution_plan_state == ExecutionPlanState::Unplanned {
            return Err(KanbanError::ExecutionPlanRequired(
                "add steps or mark execution plan not_required before releasing task".to_owned(),
            ));
        }
        let now = self.clock.now_ms();
        let target = recompute_ready_status(
            ReadinessFacts {
                title: &task.title,
                description: task.description.as_deref(),
                scheduled_at: task.scheduled_at,
                dependencies_done: !task.dependency_blocked,
            },
            now,
        );
        if target != TaskStatus::Ready {
            return Err(KanbanError::InvalidTransition(match target {
                TaskStatus::Todo => "dependency blocked".to_owned(),
                TaskStatus::Scheduled => "scheduled_at is in the future".to_owned(),
                TaskStatus::Triage => "task spec is incomplete".to_owned(),
                _ => format!("cannot release to {}", target.as_str()),
            }));
        }
        self.store
            .release_task(
                task_id,
                ReleaseTaskRecord {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    claim_token: command.claim_token,
                    event_id: new_event_id(),
                    now,
                },
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicUsize};

    use kanban_core::{KanbanError, Result, TaskStatus};

    use crate::operations::test_support::{FixedClock, StubStore, task_for_id};
    use crate::*;

    impl TaskRelease for StubStore {
        async fn get_task(&self, task_id: &str) -> Result<TaskRecord> {
            Ok(task_for_id(task_id))
        }

        async fn release_task(
            &self,
            task_id: &str,
            input: ReleaseTaskRecord,
        ) -> Result<TaskRecord> {
            assert_eq!(task_id, "t_release");
            assert_eq!(input.expected_lock_version, 2);
            assert_eq!(input.actor, "worker");
            assert!(input.event_id.starts_with("e_"));
            assert_eq!(input.now, 100);
            if input.claim_token != "claim_valid" {
                return Err(KanbanError::InvalidTransition(
                    "claim token mismatch".to_owned(),
                ));
            }
            let mut task = task_for_id(task_id);
            task.status = TaskStatus::Ready;
            task.has_claim_token = false;
            task.claim_owner = None;
            task.claim_expires_at = None;
            task.last_heartbeat_at = None;
            task.current_run_id = None;
            task.updated_at = input.now;
            task.lock_version += 1;
            Ok(task)
        }
    }
    #[tokio::test]
    async fn release_task_validates_readiness_owner_and_exact_token() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let released = service
            .release_task(ReleaseTaskCommand {
                task_id: " t_release ".into(),
                actor: " worker ".into(),
                claim_token: "claim_valid".into(),
            })
            .await
            .unwrap();
        assert_eq!(released.status, TaskStatus::Ready);
        assert!(!released.has_claim_token);
        assert_eq!(released.claim_owner, None);
        assert_eq!(released.claim_expires_at, None);
        assert_eq!(released.last_heartbeat_at, None);
        assert_eq!(released.current_run_id, None);
        assert_eq!(released.lock_version, 3);

        let padded_token = service
            .release_task(ReleaseTaskCommand {
                task_id: "t_release".into(),
                actor: "worker".into(),
                claim_token: " claim_valid ".into(),
            })
            .await
            .unwrap_err();
        assert!(matches!(
            padded_token,
            KanbanError::InvalidTransition(message) if message.contains("claim token mismatch")
        ));

        for command in [
            ReleaseTaskCommand {
                task_id: "t_release".into(),
                actor: "other".into(),
                claim_token: "claim_valid".into(),
            },
            ReleaseTaskCommand {
                task_id: "t_claim".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
            },
            ReleaseTaskCommand {
                task_id: "t_release_dependency".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
            },
            ReleaseTaskCommand {
                task_id: "t_release_future".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
            },
        ] {
            assert!(matches!(
                service.release_task(command).await,
                Err(KanbanError::InvalidTransition(_))
            ));
        }

        let unplanned = service
            .release_task(ReleaseTaskCommand {
                task_id: "t_release_unplanned".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
            })
            .await
            .unwrap_err();
        assert!(matches!(unplanned, KanbanError::ExecutionPlanRequired(_)));
    }

    #[tokio::test]
    async fn release_task_rejects_invalid_identity_and_token() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        for command in [
            ReleaseTaskCommand {
                task_id: "default#1".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
            },
            ReleaseTaskCommand {
                task_id: "t_release".into(),
                actor: " ".into(),
                claim_token: "claim_valid".into(),
            },
            ReleaseTaskCommand {
                task_id: "t_release".into(),
                actor: "worker".into(),
                claim_token: " ".into(),
            },
        ] {
            assert!(matches!(
                service.release_task(command).await,
                Err(KanbanError::InvalidInput(_))
            ));
        }
    }
}
