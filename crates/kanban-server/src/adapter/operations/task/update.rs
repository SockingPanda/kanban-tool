use kanban_service::{
    TaskRecord as ApplicationTask, TaskUpdate, UpdateTaskRecord as ApplicationUpdateTask,
};
use kanban_core::Result;
use kanban_store_turso::UpdateTaskInput as StoreUpdateTask;

use crate::adapter::{TursoApplicationStore, application_task, store_error};

impl TaskUpdate for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<ApplicationTask> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn update_task(
        &self,
        task_id: &str,
        input: ApplicationUpdateTask,
    ) -> Result<ApplicationTask> {
        self.store
            .update_task(
                task_id,
                StoreUpdateTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    title: input.title,
                    description: input.description,
                    assignee: input.assignee,
                    priority: input.priority,
                    scheduled_at: input.scheduled_at,
                    due_at: input.due_at,
                    max_retries: input.max_retries,
                    metadata_json: input.metadata_json,
                    event_id: input.event_id,
                    now: input.now,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }
}
