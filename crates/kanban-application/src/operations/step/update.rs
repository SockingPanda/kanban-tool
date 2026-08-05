use kanban_core::{Clock, KanbanError, Result, TaskStatus, new_event_id};

use crate::{ApplicationService, ApplicationStore, TaskStepsRecord};

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateStepCommand {
    pub task_id: String,
    pub step_id: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub linked_task_id: Option<String>,
    pub unlink_task: bool,
    pub position: Option<i64>,
    pub required: Option<bool>,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateStepRecord {
    pub title: Option<String>,
    pub body: Option<String>,
    pub linked_task_id: Option<String>,
    pub unlink_task: bool,
    pub position: Option<i64>,
    pub required: Option<bool>,
    pub updated_by: String,
    pub event_id: String,
    pub updated_at: i64,
    pub expected_lock_version: i64,
}

impl<S, C> ApplicationService<S, C>
where
    S: ApplicationStore,
    C: Clock,
{
    pub async fn update_step(&self, command: UpdateStepCommand) -> Result<TaskStepsRecord> {
        let task_id = command.task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        let step_id = command.step_id.trim();
        if !step_id.starts_with("step_") || step_id.len() <= 5 {
            return Err(KanbanError::InvalidInput(
                "step_id must be a global step_... id".to_owned(),
            ));
        }
        let actor = command.actor.trim();
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("actor is required".to_owned()));
        }
        if command
            .title
            .as_deref()
            .is_some_and(|title| title.trim().is_empty())
        {
            return Err(KanbanError::InvalidInput(
                "step title is required when provided".to_owned(),
            ));
        }
        if command.position.is_some_and(|position| position < 0) {
            return Err(KanbanError::InvalidInput(
                "step position must be non-negative".to_owned(),
            ));
        }
        if command.linked_task_id.is_some() && command.unlink_task {
            return Err(KanbanError::InvalidInput(
                "linked_task_ref and unlink_task cannot be used together".to_owned(),
            ));
        }
        if command.title.is_none()
            && command.body.is_none()
            && command.linked_task_id.is_none()
            && !command.unlink_task
            && command.position.is_none()
            && command.required.is_none()
        {
            return Err(KanbanError::InvalidInput(
                "step update requires at least one field".to_owned(),
            ));
        }
        let _mutation = self.mutation_gate.lock().await;
        let parent = self.store.get_task(task_id).await?;
        if parent.archived_at.is_some() || parent.status == TaskStatus::Archived {
            return Err(KanbanError::InvalidTransition(
                "archived parent task cannot receive step updates".to_owned(),
            ));
        }
        let linked_task_id = command
            .linked_task_id
            .map(|linked_task_id| {
                let linked_task_id = linked_task_id.trim().to_owned();
                if !linked_task_id.starts_with("t_") || linked_task_id.len() <= 2 {
                    return Err(KanbanError::InvalidInput(
                        "linked_task_id must be a global t_... id".to_owned(),
                    ));
                }
                Ok(linked_task_id)
            })
            .transpose()?;
        if let Some(linked_task_id) = linked_task_id.as_deref() {
            let linked = self.store.get_task(linked_task_id).await?;
            if linked.board_id != parent.board_id {
                return Err(KanbanError::InvalidInput(
                    "linked task must belong to the parent board".to_owned(),
                ));
            }
            if linked.id == parent.id {
                return Err(KanbanError::InvalidInput(
                    "step cannot link to its parent task".to_owned(),
                ));
            }
            if linked.archived_at.is_some() || linked.status == TaskStatus::Archived {
                return Err(KanbanError::InvalidInput(
                    "archived linked task is not allowed".to_owned(),
                ));
            }
        }
        let now = self.clock.now_ms();
        self.store
            .update_step(
                task_id,
                step_id,
                UpdateStepRecord {
                    title: command.title.map(|title| title.trim().to_owned()),
                    body: command.body.map(|body| body.trim().to_owned()),
                    linked_task_id,
                    unlink_task: command.unlink_task,
                    position: command.position,
                    required: command.required,
                    updated_by: actor.to_owned(),
                    event_id: new_event_id(),
                    updated_at: now,
                    expected_lock_version: parent.lock_version,
                },
            )
            .await?;
        self.store.list_steps(task_id).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicUsize};

    use kanban_core::KanbanError;

    use crate::operations::test_support::{FixedClock, StubStore};
    use crate::*;
    #[tokio::test]
    async fn update_step_validates_patch_and_preserves_null_body_semantics() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let updated = service
            .update_step(UpdateStepCommand {
                task_id: " t_step ".into(),
                step_id: " step_fixture ".into(),
                title: Some(" updated step ".into()),
                body: None,
                linked_task_id: None,
                unlink_task: false,
                position: Some(2048),
                required: Some(false),
                actor: " tester ".into(),
            })
            .await
            .unwrap();
        assert_eq!(updated.steps.len(), 0);
        assert_eq!(updated.execution_plan.state, ExecutionPlanState::Planned);

        let conflict = service
            .update_step(UpdateStepCommand {
                task_id: "t_step".into(),
                step_id: "step_fixture".into(),
                title: None,
                body: None,
                linked_task_id: Some("t_link".into()),
                unlink_task: true,
                position: None,
                required: None,
                actor: "tester".into(),
            })
            .await
            .expect_err("link and unlink are mutually exclusive");
        assert!(
            matches!(conflict, KanbanError::InvalidInput(message) if message.contains("together"))
        );

        let empty = service
            .update_step(UpdateStepCommand {
                task_id: "t_step".into(),
                step_id: "step_fixture".into(),
                title: None,
                body: None,
                linked_task_id: None,
                unlink_task: false,
                position: None,
                required: None,
                actor: "tester".into(),
            })
            .await
            .expect_err("empty patch is invalid");
        assert!(
            matches!(empty, KanbanError::InvalidInput(message) if message.contains("at least one"))
        );
    }
}
