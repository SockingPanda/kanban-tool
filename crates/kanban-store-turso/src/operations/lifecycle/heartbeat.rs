use turso::transaction::TransactionBehavior;

use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatTaskInput {
    pub expected_lock_version: i64,
    pub actor: String,
    pub claim_token: String,
    pub event_id: String,
    pub note: Option<String>,
    pub now: i64,
    pub claim_expires_at: i64,
}
impl TursoStore {
    pub async fn heartbeat_task(
        &self,
        task_id: &str,
        input: HeartbeatTaskInput,
    ) -> Result<TaskRecord, StoreError> {
        validate_heartbeat_task_input(task_id, &input)?;
        let actor = input.actor.trim().to_owned();
        let claim_token = input.claim_token.as_str();
        let event_id = input.event_id.trim().to_owned();
        let note = input.note.as_deref();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let task = first_row(
                transaction
                    .query(
                        "SELECT t.board_id, t.status, t.archived_at, b.archived_at, t.lock_version, t.claim_token, t.claim_owner, t.claim_expires_at, t.current_run_id FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
                        [(":task_id", task_id)],
                    )
                    .await?,
            )
            .await
            .map_err(|error| match error {
                turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(task_id.to_owned()),
                other => StoreError::Turso(other),
            })?;
        let board_id = text_value(task.get_value(0)?, "tasks.board_id")?;
        let status = text_value(task.get_value(1)?, "tasks.status")?;
        let task_archived_at = optional_integer_value(task.get_value(2)?, "tasks.archived_at")?;
        let board_archived_at = optional_integer_value(task.get_value(3)?, "boards.archived_at")?;
        if task_archived_at.is_some() || board_archived_at.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived task or board cannot be heartbeated".to_owned(),
            ));
        }
        if status != "running" {
            return Err(StoreError::InvalidTransition(
                "heartbeat requires a running task".to_owned(),
            ));
        }

        let lock_version = integer_value(task.get_value(4)?, "tasks.lock_version")?;
        if lock_version != input.expected_lock_version {
            return Err(StoreError::ClaimConflict(
                "lock_version mismatch".to_owned(),
            ));
        }
        let task_claim_token = optional_text_value(task.get_value(5)?, "tasks.claim_token")?;
        if task_claim_token.as_deref() != Some(claim_token) {
            return Err(StoreError::ClaimTokenMismatch);
        }
        let task_claim_owner = optional_text_value(task.get_value(6)?, "tasks.claim_owner")?;
        if task_claim_owner.as_deref() != Some(actor.as_str()) {
            return Err(StoreError::InvalidTransition(
                "claim owner mismatch".to_owned(),
            ));
        }
        if optional_integer_value(task.get_value(7)?, "tasks.claim_expires_at")?.is_none() {
            return Err(StoreError::InvalidTransition(
                "heartbeat requires an active claim".to_owned(),
            ));
        }
        let run_id = optional_text_value(task.get_value(8)?, "tasks.current_run_id")?
            .filter(|run_id| !run_id.trim().is_empty())
            .ok_or_else(|| {
                StoreError::InvalidTransition("heartbeat requires a current running run".to_owned())
            })?;

        let active_run_count = first_row(
                transaction
                    .query(
                        "SELECT COUNT(*) FROM task_runs WHERE board_id = :board_id AND task_id = :task_id AND status = 'running'",
                        [
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                        ],
                    )
                    .await?,
            )
            .await?;
        if integer_value(active_run_count.get_value(0)?, "task_runs.active_count")? != 1 {
            return Err(StoreError::InvalidTransition(
                "heartbeat requires exactly one running run".to_owned(),
            ));
        }

        let run = first_row(
                transaction
                    .query(
                        "SELECT id, board_id, task_id, status, worker_profile, worker_pid, claim_token, claim_owner, claim_expires_at, started_at, last_heartbeat_at, finished_at, exit_code, summary, error, log_path, metadata_json FROM task_runs WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id LIMIT 1",
                        [
                            (":run_id", run_id.as_str()),
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                        ],
                    )
                    .await?,
            )
            .await
            .map_err(|error| match error {
                turso::Error::QueryReturnedNoRows => StoreError::InvalidTransition(
                    "heartbeat requires a matching running run".to_owned(),
                ),
                other => StoreError::Turso(other),
            })?;
        let run_status = text_value(run.get_value(3)?, "task_runs.status")?;
        if run_status != "running" {
            return Err(StoreError::InvalidTransition(
                "heartbeat requires a matching running run".to_owned(),
            ));
        }
        let run_claim_token = text_value(run.get_value(6)?, "task_runs.claim_token")?;
        let run_claim_owner = text_value(run.get_value(7)?, "task_runs.claim_owner")?;
        if task_claim_token.as_deref() != Some(run_claim_token.as_str())
            || task_claim_owner.as_deref() != Some(run_claim_owner.as_str())
        {
            return Err(StoreError::InvalidTransition(
                "active run claim is inconsistent".to_owned(),
            ));
        }

        let changed = transaction
                .execute(
                    "UPDATE tasks SET claim_expires_at = :claim_expires_at, last_heartbeat_at = :last_heartbeat_at, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = 'running' AND claim_token = :claim_token AND claim_owner = :claim_owner AND current_run_id = :run_id AND lock_version = :expected_lock_version",
                    (
                        (":claim_expires_at", input.claim_expires_at),
                        (":last_heartbeat_at", input.now),
                        (":updated_at", input.now),
                        (":task_id", task_id),
                        (":board_id", board_id.as_str()),
                        (":claim_token", claim_token),
                        (":claim_owner", actor.as_str()),
                        (":run_id", run_id.as_str()),
                        (":expected_lock_version", input.expected_lock_version),
                    ),
                )
                .await?;
        if changed != 1 {
            return Err(StoreError::ClaimConflict(
                "heartbeat compare-and-set failed".to_owned(),
            ));
        }

        let changed = transaction
                .execute(
                    "UPDATE task_runs SET claim_expires_at = :claim_expires_at, last_heartbeat_at = :last_heartbeat_at WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id AND status = 'running' AND claim_token = :claim_token AND claim_owner = :claim_owner",
                    (
                        (":claim_expires_at", input.claim_expires_at),
                        (":last_heartbeat_at", input.now),
                        (":run_id", run_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":claim_token", claim_token),
                        (":claim_owner", actor.as_str()),
                    ),
                )
                .await?;
        if changed != 1 {
            return Err(StoreError::InvalidTransition(
                "heartbeat requires a matching running run".to_owned(),
            ));
        }

        transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, :run_id, 'task.heartbeat', :actor, json_object('note', :note), :created_at)",
                    (
                        (":event_id", event_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":run_id", run_id.as_str()),
                        (":actor", actor.as_str()),
                        (":note", note),
                        (":created_at", input.now),
                    ),
                )
                .await?;

        let heartbeated = task_from_row(
            first_row(
                transaction
                    .query(
                        &format!(
                            "{TASK_SELECT} WHERE t.board_id = :board_id AND t.id = :task_id LIMIT 1"
                        ),
                        [(":board_id", board_id.as_str()), (":task_id", task_id)],
                    )
                    .await?,
            )
            .await?,
        )?;

        transaction.commit().await?;
        Ok(heartbeated)
    }
}

pub(crate) fn validate_heartbeat_task_input(
    task_id: &str,
    input: &HeartbeatTaskInput,
) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    if input.expected_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_lock_version must be non-negative".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    if input.claim_token.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "claim_token is required".to_owned(),
        ));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id must start with e_".to_owned(),
        ));
    }
    if input.now < 0 {
        return Err(StoreError::InvalidInput(
            "now must be non-negative".to_owned(),
        ));
    }
    if input.claim_expires_at <= input.now {
        return Err(StoreError::InvalidInput(
            "claim_expires_at must be after now".to_owned(),
        ));
    }
    Ok(())
}
