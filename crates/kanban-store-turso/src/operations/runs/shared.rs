use crate::{
    db::TursoStore,
    domain::TaskRunRecord,
    error::StoreError,
    shared::{first_row, run_from_row},
};

/// Canonical projection used by every run query.
pub(crate) const RUN_SELECT: &str = "SELECT id, board_id, task_id, status, worker_profile, worker_pid, claim_token, claim_owner, claim_expires_at, started_at, last_heartbeat_at, finished_at, exit_code, summary, error, log_path, metadata_json FROM task_runs";

/// Load one run by its global id while preserving the store's typed not-found error.
#[allow(dead_code)]
pub(super) async fn load_run(
    store: &TursoStore,
    run_id: &str,
) -> Result<TaskRunRecord, StoreError> {
    let connection = store.connection().await?;
    let row = first_row(
        connection
            .query(
                &format!("{RUN_SELECT} WHERE id = :run_id LIMIT 1"),
                [(":run_id", run_id)],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::RunNotFound(run_id.to_owned()),
        other => StoreError::Turso(other),
    })?;
    run_from_row(row)
}
