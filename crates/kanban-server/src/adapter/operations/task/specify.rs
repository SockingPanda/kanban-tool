use kanban_service::{
    SpecifyTaskRecord as ApplicationSpecifyTask, TaskRecord as ApplicationTask, TaskSpecify,
};
use kanban_core::Result;
use kanban_store_turso::SpecifyTaskInput as StoreSpecifyTask;

use crate::adapter::{TursoApplicationStore, application_task, store_error};

impl TaskSpecify for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<ApplicationTask> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn specify_task(
        &self,
        task_id: &str,
        input: ApplicationSpecifyTask,
    ) -> Result<ApplicationTask> {
        self.store
            .specify_task(
                task_id,
                StoreSpecifyTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    description: input.description,
                    scheduled_at: input.scheduled_at,
                    event_id: input.event_id,
                    now: input.now,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }
}
