use crate::ArchiveTaskInput as StoreArchiveTask;
use crate::{
    ArchiveTaskRecord as ApplicationArchiveTask, TaskArchive, TaskRecord as ApplicationTask,
};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, application_task, store_error};

impl TaskArchive for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<ApplicationTask> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn archive_task(
        &self,
        task_id: &str,
        input: ApplicationArchiveTask,
    ) -> Result<ApplicationTask> {
        self.store
            .archive_task(
                task_id,
                StoreArchiveTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    force: input.force,
                    event_id: input.event_id,
                    now: input.now,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }
}
