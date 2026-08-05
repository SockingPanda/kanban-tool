use std::future::Future;

use kanban_core::{Clock, KanbanError, Result, new_event_id};

use crate::{ApplicationService, ApplicationStore, ExecutionPlanRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkExecutionPlanNotRequiredCommand {
    pub task_id: String,
    pub reason: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkExecutionPlanNotRequiredRecord {
    pub reason: String,
    pub actor: String,
    pub event_id: String,
    pub updated_at: i64,
}

pub trait TaskPlanNotRequired: ApplicationStore {
    fn mark_execution_plan_not_required(
        &self,
        task_id: &str,
        input: MarkExecutionPlanNotRequiredRecord,
    ) -> impl Future<Output = Result<ExecutionPlanRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: TaskPlanNotRequired,
    C: Clock,
{
    pub async fn mark_execution_plan_not_required(
        &self,
        command: MarkExecutionPlanNotRequiredCommand,
    ) -> Result<ExecutionPlanRecord> {
        let task_id = command.task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        let reason = command.reason.trim();
        if reason.is_empty() {
            return Err(KanbanError::InvalidInput(
                "execution plan not_required reason is required".to_owned(),
            ));
        }
        let actor = command.actor.trim();
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("actor is required".to_owned()));
        }
        let _mutation = self.mutation_gate.lock().await;
        self.store
            .mark_execution_plan_not_required(
                task_id,
                MarkExecutionPlanNotRequiredRecord {
                    reason: reason.to_owned(),
                    actor: actor.to_owned(),
                    event_id: new_event_id(),
                    updated_at: self.clock.now_ms(),
                },
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicUsize};

    use kanban_core::{KanbanError, Result};

    use crate::operations::test_support::{FixedClock, StubStore};
    use crate::*;

    impl TaskPlanNotRequired for StubStore {
        async fn mark_execution_plan_not_required(
            &self,
            task_id: &str,
            input: MarkExecutionPlanNotRequiredRecord,
        ) -> Result<ExecutionPlanRecord> {
            assert_eq!(task_id, "t_show");
            assert_eq!(input.reason, "small task");
            assert_eq!(input.actor, "tester");
            assert!(input.event_id.starts_with("e_"));
            assert_eq!(input.updated_at, 100);
            Ok(ExecutionPlanRecord {
                board_id: "b_default".into(),
                task_id: task_id.to_owned(),
                state: ExecutionPlanState::NotRequired,
                reason: Some(input.reason),
                updated_by: input.actor,
                updated_at: input.updated_at,
            })
        }
    }
    #[tokio::test]
    async fn mark_execution_plan_not_required_canonicalizes_command() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let plan = service
            .mark_execution_plan_not_required(MarkExecutionPlanNotRequiredCommand {
                task_id: " t_show ".into(),
                reason: " small task ".into(),
                actor: " tester ".into(),
            })
            .await
            .unwrap();
        assert_eq!(plan.state, ExecutionPlanState::NotRequired);
        assert_eq!(plan.reason.as_deref(), Some("small task"));

        for command in [
            MarkExecutionPlanNotRequiredCommand {
                task_id: "default#1".into(),
                reason: "small".into(),
                actor: "tester".into(),
            },
            MarkExecutionPlanNotRequiredCommand {
                task_id: "t_show".into(),
                reason: " ".into(),
                actor: "tester".into(),
            },
            MarkExecutionPlanNotRequiredCommand {
                task_id: "t_show".into(),
                reason: "small".into(),
                actor: " ".into(),
            },
        ] {
            assert!(matches!(
                service.mark_execution_plan_not_required(command).await,
                Err(KanbanError::InvalidInput(_))
            ));
        }
    }
}
