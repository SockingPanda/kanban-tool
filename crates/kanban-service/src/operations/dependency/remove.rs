use kanban_core::{Clock, KanbanError, Result, new_event_id};

use crate::{DependencySnapshotRecord, KanbanService};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveDependencyCommand {
    pub child_task_id: String,
    pub parent_task_id: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveDependencyResult {
    pub removed: bool,
    pub dependencies: DependencySnapshotRecord,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn remove_dependency(
        &self,
        command: RemoveDependencyCommand,
    ) -> Result<RemoveDependencyResult> {
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
        let result = self
            .application
            .store
            .store
            .remove_dependency(
                child_task_id,
                parent_task_id,
                crate::store_operations::RemoveDependencyInput {
                    actor: actor.to_owned(),
                    event_id: new_event_id(),
                    now: self.clock.now_ms(),
                },
            )
            .await
            .map_err(crate::adapter::store_error)?;
        Ok(RemoveDependencyResult {
            removed: result.removed,
            dependencies: crate::adapter::application_dependency_snapshot(result.dependencies)?,
        })
    }
}
