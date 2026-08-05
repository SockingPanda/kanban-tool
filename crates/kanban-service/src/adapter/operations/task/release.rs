use crate::ReleaseTaskInput as StoreReleaseTask;
use crate::{
    ReleaseTaskRecord as ApplicationReleaseTask, TaskRecord as ApplicationTask, TaskRelease,
};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, application_task, store_error};

impl TaskRelease for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<ApplicationTask> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn release_task(
        &self,
        task_id: &str,
        input: ApplicationReleaseTask,
    ) -> Result<ApplicationTask> {
        self.store
            .release_task(
                task_id,
                StoreReleaseTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    claim_token: input.claim_token,
                    event_id: input.event_id,
                    now: input.now,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }
}
