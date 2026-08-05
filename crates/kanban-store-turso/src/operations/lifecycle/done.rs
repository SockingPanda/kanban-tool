use turso::transaction::TransactionBehavior;

use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteTaskInput {
    pub expected_lock_version: i64,
    pub actor: String,
    pub claim_token: Option<String>,
    pub force: bool,
    pub summary: Option<String>,
    pub result_json: Option<String>,
    pub now: i64,
    pub event_id: String,
}
impl TursoStore {
    pub async fn complete_task(
        &self,
        task_id: &str,
        input: CompleteTaskInput,
    ) -> Result<TaskRecord, StoreError> {
        validate_complete_task_input(task_id, &input)?;
        let actor = input.actor.trim().to_owned();
        let input_claim_token = input.claim_token.as_deref();
        let event_id = input.event_id.trim().to_owned();
        let summary = input.summary.as_deref();
        let result_json = input.result_json.as_deref();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        if let Some(result_json) = result_json {
            let valid = first_row(
                transaction
                    .query(
                        "SELECT json_valid(:result_json)",
                        [(":result_json", result_json)],
                    )
                    .await?,
            )
            .await?;
            if integer_value(valid.get_value(0)?, "tasks.result_json_valid")? == 0 {
                return Err(StoreError::InvalidInput(
                    "result_json must be valid JSON".to_owned(),
                ));
            }
        }

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
                "archived task or board cannot complete".to_owned(),
            ));
        }
        if status != "running" && status != "review" {
            return Err(StoreError::InvalidTransition(
                "complete requires running or review".to_owned(),
            ));
        }

        let lock_version = integer_value(task.get_value(4)?, "tasks.lock_version")?;
        if lock_version != input.expected_lock_version {
            return Err(StoreError::ClaimConflict(
                "lock_version mismatch".to_owned(),
            ));
        }

        let task_claim_token = optional_text_value(task.get_value(5)?, "tasks.claim_token")?;
        let task_claim_owner = optional_text_value(task.get_value(6)?, "tasks.claim_owner")?;
        let task_claim_expires_at =
            optional_integer_value(task.get_value(7)?, "tasks.claim_expires_at")?;
        let run_id = optional_text_value(task.get_value(8)?, "tasks.current_run_id")?;
        let mut run_claim_token = None;
        let mut run_claim_owner = None;
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
        let active_run_count =
            integer_value(active_run_count.get_value(0)?, "task_runs.active_count")?;

        if status == "running" {
            let run_id = run_id
                .clone()
                .filter(|run_id| !run_id.trim().is_empty())
                .ok_or_else(|| {
                    StoreError::InvalidTransition(
                        "complete requires a current running run".to_owned(),
                    )
                })?;
            if active_run_count != 1 {
                return Err(StoreError::InvalidTransition(
                    "complete requires exactly one running run".to_owned(),
                ));
            }
            let run = first_row(
                    transaction
                        .query(
                            "SELECT status, claim_token, claim_owner FROM task_runs WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id LIMIT 1",
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
                        "complete requires a matching running run".to_owned(),
                    ),
                    other => StoreError::Turso(other),
                })?;
            if text_value(run.get_value(0)?, "task_runs.status")? != "running" {
                return Err(StoreError::InvalidTransition(
                    "complete requires a matching running run".to_owned(),
                ));
            }
            let canonical_run_token = text_value(run.get_value(1)?, "task_runs.claim_token")?;
            let canonical_run_owner = text_value(run.get_value(2)?, "task_runs.claim_owner")?;
            if task_claim_token.as_deref() != Some(canonical_run_token.as_str())
                || task_claim_owner.as_deref() != Some(canonical_run_owner.as_str())
            {
                return Err(StoreError::InvalidTransition(
                    "active run claim is inconsistent".to_owned(),
                ));
            }
            if task_claim_expires_at.is_none() {
                return Err(StoreError::InvalidTransition(
                    "complete requires an active claim".to_owned(),
                ));
            }
            if !input.force {
                if input_claim_token != task_claim_token.as_deref() {
                    return Err(StoreError::ClaimTokenMismatch);
                }
                if task_claim_owner.as_deref() != Some(actor.as_str()) {
                    return Err(StoreError::InvalidTransition(
                        "claim owner mismatch".to_owned(),
                    ));
                }
            }
            run_claim_token = Some(canonical_run_token);
            run_claim_owner = Some(canonical_run_owner);
        } else {
            if active_run_count != 0 {
                return Err(StoreError::InvalidTransition(
                    "review task cannot have an active running run".to_owned(),
                ));
            }
            if let Some(run_id) = run_id.as_deref().filter(|run_id| !run_id.trim().is_empty()) {
                let run = first_row(
                        transaction
                            .query(
                                "SELECT status FROM task_runs WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id LIMIT 1",
                                [
                                    (":run_id", run_id),
                                    (":board_id", board_id.as_str()),
                                    (":task_id", task_id),
                                ],
                            )
                            .await?,
                    )
                    .await
                    .map_err(|error| match error {
                        turso::Error::QueryReturnedNoRows => StoreError::InvalidTransition(
                            "complete requires a succeeded current run".to_owned(),
                        ),
                        other => StoreError::Turso(other),
                    })?;
                if text_value(run.get_value(0)?, "task_runs.status")? != "succeeded" {
                    return Err(StoreError::InvalidTransition(
                        "complete requires a succeeded current run".to_owned(),
                    ));
                }
            }
        }

        let incomplete_steps = first_row(
                transaction
                    .query(
                        "SELECT COUNT(*) FROM task_steps WHERE board_id = :board_id AND parent_task_id = :task_id AND required = 1 AND status NOT IN ('done', 'skipped')",
                        [
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                        ],
                    )
                    .await?,
            )
            .await?;
        let incomplete_steps = integer_value(
            incomplete_steps.get_value(0)?,
            "task_steps.incomplete_required_count",
        )?;
        if incomplete_steps != 0 {
            return Err(StoreError::StepsIncomplete(format!(
                "{incomplete_steps} required step(s) incomplete"
            )));
        }

        if let Some(run_id) = run_id.as_deref().filter(|run_id| !run_id.trim().is_empty())
            && let (Some(run_claim_token), Some(run_claim_owner)) =
                (run_claim_token.as_deref(), run_claim_owner.as_deref())
        {
            let changed = transaction
                    .execute(
                        "UPDATE task_runs SET status = 'succeeded', finished_at = :finished_at, exit_code = 0, error = NULL, summary = COALESCE(:summary, summary) WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id AND status = 'running' AND claim_token = :claim_token AND claim_owner = :claim_owner",
                        (
                            (":finished_at", input.now),
                            (":summary", summary),
                            (":run_id", run_id),
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                            (":claim_token", run_claim_token),
                            (":claim_owner", run_claim_owner),
                        ),
                    )
                    .await?;
            if changed != 1 {
                return Err(StoreError::InvalidTransition(
                    "complete requires a matching running run".to_owned(),
                ));
            }
        }

        let changed = transaction
                .execute(
                    "UPDATE tasks SET status = 'done', status_reason = NULL, completed_at = :completed_at, claim_token = NULL, claim_owner = NULL, claim_expires_at = NULL, last_heartbeat_at = NULL, result_summary = COALESCE(:summary, result_summary), result_json = COALESCE(:result_json, result_json), updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = :source_status AND lock_version = :expected_lock_version",
                    (
                        (":completed_at", input.now),
                        (":summary", summary),
                        (":result_json", result_json),
                        (":updated_at", input.now),
                        (":task_id", task_id),
                        (":board_id", board_id.as_str()),
                        (":source_status", status.as_str()),
                        (":expected_lock_version", input.expected_lock_version),
                    ),
                )
                .await?;
        if changed != 1 {
            return Err(StoreError::ClaimConflict(
                "complete compare-and-set failed".to_owned(),
            ));
        }

        transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, :run_id, 'task.completed', :actor, json_object('result', json(:result_json)), :created_at)",
                    (
                        (":event_id", event_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":run_id", run_id.as_deref()),
                        (":actor", actor.as_str()),
                        (":result_json", result_json),
                        (":created_at", input.now),
                    ),
                )
                .await?;

        let completed = task_from_row(
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
        Ok(completed)
    }
}

pub(crate) fn validate_complete_task_input(
    task_id: &str,
    input: &CompleteTaskInput,
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
    Ok(())
}
