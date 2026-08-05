use std::future::Future;

use kanban_core::{Clock, KanbanError, Result, new_event_id};

use crate::{ApplicationService, ApplicationStore, DependencySnapshotRecord};

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

pub trait DependencyRemove: ApplicationStore {
    fn remove_dependency(
        &self,
        child_task_id: &str,
        parent_task_id: &str,
        actor: String,
        event_id: String,
        now: i64,
    ) -> impl Future<Output = Result<RemoveDependencyResult>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: DependencyRemove,
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
            .store
            .remove_dependency(
                child_task_id,
                parent_task_id,
                actor.to_owned(),
                new_event_id(),
                self.clock.now_ms(),
            )
            .await?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use kanban_core::{KanbanError, Result};

    use crate::operations::test_support::StubStore;
    use crate::*;

    impl DependencyRemove for StubStore {
        async fn remove_dependency(
            &self,
            _child_task_id: &str,
            _parent_task_id: &str,
            _actor: String,
            _event_id: String,
            _now: i64,
        ) -> Result<RemoveDependencyResult> {
            Err(KanbanError::FeatureNotAvailable(
                "dependency stub is not configured".to_owned(),
            ))
        }
    }
}
