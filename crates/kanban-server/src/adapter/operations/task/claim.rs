use kanban_service::{
    ClaimRecord as ApplicationClaim, ClaimTaskRecord as ApplicationClaimTask, TaskClaim,
    TaskRecord as ApplicationTask,
};
use kanban_core::Result;
use kanban_store_turso::{ClaimTaskInput as StoreClaimTask, ClaimTaskRecord as StoreClaim};

use crate::adapter::{TursoApplicationStore, application_run, application_task, store_error};

impl TaskClaim for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<ApplicationTask> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn claim_task(
        &self,
        task_id: &str,
        input: ApplicationClaimTask,
    ) -> Result<ApplicationClaim> {
        self.store
            .claim_task(
                task_id,
                StoreClaimTask {
                    expected_lock_version: input.expected_lock_version,
                    owner: input.actor,
                    claim_token: input.claim_token,
                    run_id: input.run_id,
                    event_id: input.event_id,
                    worker_profile: input.worker_profile,
                    metadata_json: input.metadata_json,
                    log_path: input.log_path,
                    now: input.now,
                    claim_expires_at: input.claim_expires_at,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_claim)
    }
}

fn application_claim(claim: StoreClaim) -> Result<ApplicationClaim> {
    Ok(ApplicationClaim {
        task: application_task(claim.task)?,
        run: application_run(claim.run)?,
        claim_token: claim.claim_token,
        claim_expires_at: claim.claim_expires_at,
    })
}
