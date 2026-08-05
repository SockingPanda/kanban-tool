use kanban_core::{Clock, KanbanError, Result, new_event_id, running_claim_is_present};

use crate::{ApplicationService, ApplicationStore, TaskRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitReviewTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub claim_token: Option<String>,
    pub force: bool,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitReviewTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub claim_token: Option<String>,
    pub force: bool,
    pub summary: Option<String>,
    pub event_id: String,
    pub now: i64,
}

impl<S, C> ApplicationService<S, C>
where
    S: ApplicationStore,
    C: Clock,
{
    pub async fn submit_review_task(&self, command: SubmitReviewTaskCommand) -> Result<TaskRecord> {
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
        let _mutation = self.mutation_gate.lock().await;
        let task = self.store.get_task(task_id).await?;
        if !running_claim_is_present(
            task.status,
            task.has_claim_token,
            task.current_run_id.is_some(),
        ) {
            return Err(KanbanError::InvalidTransition(
                "review requires an active running claim".to_owned(),
            ));
        }
        if !command.force && task.claim_owner.as_deref() != Some(actor) {
            return Err(KanbanError::InvalidTransition(
                "claim owner mismatch".to_owned(),
            ));
        }

        self.store
            .submit_review_task(
                task_id,
                SubmitReviewTaskRecord {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    claim_token: command.claim_token,
                    force: command.force,
                    summary: command.summary,
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

    use kanban_core::{KanbanError, TaskStatus};

    use crate::operations::test_support::{FixedClock, StubStore};
    use crate::*;
    #[tokio::test]
    async fn submit_review_task_validates_owner_and_preserves_exact_token() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let reviewed = service
            .submit_review_task(SubmitReviewTaskCommand {
                task_id: " t_review ".into(),
                actor: " worker ".into(),
                claim_token: Some("claim_valid".into()),
                force: false,
                summary: Some("ready for review".into()),
            })
            .await
            .unwrap();
        assert_eq!(reviewed.status, TaskStatus::Review);
        assert!(!reviewed.has_claim_token);
        assert_eq!(reviewed.claim_owner, None);
        assert_eq!(reviewed.claim_expires_at, None);
        assert_eq!(reviewed.last_heartbeat_at, None);
        assert_eq!(reviewed.current_run_id.as_deref(), Some("r_review"));
        assert_eq!(reviewed.result_summary.as_deref(), Some("ready for review"));
        assert_eq!(reviewed.lock_version, 3);

        for command in [
            SubmitReviewTaskCommand {
                task_id: "t_review".into(),
                actor: "other".into(),
                claim_token: Some("claim_valid".into()),
                force: false,
                summary: None,
            },
            SubmitReviewTaskCommand {
                task_id: "t_review".into(),
                actor: "worker".into(),
                claim_token: Some(" claim_valid ".into()),
                force: false,
                summary: None,
            },
            SubmitReviewTaskCommand {
                task_id: "t_claim".into(),
                actor: "worker".into(),
                claim_token: Some("claim_valid".into()),
                force: false,
                summary: None,
            },
        ] {
            assert!(matches!(
                service.submit_review_task(command).await,
                Err(KanbanError::InvalidTransition(_))
            ));
        }

        let forced = service
            .submit_review_task(SubmitReviewTaskCommand {
                task_id: "t_review".into(),
                actor: "worker".into(),
                claim_token: None,
                force: true,
                summary: None,
            })
            .await
            .unwrap();
        assert_eq!(forced.status, TaskStatus::Review);
    }

    #[tokio::test]
    async fn submit_review_task_rejects_invalid_identity_and_credentials() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        for command in [
            SubmitReviewTaskCommand {
                task_id: "default#1".into(),
                actor: "worker".into(),
                claim_token: Some("claim_valid".into()),
                force: false,
                summary: None,
            },
            SubmitReviewTaskCommand {
                task_id: "t_review".into(),
                actor: " ".into(),
                claim_token: Some("claim_valid".into()),
                force: false,
                summary: None,
            },
        ] {
            assert!(matches!(
                service.submit_review_task(command).await,
                Err(KanbanError::InvalidInput(_))
            ));
        }

        for claim_token in [None, Some(" ".into())] {
            let error = service
                .submit_review_task(SubmitReviewTaskCommand {
                    task_id: "t_review".into(),
                    actor: "worker".into(),
                    claim_token,
                    force: false,
                    summary: None,
                })
                .await
                .unwrap_err();
            assert!(matches!(
                error,
                KanbanError::InvalidTransition(message)
                    if message.contains("claim token mismatch")
            ));
        }
    }
}
