use kanban_core::{Clock, KanbanError, Result};

use crate::{DependencySnapshotRecord, KanbanService};

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn list_dependencies(&self, task_id: &str) -> Result<DependencySnapshotRecord> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        self.application
            .store
            .store
            .list_dependencies(task_id)
            .await
            .map_err(crate::adapter::store_error)
            .and_then(crate::adapter::application_dependency_snapshot)
    }
}
