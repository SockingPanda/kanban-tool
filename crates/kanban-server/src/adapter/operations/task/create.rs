use kanban_application::{
    CreateTaskRecord as ApplicationCreateTask, TaskCreate, TaskRecord as ApplicationTask,
};
use kanban_core::Result;
use kanban_store_turso::CreateTaskInput as StoreCreateTask;

use crate::adapter::{TursoApplicationStore, application_task, store_error};

impl TaskCreate for TursoApplicationStore {
    async fn create_task(
        &self,
        board: &str,
        input: ApplicationCreateTask,
    ) -> Result<ApplicationTask> {
        self.store
            .create_task(
                board,
                StoreCreateTask {
                    id: input.id,
                    idempotency_key: input.idempotency_key,
                    title: input.title,
                    status: input.status.as_str().to_owned(),
                    description: input.description,
                    assignee: input.assignee,
                    priority: input.priority,
                    scheduled_at: input.scheduled_at,
                    due_at: input.due_at,
                    max_retries: input.max_retries,
                    metadata_json: input.metadata_json,
                    labels: input.labels,
                    depends_on: input.depends_on,
                    created_by: input.created_by,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }
}
