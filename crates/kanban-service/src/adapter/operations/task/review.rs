use crate::SubmitReviewTaskInput as StoreSubmitReviewTask;
use crate::{
    SubmitReviewTaskRecord as ApplicationSubmitReviewTask, TaskRecord as ApplicationTask,
    TaskReview,
};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, store_error};
use crate::operations::application_task;

impl TaskReview for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<ApplicationTask> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn submit_review_task(
        &self,
        task_id: &str,
        input: ApplicationSubmitReviewTask,
    ) -> Result<ApplicationTask> {
        self.store
            .submit_review_task(
                task_id,
                StoreSubmitReviewTask {
                    expected_lock_version: input.expected_lock_version,
                    actor: input.actor,
                    claim_token: input.claim_token,
                    force: input.force,
                    summary: input.summary,
                    event_id: input.event_id,
                    now: input.now,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task)
    }
}
