use crate::PromoteTaskInput as StorePromoteTask;
use crate::{
    PromoteTaskRecord as ApplicationPromoteTask, TaskPromote, TaskRecord as ApplicationTask,
};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, store_error};
use crate::operations::application_task;

impl TaskPromote for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<ApplicationTask> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn promote_task(
        &self,
        task_id: &str,
        input: ApplicationPromoteTask,
    ) -> Result<ApplicationTask> {
        self.store
            .promote_task(
                task_id,
                StorePromoteTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    event_id: input.event_id,
                    updated_at: input.updated_at,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }
}
