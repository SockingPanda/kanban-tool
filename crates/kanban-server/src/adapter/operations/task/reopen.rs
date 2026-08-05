use kanban_application::{
    ReopenTaskRecord as ApplicationReopenTask, TaskRecord as ApplicationTask, TaskReopen,
};
use kanban_core::Result;
use kanban_store_turso::ReopenTaskInput as StoreReopenTask;

use crate::adapter::{TursoApplicationStore, application_task, store_error};

impl TaskReopen for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<ApplicationTask> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn reopen_task(
        &self,
        task_id: &str,
        input: ApplicationReopenTask,
    ) -> Result<ApplicationTask> {
        self.store
            .reopen_task(
                task_id,
                StoreReopenTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    reason: input.reason,
                    event_id: input.event_id,
                    now: input.now,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }
}
