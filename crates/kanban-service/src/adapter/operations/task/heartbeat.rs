use crate::HeartbeatTaskInput as StoreHeartbeatTask;
use crate::{
    HeartbeatTaskRecord as ApplicationHeartbeatTask, TaskHeartbeat, TaskRecord as ApplicationTask,
};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, application_task, store_error};

impl TaskHeartbeat for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<ApplicationTask> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn heartbeat_task(
        &self,
        task_id: &str,
        input: ApplicationHeartbeatTask,
    ) -> Result<ApplicationTask> {
        self.store
            .heartbeat_task(
                task_id,
                StoreHeartbeatTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    claim_token: input.claim_token,
                    event_id: input.event_id,
                    note: input.note,
                    now: input.now,
                    claim_expires_at: input.claim_expires_at,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }
}
