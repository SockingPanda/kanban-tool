use kanban_core::{Clock, KanbanError, Result, TaskStatus, new_event_id, running_claim_is_present};

use crate::{ApplicationService, ApplicationStore, TaskRecord};

#[derive(Debug, Clone, PartialEq)]
pub struct CompleteTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub claim_token: Option<String>,
    pub force: bool,
    pub summary: Option<String>,
    pub result: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub claim_token: Option<String>,
    pub force: bool,
    pub summary: Option<String>,
    pub result_json: Option<String>,
    pub event_id: String,
    pub now: i64,
}

impl<S, C> ApplicationService<S, C>
where
    S: ApplicationStore,
    C: Clock,
{
    pub async fn complete_task(&self, command: CompleteTaskCommand) -> Result<TaskRecord> {
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
        let result_json = command
            .result
            .map(|result| serde_json::to_string(&result))
            .transpose()
            .map_err(|error| KanbanError::InvalidInput(format!("invalid result: {error}")))?;

        let _mutation = self.mutation_gate.lock().await;
        let task = self.store.get_task(task_id).await?;
        if !matches!(task.status, TaskStatus::Running | TaskStatus::Review) {
            return Err(KanbanError::InvalidTransition(
                "complete requires running or review".to_owned(),
            ));
        }
        if task.status == TaskStatus::Running {
            if !running_claim_is_present(
                task.status,
                task.has_claim_token,
                task.current_run_id.is_some(),
            ) {
                return Err(KanbanError::InvalidTransition(
                    "complete requires an active running claim".to_owned(),
                ));
            }
            if !command.force && task.claim_owner.as_deref() != Some(actor) {
                return Err(KanbanError::InvalidTransition(
                    "claim owner mismatch".to_owned(),
                ));
            }
        }
        if task.completed_required_step_count != task.required_step_count {
            return Err(KanbanError::StepsIncomplete(format!(
                "{} required step(s) remain incomplete",
                task.required_step_count
                    .saturating_sub(task.completed_required_step_count)
            )));
        }

        self.store
            .complete_task(
                task_id,
                CompleteTaskRecord {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    claim_token: command.claim_token,
                    force: command.force,
                    summary: command.summary,
                    result_json,
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
    async fn complete_task_handles_running_and_review_sources() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let completed = service
            .complete_task(CompleteTaskCommand {
                task_id: " t_complete_running ".into(),
                actor: " worker ".into(),
                claim_token: Some("claim_valid".into()),
                force: false,
                summary: Some("finished".into()),
                result: Some(serde_json::json!({"ok": true})),
            })
            .await
            .unwrap();
        assert_eq!(completed.status, TaskStatus::Done);
        assert_eq!(completed.completed_at, Some(100));
        assert_eq!(completed.current_run_id.as_deref(), Some("r_complete"));
        assert_eq!(completed.result_summary.as_deref(), Some("finished"));
        assert_eq!(completed.result_json.as_deref(), Some(r#"{"ok":true}"#));
        assert!(!completed.has_claim_token);
        assert_eq!(completed.lock_version, 3);

        let completed = service
            .complete_task(CompleteTaskCommand {
                task_id: "t_complete_review".into(),
                actor: "worker".into(),
                claim_token: None,
                force: false,
                summary: None,
                result: None,
            })
            .await
            .unwrap();
        assert_eq!(completed.status, TaskStatus::Done);
        assert_eq!(completed.current_run_id.as_deref(), Some("r_complete"));
        assert_eq!(completed.lock_version, 4);

        let forced = service
            .complete_task(CompleteTaskCommand {
                task_id: "t_complete_running".into(),
                actor: "worker".into(),
                claim_token: None,
                force: true,
                summary: None,
                result: None,
            })
            .await
            .unwrap();
        assert_eq!(forced.status, TaskStatus::Done);
    }

    #[tokio::test]
    async fn complete_task_enforces_credentials_source_and_required_steps() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        for command in [
            CompleteTaskCommand {
                task_id: "t_complete_running".into(),
                actor: "other".into(),
                claim_token: Some("claim_valid".into()),
                force: false,
                summary: None,
                result: None,
            },
            CompleteTaskCommand {
                task_id: "t_complete_running".into(),
                actor: "worker".into(),
                claim_token: None,
                force: false,
                summary: None,
                result: None,
            },
            CompleteTaskCommand {
                task_id: "t_complete_running".into(),
                actor: "worker".into(),
                claim_token: Some(" claim_valid ".into()),
                force: false,
                summary: None,
                result: None,
            },
            CompleteTaskCommand {
                task_id: "t_claim".into(),
                actor: "worker".into(),
                claim_token: Some("claim_valid".into()),
                force: false,
                summary: None,
                result: None,
            },
        ] {
            assert!(matches!(
                service.complete_task(command).await,
                Err(KanbanError::InvalidTransition(_))
            ));
        }

        for force in [false, true] {
            let error = service
                .complete_task(CompleteTaskCommand {
                    task_id: "t_complete_steps".into(),
                    actor: "worker".into(),
                    claim_token: None,
                    force,
                    summary: None,
                    result: None,
                })
                .await
                .unwrap_err();
            assert!(matches!(error, KanbanError::StepsIncomplete(_)));
        }
    }

    #[tokio::test]
    async fn complete_task_rejects_invalid_identity() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        for command in [
            CompleteTaskCommand {
                task_id: "default#1".into(),
                actor: "worker".into(),
                claim_token: None,
                force: true,
                summary: None,
                result: None,
            },
            CompleteTaskCommand {
                task_id: "t_complete_review".into(),
                actor: " ".into(),
                claim_token: None,
                force: false,
                summary: None,
                result: None,
            },
        ] {
            assert!(matches!(
                service.complete_task(command).await,
                Err(KanbanError::InvalidInput(_))
            ));
        }
    }
}
