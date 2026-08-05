use kanban_core::{Clock, KanbanError, Result, new_event_id, running_claim_is_present};

use crate::{ApplicationService, ApplicationStore, TaskRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub claim_token: String,
    pub ttl_ms: i64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub claim_token: String,
    pub event_id: String,
    pub note: Option<String>,
    pub now: i64,
    pub claim_expires_at: i64,
}

impl<S, C> ApplicationService<S, C>
where
    S: ApplicationStore,
    C: Clock,
{
    pub async fn heartbeat_task(&self, command: HeartbeatTaskCommand) -> Result<TaskRecord> {
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
        if command.ttl_ms <= 0 {
            return Err(KanbanError::InvalidInput(
                "ttl_ms must be positive".to_owned(),
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
                "heartbeat requires an active running claim".to_owned(),
            ));
        }
        if task.claim_owner.as_deref() != Some(actor) {
            return Err(KanbanError::InvalidTransition(
                "claim owner mismatch".to_owned(),
            ));
        }
        let now = self.clock.now_ms();
        let claim_expires_at = now.checked_add(command.ttl_ms).ok_or_else(|| {
            KanbanError::InvalidInput("ttl_ms produces an invalid claim expiry".to_owned())
        })?;
        self.store
            .heartbeat_task(
                task_id,
                HeartbeatTaskRecord {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    claim_token: command.claim_token,
                    event_id: new_event_id(),
                    note: command.note,
                    now,
                    claim_expires_at,
                },
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicUsize};

    use kanban_core::{KanbanError, TaskStatus};

    use crate::operations::test_support::{FixedClock, StubStore};
    use crate::*;
    #[tokio::test]
    async fn heartbeat_task_validates_running_claim_owner_and_lease() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let heartbeat = service
            .heartbeat_task(HeartbeatTaskCommand {
                task_id: " t_heartbeat ".into(),
                actor: " worker ".into(),
                claim_token: "claim_valid".into(),
                ttl_ms: 300,
                note: Some(" alive ".into()),
            })
            .await
            .unwrap();
        assert_eq!(heartbeat.status, TaskStatus::Running);
        assert_eq!(heartbeat.claim_expires_at, Some(400));
        assert_eq!(heartbeat.last_heartbeat_at, Some(100));
        assert_eq!(heartbeat.lock_version, 3);

        let padded_token = service
            .heartbeat_task(HeartbeatTaskCommand {
                task_id: "t_heartbeat".into(),
                actor: "worker".into(),
                claim_token: " claim_valid ".into(),
                ttl_ms: 300,
                note: None,
            })
            .await
            .unwrap_err();
        assert!(matches!(
            padded_token,
            KanbanError::InvalidTransition(message) if message.contains("claim token mismatch")
        ));

        let wrong_token = service
            .heartbeat_task(HeartbeatTaskCommand {
                task_id: "t_heartbeat".into(),
                actor: "worker".into(),
                claim_token: "wrong".into(),
                ttl_ms: 300,
                note: None,
            })
            .await
            .unwrap_err();
        assert!(matches!(
            wrong_token,
            KanbanError::InvalidTransition(message) if message.contains("claim token mismatch")
        ));

        let wrong_owner = service
            .heartbeat_task(HeartbeatTaskCommand {
                task_id: "t_heartbeat".into(),
                actor: "other".into(),
                claim_token: "claim_valid".into(),
                ttl_ms: 300,
                note: None,
            })
            .await
            .unwrap_err();
        assert!(matches!(
            wrong_owner,
            KanbanError::InvalidTransition(message) if message.contains("claim owner mismatch")
        ));

        let inactive = service
            .heartbeat_task(HeartbeatTaskCommand {
                task_id: "t_claim".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
                ttl_ms: 300,
                note: None,
            })
            .await
            .unwrap_err();
        assert!(matches!(inactive, KanbanError::InvalidTransition(_)));
    }

    #[tokio::test]
    async fn heartbeat_task_rejects_invalid_identity_token_and_lease() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(i64::MAX - 10),
        );
        for command in [
            HeartbeatTaskCommand {
                task_id: "default#1".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
                ttl_ms: 300,
                note: None,
            },
            HeartbeatTaskCommand {
                task_id: "t_heartbeat".into(),
                actor: " ".into(),
                claim_token: "claim_valid".into(),
                ttl_ms: 300,
                note: None,
            },
            HeartbeatTaskCommand {
                task_id: "t_heartbeat".into(),
                actor: "worker".into(),
                claim_token: " ".into(),
                ttl_ms: 300,
                note: None,
            },
            HeartbeatTaskCommand {
                task_id: "t_heartbeat".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
                ttl_ms: 0,
                note: None,
            },
            HeartbeatTaskCommand {
                task_id: "t_heartbeat".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
                ttl_ms: 20,
                note: None,
            },
        ] {
            assert!(matches!(
                service.heartbeat_task(command).await,
                Err(KanbanError::InvalidInput(_))
            ));
        }
    }
}
