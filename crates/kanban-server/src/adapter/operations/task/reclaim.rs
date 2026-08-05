use kanban_application::{
    ReclaimExpiredTaskRecord as ApplicationReclaimExpiredTask, TaskReclaim,
    TaskRecord as ApplicationTask,
};
use kanban_core::Result;
use kanban_store_turso::ReclaimExpiredTaskInput as StoreReclaimExpiredTask;

use crate::adapter::{TursoApplicationStore, application_task, store_error};

impl TaskReclaim for TursoApplicationStore {
    async fn list_expired_claims(&self, board: &str, now: i64) -> Result<Vec<ApplicationTask>> {
        self.store
            .list_expired_claims(board, now)
            .await
            .map_err(store_error)?
            .into_iter()
            .map(application_task)
            .collect()
    }

    async fn reclaim_expired_task(
        &self,
        task_id: &str,
        input: ApplicationReclaimExpiredTask,
    ) -> Result<Option<ApplicationTask>> {
        self.store
            .reclaim_expired_task(
                task_id,
                StoreReclaimExpiredTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    event_id: input.event_id,
                    target_status: input.target_status.as_str().to_owned(),
                    retry_count: input.retry_count,
                    reason: input.reason,
                    now: input.now,
                },
            )
            .await
            .map_err(store_error)?
            .map(application_task)
            .transpose()
    }
}
