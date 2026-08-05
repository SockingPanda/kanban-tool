use crate::{
    CompleteTaskRecord as ApplicationCompleteTask, TaskDone, TaskRecord as ApplicationTask,
};
use kanban_core::Result;
use crate::CompleteTaskInput as StoreCompleteTask;

use crate::adapter::{TursoApplicationStore, application_task, store_error};

impl TaskDone for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<ApplicationTask> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn complete_task(
        &self,
        task_id: &str,
        input: ApplicationCompleteTask,
    ) -> Result<ApplicationTask> {
        self.store
            .complete_task(
                task_id,
                StoreCompleteTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    claim_token: input.claim_token,
                    force: input.force,
                    summary: input.summary,
                    result_json: input.result_json,
                    event_id: input.event_id,
                    now: input.now,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }
}
