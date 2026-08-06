use kanban_core::{Clock, KanbanError, Result};

use crate::{KanbanService, RunRecord};

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn list_runs(&self, task_id: &str) -> Result<Vec<RunRecord>> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        self.application
            .store
            .store
            .list_runs(task_id)
            .await
            .map_err(crate::adapter::store_error)?
            .into_iter()
            .map(super::application_run)
            .collect()
    }
}
