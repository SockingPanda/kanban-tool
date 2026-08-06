use kanban_core::{Clock, KanbanError, Result};

use crate::{KanbanService, TaskRecord};

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn get_task(&self, task_id: &str) -> Result<TaskRecord> {
        let task_id = validate_task_id(task_id)?;
        self.application
            .store
            .store
            .get_task_global(task_id)
            .await
            .map_err(crate::adapter::store_error)
            .and_then(super::application_task)
    }
}

fn validate_task_id(task_id: &str) -> Result<&str> {
    let task_id = task_id.trim();
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(KanbanError::InvalidInput(
            "task_id must be a global t_... id".to_owned(),
        ));
    }
    Ok(task_id)
}

#[cfg(test)]
mod tests {
    use kanban_core::KanbanError;

    #[test]
    fn task_id_validation_accepts_global_id_and_rejects_selector() {
        super::validate_task_id(" t_show ").unwrap();
        let error = super::validate_task_id("default#1").unwrap_err();
        assert!(matches!(error, KanbanError::InvalidInput(_)));
    }
}
