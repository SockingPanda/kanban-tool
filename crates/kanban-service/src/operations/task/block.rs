use std::future::Future;

use kanban_core::{Clock, KanbanError, Result, TaskStatus, new_event_id, running_claim_is_present};

use crate::{ApplicationService, ApplicationStore, TaskRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub reason: String,
    pub claim_token: Option<String>,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub reason: String,
    pub claim_token: Option<String>,
    pub force: bool,
    pub event_id: String,
    pub now: i64,
}

pub trait TaskBlock: ApplicationStore {
    fn get_task(&self, task_id: &str) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn block_task(
        &self,
        task_id: &str,
        input: BlockTaskRecord,
    ) -> impl Future<Output = Result<TaskRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: TaskBlock,
    C: Clock,
{
    pub async fn block_task(&self, command: BlockTaskCommand) -> Result<TaskRecord> {
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
        if command.reason.trim().is_empty() {
            return Err(KanbanError::InvalidInput(
                "block reason is required".to_owned(),
            ));
        }

        let _mutation = self.mutation_gate.lock().await;
        let task = self.store.get_task(task_id).await?;
        if !matches!(
            task.status,
            TaskStatus::Triage
                | TaskStatus::Todo
                | TaskStatus::Scheduled
                | TaskStatus::Ready
                | TaskStatus::Running
                | TaskStatus::Review
        ) {
            return Err(KanbanError::InvalidTransition(
                "cannot block task".to_owned(),
            ));
        }
        if task.status == TaskStatus::Running {
            if !running_claim_is_present(
                task.status,
                task.has_claim_token,
                task.current_run_id.is_some(),
            ) {
                return Err(KanbanError::InvalidTransition(
                    "block requires an active running claim".to_owned(),
                ));
            }
            if !command.force && task.claim_owner.as_deref() != Some(actor) {
                return Err(KanbanError::InvalidTransition(
                    "claim owner mismatch".to_owned(),
                ));
            }
        }

        self.store
            .block_task(
                task_id,
                BlockTaskRecord {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    reason: command.reason,
                    claim_token: command.claim_token,
                    force: command.force,
                    event_id: new_event_id(),
                    now: self.clock.now_ms(),
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

    impl TaskBlock for StubStore {
        async fn get_task(&self, task_id: &str) -> Result<TaskRecord> {
            Ok(task_for_id(task_id))
        }

        async fn block_task(&self, task_id: &str, input: BlockTaskRecord) -> Result<TaskRecord> {
            let source = task_for_id(task_id);
            assert_eq!(input.expected_lock_version, source.lock_version);
            assert!(matches!(input.actor.as_str(), "worker" | "admin"));
            assert_eq!(input.reason.trim(), "waiting");
            assert!(input.event_id.starts_with("e_"));
            assert_eq!(input.now, 100);
            if source.status == TaskStatus::Running
                && !input.force
                && input.claim_token.as_deref() != Some("claim_valid")
            {
                return Err(KanbanError::InvalidTransition(
                    "claim token mismatch".to_owned(),
                ));
            }
            let mut task = source;
            task.status = TaskStatus::Blocked;
            task.status_reason = Some(input.reason);
            task.has_claim_token = false;
            task.claim_owner = None;
            task.claim_expires_at = None;
            task.last_heartbeat_at = None;
            task.updated_at = input.now;
            task.lock_version += 1;
            Ok(task)
        }
    }
    #[tokio::test]
    async fn block_task_handles_running_and_non_running_sources() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let running = service
            .block_task(BlockTaskCommand {
                task_id: " t_block_running ".into(),
                actor: " worker ".into(),
                reason: " waiting ".into(),
                claim_token: Some("claim_valid".into()),
                force: false,
            })
            .await
            .unwrap();
        assert_eq!(running.status, TaskStatus::Blocked);
        assert_eq!(running.status_reason.as_deref(), Some(" waiting "));
        assert_eq!(running.current_run_id.as_deref(), Some("r_block"));
        assert!(!running.has_claim_token);
        assert_eq!(running.lock_version, 3);

        let todo = service
            .block_task(BlockTaskCommand {
                task_id: "t_block_todo".into(),
                actor: "worker".into(),
                reason: "waiting".into(),
                claim_token: None,
                force: false,
            })
            .await
            .unwrap();
        assert_eq!(todo.status, TaskStatus::Blocked);
        assert_eq!(todo.status_reason.as_deref(), Some("waiting"));
        assert_eq!(todo.lock_version, 1);
    }

    #[tokio::test]
    async fn block_task_enforces_identity_reason_source_and_running_credentials() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        for command in [
            BlockTaskCommand {
                task_id: "default#1".into(),
                actor: "worker".into(),
                reason: "waiting".into(),
                claim_token: None,
                force: false,
            },
            BlockTaskCommand {
                task_id: "t_block_todo".into(),
                actor: " ".into(),
                reason: "waiting".into(),
                claim_token: None,
                force: false,
            },
            BlockTaskCommand {
                task_id: "t_block_todo".into(),
                actor: "worker".into(),
                reason: " ".into(),
                claim_token: None,
                force: false,
            },
        ] {
            assert!(matches!(
                service.block_task(command).await,
                Err(KanbanError::InvalidInput(_))
            ));
        }

        for command in [
            BlockTaskCommand {
                task_id: "t_block_running".into(),
                actor: "other".into(),
                reason: "waiting".into(),
                claim_token: Some("claim_valid".into()),
                force: false,
            },
            BlockTaskCommand {
                task_id: "t_block_running".into(),
                actor: "worker".into(),
                reason: "waiting".into(),
                claim_token: None,
                force: false,
            },
            BlockTaskCommand {
                task_id: "t_block_done".into(),
                actor: "worker".into(),
                reason: "waiting".into(),
                claim_token: None,
                force: false,
            },
        ] {
            assert!(matches!(
                service.block_task(command).await,
                Err(KanbanError::InvalidTransition(_))
            ));
        }

        let forced = service
            .block_task(BlockTaskCommand {
                task_id: "t_block_running".into(),
                actor: "admin".into(),
                reason: "waiting".into(),
                claim_token: None,
                force: true,
            })
            .await
            .unwrap();
        assert_eq!(forced.status, TaskStatus::Blocked);
    }
}
