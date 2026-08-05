use kanban_application::{
    ClaimRecord as ApplicationClaim, ClaimTaskRecord as ApplicationClaimTask,
    RunRecord as ApplicationRun, RunStatus as ApplicationRunStatus, TaskClaim,
    TaskRecord as ApplicationTask,
};
use kanban_core::{KanbanError, Result};
use kanban_store_turso::{ClaimTaskInput as StoreClaimTask, ClaimTaskRecord as StoreClaim};

use crate::adapter::{TursoApplicationStore, application_task, store_error};

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

fn application_run(run: kanban_store_turso::TaskRunRecord) -> Result<ApplicationRun> {
    let status = match run.status.as_str() {
        "running" => ApplicationRunStatus::Running,
        "succeeded" => ApplicationRunStatus::Succeeded,
        "failed" => ApplicationRunStatus::Failed,
        "canceled" => ApplicationRunStatus::Canceled,
        "expired" => ApplicationRunStatus::Expired,
        other => {
            return Err(KanbanError::Storage(format!(
                "stored run status is invalid: {other}"
            )));
        }
    };
    Ok(ApplicationRun {
        id: run.id,
        board_id: run.board_id,
        task_id: run.task_id,
        status,
        worker_profile: run.worker_profile,
        worker_pid: run.worker_pid,
        claim_owner: run.claim_owner,
        claim_expires_at: run.claim_expires_at,
        started_at: run.started_at,
        last_heartbeat_at: run.last_heartbeat_at,
        finished_at: run.finished_at,
        exit_code: run.exit_code,
        summary: run.summary,
        error: run.error,
        log_path: run.log_path,
        metadata_json: run.metadata_json,
    })
}
