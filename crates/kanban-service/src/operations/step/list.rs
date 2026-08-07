use kanban_core::{Clock, KanbanError, Result};

use crate::{KanbanService, TaskStepsRecord};

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn list_steps(&self, task_id: &str) -> Result<TaskStepsRecord> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        let steps = self
            .store
            .list_steps(task_id)
            .await
            .map_err(crate::error::store_error)?;
        super::application_steps(steps)
    }
}
