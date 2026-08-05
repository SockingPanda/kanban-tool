use turso::transaction::TransactionBehavior;

use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReclaimExpiredTaskInput {
    pub expected_lock_version: i64,
    pub actor: String,
    pub event_id: String,
    pub target_status: String,
    pub retry_count: i64,
    pub reason: String,
    pub now: i64,
}
impl TursoStore {
    pub async fn reclaim_expired_task(
        &self,
        task_id: &str,
        input: ReclaimExpiredTaskInput,
    ) -> Result<Option<TaskRecord>, StoreError> {
        validate_reclaim_expired_task_input(task_id, &input)?;
        let actor = input.actor.trim().to_owned();
        let event_id = input.event_id.trim().to_owned();
        let target_status = input.target_status.trim().to_owned();
        let reason = input.reason.trim().to_owned();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let task = first_row(
                transaction
                    .query(
                        "SELECT t.board_id, t.status, t.archived_at, b.archived_at, t.lock_version, t.claim_token, t.claim_owner, t.claim_expires_at, t.last_heartbeat_at, t.current_run_id, t.retry_count, t.max_retries, t.title, t.description, t.scheduled_at FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
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
        if status == "archived" || task_archived_at.is_some() || board_archived_at.is_some() {
            return Ok(None);
        }
        if status != "running" {
            return Ok(None);
        }

        let lock_version = integer_value(task.get_value(4)?, "tasks.lock_version")?;
        if lock_version != input.expected_lock_version {
            return Ok(None);
        }
        let claim_token = optional_text_value(task.get_value(5)?, "tasks.claim_token")?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                StoreError::InvalidTransition(
                    "reclaim requires a matching task claim token".to_owned(),
                )
            })?;
        let claim_owner = optional_text_value(task.get_value(6)?, "tasks.claim_owner")?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                StoreError::InvalidTransition(
                    "reclaim requires a matching task claim owner".to_owned(),
                )
            })?;
        let claim_expires_at = optional_integer_value(
            task.get_value(7)?,
            "tasks.claim_expires_at",
        )?
        .ok_or_else(|| {
            StoreError::InvalidTransition("reclaim requires an expiring task claim".to_owned())
        })?;
        if claim_expires_at > input.now {
            return Ok(None);
        }
        let run_id = optional_text_value(task.get_value(9)?, "tasks.current_run_id")?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                StoreError::InvalidTransition("reclaim requires a current running run".to_owned())
            })?;
        let retry_count = integer_value(task.get_value(10)?, "tasks.retry_count")?;
        let max_retries = optional_integer_value(task.get_value(11)?, "tasks.max_retries")?;
        let title = text_value(task.get_value(12)?, "tasks.title")?;
        let description = optional_text_value(task.get_value(13)?, "tasks.description")?;
        let scheduled_at = optional_integer_value(task.get_value(14)?, "tasks.scheduled_at")?;

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
                "reclaim requires exactly one running run".to_owned(),
            ));
        }

        let run = first_row(
                transaction
                    .query(
                        "SELECT status, claim_token, claim_owner, claim_expires_at FROM task_runs WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id LIMIT 1",
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
                    "reclaim requires a matching running run".to_owned(),
                ),
                other => StoreError::Turso(other),
            })?;
        if text_value(run.get_value(0)?, "task_runs.status")? != "running" {
            return Err(StoreError::InvalidTransition(
                "reclaim requires a matching running run".to_owned(),
            ));
        }
        let run_claim_token = text_value(run.get_value(1)?, "task_runs.claim_token")?;
        let run_claim_owner = text_value(run.get_value(2)?, "task_runs.claim_owner")?;
        let run_claim_expires_at = integer_value(run.get_value(3)?, "task_runs.claim_expires_at")?;
        if run_claim_token != claim_token
            || run_claim_owner != claim_owner
            || run_claim_expires_at != claim_expires_at
        {
            return Err(StoreError::InvalidTransition(
                "active run claim is inconsistent".to_owned(),
            ));
        }
        if run_claim_expires_at > input.now {
            return Err(StoreError::InvalidTransition(
                "active run claim is not expired".to_owned(),
            ));
        }

        let dependency_blocked = first_row(
                transaction
                    .query(
                        "SELECT EXISTS (SELECT 1 FROM task_dependencies AS d JOIN tasks AS p ON p.id = d.parent_task_id AND p.board_id = d.board_id WHERE d.board_id = :board_id AND d.child_task_id = :task_id AND p.status NOT IN ('done', 'archived'))",
                        [
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                        ],
                    )
                    .await?,
            )
            .await?;
        let dependency_blocked = integer_value(
            dependency_blocked.get_value(0)?,
            "task_dependencies.unfinished_parent",
        )? != 0;
        let execution_plan_ready = first_row(
                transaction
                    .query(
                        "SELECT EXISTS (SELECT 1 FROM task_steps AS s WHERE s.board_id = :board_id AND s.parent_task_id = :task_id) OR EXISTS (SELECT 1 FROM task_execution_plans AS ep WHERE ep.board_id = :board_id AND ep.task_id = :task_id AND ep.state = 'not_required')",
                        [
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                        ],
                    )
                    .await?,
            )
            .await?;
        let execution_plan_ready = integer_value(
            execution_plan_ready.get_value(0)?,
            "task_execution_plans.ready",
        )? != 0;

        let next_retry_count = retry_count
            .checked_add(1)
            .ok_or_else(|| StoreError::InvalidTransition("retry_count overflow".to_owned()))?;
        if input.retry_count != next_retry_count {
            return Err(StoreError::InvalidTransition(
                "retry_count does not match canonical task state".to_owned(),
            ));
        }
        let canonical_status = if max_retries.is_some_and(|max| next_retry_count >= max) {
            "blocked"
        } else if title.trim().is_empty()
            || description
                .as_deref()
                .is_none_or(|description| description.trim().is_empty())
        {
            "triage"
        } else if scheduled_at.is_some_and(|scheduled_at| scheduled_at > input.now) {
            "scheduled"
        } else if dependency_blocked || !execution_plan_ready {
            "todo"
        } else {
            "ready"
        };
        if target_status != canonical_status {
            return Err(StoreError::InvalidTransition(
                "target_status does not match canonical task state".to_owned(),
            ));
        }

        let changed = transaction
                .execute(
                    "UPDATE task_runs SET status = 'expired', finished_at = :finished_at, error = 'claim expired' WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id AND status = 'running' AND claim_token = :claim_token AND claim_owner = :claim_owner AND claim_expires_at = :claim_expires_at AND claim_expires_at <= :now",
                    (
                        (":finished_at", input.now),
                        (":run_id", run_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":claim_token", claim_token.as_str()),
                        (":claim_owner", claim_owner.as_str()),
                        (":claim_expires_at", claim_expires_at),
                        (":now", input.now),
                    ),
                )
                .await?;
        if changed != 1 {
            return Err(StoreError::InvalidTransition(
                "reclaim requires a matching expired running run".to_owned(),
            ));
        }

        let status_reason = (canonical_status == "blocked").then_some(reason.as_str());
        let changed = transaction
                .execute(
                    "UPDATE tasks SET status = :status, status_reason = :status_reason, claim_token = NULL, claim_owner = NULL, claim_expires_at = NULL, last_heartbeat_at = NULL, current_run_id = NULL, retry_count = :retry_count, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = 'running' AND claim_token = :claim_token AND claim_owner = :claim_owner AND claim_expires_at = :claim_expires_at AND current_run_id = :run_id AND lock_version = :expected_lock_version",
                    (
                        (":status", canonical_status),
                        (":status_reason", status_reason),
                        (":retry_count", input.retry_count),
                        (":updated_at", input.now),
                        (":task_id", task_id),
                        (":board_id", board_id.as_str()),
                        (":claim_token", claim_token.as_str()),
                        (":claim_owner", claim_owner.as_str()),
                        (":claim_expires_at", claim_expires_at),
                        (":run_id", run_id.as_str()),
                        (":expected_lock_version", input.expected_lock_version),
                    ),
                )
                .await?;
        if changed != 1 {
            return Err(StoreError::ClaimConflict(
                "reclaim compare-and-set failed".to_owned(),
            ));
        }

        transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, :run_id, 'task.reclaimed', :actor, json_object('retry_count', :retry_count, 'max_retries', :max_retries, 'to_status', :to_status, 'reason', :reason), :created_at)",
                    (
                        (":event_id", event_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":run_id", run_id.as_str()),
                        (":actor", actor.as_str()),
                        (":retry_count", input.retry_count),
                        (":max_retries", max_retries),
                        (":to_status", canonical_status),
                        (":reason", reason.as_str()),
                        (":created_at", input.now),
                    ),
                )
                .await?;

        let reclaimed = task_from_row(
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
        Ok(Some(reclaimed))
    }
}

pub(crate) fn validate_reclaim_expired_task_input(
    task_id: &str,
    input: &ReclaimExpiredTaskInput,
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
    if !matches!(
        input.target_status.trim(),
        "triage" | "todo" | "scheduled" | "ready" | "blocked"
    ) {
        return Err(StoreError::InvalidInput(
            "target_status must be triage, todo, scheduled, ready, or blocked".to_owned(),
        ));
    }
    if input.retry_count < 0 {
        return Err(StoreError::InvalidInput(
            "retry_count must be non-negative".to_owned(),
        ));
    }
    if input.reason.trim().is_empty() {
        return Err(StoreError::InvalidInput("reason is required".to_owned()));
    }
    if input.now < 0 {
        return Err(StoreError::InvalidInput(
            "now must be non-negative".to_owned(),
        ));
    }
    Ok(())
}
