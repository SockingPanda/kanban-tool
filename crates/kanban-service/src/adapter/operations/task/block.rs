use crate::BlockTaskInput as StoreBlockTask;
use crate::{BlockTaskRecord as ApplicationBlockTask, TaskBlock, TaskRecord as ApplicationTask};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, store_error};
use crate::operations::application_task;

impl TaskBlock for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<ApplicationTask> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn block_task(
        &self,
        task_id: &str,
        input: ApplicationBlockTask,
    ) -> Result<ApplicationTask> {
        self.store
            .block_task(
                task_id,
                StoreBlockTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    reason: input.reason,
                    claim_token: input.claim_token,
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
