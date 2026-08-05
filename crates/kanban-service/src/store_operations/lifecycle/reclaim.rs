use turso::transaction::TransactionBehavior;

use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReclaimTaskInput {
    pub expected_lock_version: i64,
    pub actor: String,
    pub force: bool,
    pub target_status: String,
    pub retry_count: i64,
    pub reason: String,
    pub event_id: String,
    pub now: i64,
}

impl TursoStore {
    /// 显式回收 running claim；过期回收与 force 回收在同一事务中完成。
    pub async fn reclaim_task(
        &self,
        task_id: &str,
        input: ReclaimTaskInput,
    ) -> Result<TaskRecord, StoreError> {
        validate_reclaim_input(task_id, &input)?;
        let task_id = task_id.trim();
        let actor = input.actor.trim().to_owned();
        let target_status = input.target_status.trim().to_owned();
        let reason = input.reason.trim().to_owned();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let row = first_row(
            transaction
                .query(
                    "SELECT t.board_id, t.status, t.archived_at, b.archived_at, t.lock_version, t.claim_token, t.claim_owner, t.claim_expires_at, t.current_run_id, t.retry_count, t.max_retries, t.title, t.description, t.scheduled_at FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
                    [(":task_id", task_id)],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(task_id.to_owned()),
            other => StoreError::Turso(other),
        })?;
        let board_id = text_value(row.get_value(0)?, "tasks.board_id")?;
        let status = text_value(row.get_value(1)?, "tasks.status")?;
        if status != "running" {
            return Err(StoreError::InvalidTransition(
                "reclaim 只能用于 running 任务".to_owned(),
            ));
        }
        if optional_integer_value(row.get_value(2)?, "tasks.archived_at")?.is_some()
            || optional_integer_value(row.get_value(3)?, "boards.archived_at")?.is_some()
        {
            return Err(StoreError::InvalidTransition(
                "已归档 task 或 board 不能 reclaim".to_owned(),
            ));
        }
        if integer_value(row.get_value(4)?, "tasks.lock_version")? != input.expected_lock_version {
            return Err(StoreError::ClaimConflict("lock_version 不匹配".to_owned()));
        }
        let claim_token = optional_text_value(row.get_value(5)?, "tasks.claim_token")?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| StoreError::InvalidTransition("reclaim 需要 claim token".to_owned()))?;
        let claim_owner = optional_text_value(row.get_value(6)?, "tasks.claim_owner")?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| StoreError::InvalidTransition("reclaim 需要 claim owner".to_owned()))?;
        let claim_expires_at = optional_integer_value(row.get_value(7)?, "tasks.claim_expires_at")?
            .ok_or_else(|| StoreError::InvalidTransition("reclaim 需要 claim expiry".to_owned()))?;
        if !input.force && claim_expires_at > input.now {
            return Err(StoreError::InvalidTransition(
                "claim 未过期时必须设置 force 才能 reclaim".to_owned(),
            ));
        }
        let run_id = optional_text_value(row.get_value(8)?, "tasks.current_run_id")?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                StoreError::InvalidTransition("reclaim requires a current run".to_owned())
            })?;
        let current_retry_count = integer_value(row.get_value(9)?, "tasks.retry_count")?;
        let max_retries = optional_integer_value(row.get_value(10)?, "tasks.max_retries")?;
        let next_retry_count = current_retry_count
            .checked_add(1)
            .ok_or_else(|| StoreError::InvalidTransition("retry_count 溢出".to_owned()))?;
        if input.retry_count != next_retry_count {
            return Err(StoreError::InvalidTransition(
                "retry_count 与 canonical task state 不匹配".to_owned(),
            ));
        }
        if max_retries.is_some_and(|max| next_retry_count >= max) && target_status != "blocked" {
            return Err(StoreError::InvalidTransition(
                "已达到 max retries；target status 必须是 blocked".to_owned(),
            ));
        }
        if !matches!(
            target_status.as_str(),
            "triage" | "todo" | "scheduled" | "ready" | "blocked"
        ) {
            return Err(StoreError::InvalidInput(
                "target_status 必须是 ready、blocked 或重算得到的 active status".to_owned(),
            ));
        }

        let run = first_row(
            transaction
                .query(
                    "SELECT status, claim_token, claim_owner, claim_expires_at FROM task_runs WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id LIMIT 1",
                    [(":run_id", run_id.as_str()), (":board_id", board_id.as_str()), (":task_id", task_id)],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::InvalidTransition("reclaim 需要匹配的 running run".to_owned()),
            other => StoreError::Turso(other),
        })?;
        if text_value(run.get_value(0)?, "task_runs.status")? != "running"
            || text_value(run.get_value(1)?, "task_runs.claim_token")? != claim_token
            || text_value(run.get_value(2)?, "task_runs.claim_owner")? != claim_owner
            || integer_value(run.get_value(3)?, "task_runs.claim_expires_at")? != claim_expires_at
        {
            return Err(StoreError::InvalidTransition(
                "active run claim 不一致".to_owned(),
            ));
        }
        let run_status = if input.force && claim_expires_at > input.now {
            "canceled"
        } else {
            "expired"
        };
        if transaction
            .execute(
                "UPDATE task_runs SET status = :status, finished_at = :finished_at, error = :error WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id AND status = 'running' AND claim_token = :claim_token AND claim_owner = :claim_owner AND claim_expires_at = :claim_expires_at",
                (
                    (":status", run_status),
                    (":finished_at", input.now),
                    (":error", reason.as_str()),
                    (":run_id", run_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":claim_token", claim_token.as_str()),
                    (":claim_owner", claim_owner.as_str()),
                    (":claim_expires_at", claim_expires_at),
                ),
            )
            .await?
            != 1
        {
            return Err(StoreError::InvalidTransition("reclaim 需要匹配的 running run".to_owned()));
        }
        if transaction
            .execute(
                "UPDATE tasks SET status = :status, status_reason = CASE WHEN :status = 'blocked' THEN :reason ELSE NULL END, claim_token = NULL, claim_owner = NULL, claim_expires_at = NULL, last_heartbeat_at = NULL, current_run_id = NULL, retry_count = :retry_count, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = 'running' AND current_run_id = :run_id AND claim_token = :claim_token AND claim_owner = :claim_owner AND claim_expires_at = :claim_expires_at AND lock_version = :expected_lock_version",
                (
                    (":status", target_status.as_str()),
                    (":reason", reason.as_str()),
                    (":retry_count", input.retry_count),
                    (":updated_at", input.now),
                    (":task_id", task_id),
                    (":board_id", board_id.as_str()),
                    (":run_id", run_id.as_str()),
                    (":claim_token", claim_token.as_str()),
                    (":claim_owner", claim_owner.as_str()),
                    (":claim_expires_at", claim_expires_at),
                    (":expected_lock_version", input.expected_lock_version),
                ),
            )
            .await?
            != 1
        {
            return Err(StoreError::ClaimConflict("reclaim compare-and-set 失败".to_owned()));
        }
        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, :run_id, 'task.reclaimed', :actor, json_object('retry_count', :retry_count, 'max_retries', :max_retries, 'to_status', :to_status, 'reason', :reason), :created_at)",
                (
                    (":event_id", input.event_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":run_id", run_id.as_str()),
                    (":actor", actor.as_str()),
                    (":retry_count", input.retry_count),
                    (":max_retries", max_retries),
                    (":to_status", target_status.as_str()),
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
        Ok(reclaimed)
    }
}

fn validate_reclaim_input(task_id: &str, input: &ReclaimTaskInput) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id 必须以 t_ 开头".to_owned(),
        ));
    }
    if input.expected_lock_version < 0 || input.retry_count < 0 || input.now < 0 {
        return Err(StoreError::InvalidInput(
            "lock_version、retry_count 和 now 不能为负数".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() || input.reason.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "actor 和 reason 不能为空".to_owned(),
        ));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id 必须以 e_ 开头".to_owned(),
        ));
    }
    Ok(())
}
