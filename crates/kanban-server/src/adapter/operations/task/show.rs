use kanban_application::{TaskRecord as ApplicationTask, TaskShow};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, application_task, store_error};

impl TaskShow for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<ApplicationTask> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }
}
