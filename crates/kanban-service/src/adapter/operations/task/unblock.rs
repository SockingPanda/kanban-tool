use crate::UnblockTaskInput as StoreUnblockTask;
use crate::{
    TaskRecord as ApplicationTask, TaskUnblock, UnblockTaskRecord as ApplicationUnblockTask,
};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, store_error};
use crate::operations::application_task;

impl TaskUnblock for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<ApplicationTask> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn unblock_task(
        &self,
        task_id: &str,
        input: ApplicationUnblockTask,
    ) -> Result<ApplicationTask> {
        self.store
            .unblock_task(
                task_id,
                StoreUnblockTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    event_id: input.event_id,
                    now: input.now,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }
}
