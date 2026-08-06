use kanban_core::{
    Clock, KanbanError, ReadinessFacts, Result, TaskStatus, new_event_id, recompute_ready_status,
};

use crate::{DependencySnapshotRecord, KanbanService};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddDependencyCommand {
    pub child_task_id: String,
    pub parent_task_id: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddDependencyResult {
    pub added: bool,
    pub dependencies: DependencySnapshotRecord,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn add_dependency(
        &self,
        command: AddDependencyCommand,
    ) -> Result<AddDependencyResult> {
        let child_task_id = command.child_task_id.trim();
        let parent_task_id = command.parent_task_id.trim();
        if !child_task_id.starts_with("t_") || child_task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "child_task_id must be a global t_... id".to_owned(),
            ));
        }
        if !parent_task_id.starts_with("t_") || parent_task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "parent_task_id must be a global t_... id".to_owned(),
            ));
        }
        if child_task_id == parent_task_id {
            return Err(KanbanError::InvalidInput(
                "dependency cannot point to itself".to_owned(),
            ));
        }
        let actor = command.actor.trim();
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("actor is required".to_owned()));
        }
        let _mutation = self.mutation_gate.lock().await;
        let child = self.get_task(child_task_id).await?;
        let parent = self.get_task(parent_task_id).await?;
        if child.board_id != parent.board_id {
            return Err(KanbanError::InvalidInput(
                "cross-board dependency is not allowed".to_owned(),
            ));
        }
        if child.archived_at.is_some() || child.status == TaskStatus::Archived {
            return Err(KanbanError::InvalidTransition(
                "archived child task cannot receive dependencies".to_owned(),
            ));
        }
        if child.status == TaskStatus::Running
            && !matches!(parent.status, TaskStatus::Done | TaskStatus::Archived)
            && parent.archived_at.is_none()
        {
            return Err(KanbanError::InvalidTransition(
                "cannot add incomplete dependency to running task".to_owned(),
            ));
        }
        let now = self.clock.now_ms();
        let target_child_status = if matches!(
            child.status,
            TaskStatus::Triage | TaskStatus::Todo | TaskStatus::Scheduled | TaskStatus::Ready
        ) {
            let dependencies_done = !child.dependency_blocked
                && (matches!(parent.status, TaskStatus::Done | TaskStatus::Archived)
                    || parent.archived_at.is_some());
            let recomputed = recompute_ready_status(
                ReadinessFacts {
                    title: &child.title,
                    description: child.description.as_deref(),
                    scheduled_at: child.scheduled_at,
                    dependencies_done,
                },
                now,
            );
            if recomputed == TaskStatus::Ready {
                child.status
            } else {
                recomputed
            }
        } else {
            child.status
        };
        let result = self
            .application
            .store
            .store
            .add_dependency(
                child_task_id,
                parent_task_id,
                crate::store_operations::AddDependencyInput {
                    expected_child_lock_version: child.lock_version,
                    target_child_status: target_child_status.as_str().to_owned(),
                    actor: actor.to_owned(),
                    event_id: new_event_id(),
                    recompute_event_id: new_event_id(),
                    now,
                },
            )
            .await
            .map_err(crate::adapter::store_error)?;
        Ok(AddDependencyResult {
            added: result.added,
            dependencies: crate::adapter::application_dependency_snapshot(result.dependencies)?,
        })
    }
}
