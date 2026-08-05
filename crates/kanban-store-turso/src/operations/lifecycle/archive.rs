use turso::transaction::TransactionBehavior;

use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveTaskInput {
    pub expected_lock_version: i64,
    pub actor: String,
    pub force: bool,
    pub event_id: String,
    pub now: i64,
}

impl TursoStore {
    pub async fn archive_task(
        &self,
        task_id: &str,
        input: ArchiveTaskInput,
    ) -> Result<TaskRecord, StoreError> {
        validate_archive_input(task_id, &input)?;
        let task_id = task_id.trim();
        let actor = input.actor.trim().to_owned();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let row = first_row(
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
        let board_id = text_value(row.get_value(0)?, "tasks.board_id")?;
        let status = text_value(row.get_value(1)?, "tasks.status")?;
        if optional_integer_value(row.get_value(3)?, "boards.archived_at")?.is_some() {
            return Err(StoreError::InvalidTransition(
                "已归档 board 不能归档任务".to_owned(),
            ));
        }
        if optional_integer_value(row.get_value(2)?, "tasks.archived_at")?.is_some()
            || status == "archived"
        {
            return Err(StoreError::InvalidTransition("任务已归档".to_owned()));
        }
        if integer_value(row.get_value(4)?, "tasks.lock_version")? != input.expected_lock_version {
            return Err(StoreError::ClaimConflict("lock_version 不匹配".to_owned()));
        }
        let claim_token = optional_text_value(row.get_value(5)?, "tasks.claim_token")?;
        let claim_owner = optional_text_value(row.get_value(6)?, "tasks.claim_owner")?;
        let claim_expires_at = optional_integer_value(row.get_value(7)?, "tasks.claim_expires_at")?;
        let run_id = optional_text_value(row.get_value(8)?, "tasks.current_run_id")?;

        if !input.force {
            let incomplete = first_row(
                transaction
                    .query(
                        "SELECT COUNT(*) FROM task_steps WHERE board_id = :board_id AND parent_task_id = :task_id AND required = 1 AND status NOT IN ('done', 'skipped')",
                        [(":board_id", board_id.as_str()), (":task_id", task_id)],
                    )
                    .await?,
            )
            .await?;
            if integer_value(incomplete.get_value(0)?, "task_steps.incomplete")? != 0 {
                return Err(StoreError::StepsIncomplete("仍有必需步骤未完成".to_owned()));
            }
        }

        let event_run_id = if status == "running" {
            if !input.force {
                return Err(StoreError::InvalidTransition(
                    "归档 running 任务必须设置 force".to_owned(),
                ));
            }
            let run_id = run_id
                .clone()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    StoreError::InvalidTransition("running 任务没有 current run".to_owned())
                })?;
            if claim_token.is_none() || claim_owner.is_none() || claim_expires_at.is_none() {
                return Err(StoreError::InvalidTransition(
                    "running 任务的 claim 信息不完整".to_owned(),
                ));
            }
            let active = first_row(
                transaction
                    .query(
                        "SELECT COUNT(*) FROM task_runs WHERE board_id = :board_id AND task_id = :task_id AND status = 'running'",
                        [(":board_id", board_id.as_str()), (":task_id", task_id)],
                    )
                    .await?,
            )
            .await?;
            if integer_value(active.get_value(0)?, "task_runs.active_count")? != 1 {
                return Err(StoreError::InvalidTransition(
                    "归档要求恰好有一个 running run".to_owned(),
                ));
            }
            let running = first_row(
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
                    "归档要求匹配的 running run".to_owned(),
                ),
                other => StoreError::Turso(other),
            })?;
            if text_value(running.get_value(0)?, "task_runs.status")? != "running"
                || text_value(running.get_value(1)?, "task_runs.claim_token")?
                    != claim_token.as_deref().unwrap_or_default()
                || text_value(running.get_value(2)?, "task_runs.claim_owner")?
                    != claim_owner.as_deref().unwrap_or_default()
                || integer_value(running.get_value(3)?, "task_runs.claim_expires_at")?
                    != claim_expires_at.unwrap_or_default()
            {
                return Err(StoreError::InvalidTransition(
                    "active run claim 不一致".to_owned(),
                ));
            }
            let changed = transaction
                .execute(
                    "UPDATE task_runs SET status = 'canceled', finished_at = :finished_at, error = '任务已归档' WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id AND status = 'running'",
                    (
                        (":finished_at", input.now),
                        (":run_id", run_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ),
                )
                .await?;
            if changed != 1 {
                return Err(StoreError::InvalidTransition(
                    "归档要求匹配的 running run".to_owned(),
                ));
            }
            Some(run_id)
        } else {
            let active = first_row(
                transaction
                    .query(
                        "SELECT COUNT(*) FROM task_runs WHERE board_id = :board_id AND task_id = :task_id AND status = 'running'",
                        [(":board_id", board_id.as_str()), (":task_id", task_id)],
                    )
                    .await?,
            )
            .await?;
            if integer_value(active.get_value(0)?, "task_runs.active_count")? != 0 {
                return Err(StoreError::InvalidTransition(
                    "存在 active run 时归档必须设置 force".to_owned(),
                ));
            }
            None
        };

        if transaction
            .execute(
                "UPDATE tasks SET status = 'archived', status_reason = NULL, archived_at = :archived_at, claim_token = NULL, claim_owner = NULL, claim_expires_at = NULL, last_heartbeat_at = NULL, current_run_id = NULL, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = :source_status AND lock_version = :expected_lock_version",
                (
                    (":archived_at", input.now),
                    (":updated_at", input.now),
                    (":task_id", task_id),
                    (":board_id", board_id.as_str()),
                    (":source_status", status.as_str()),
                    (":expected_lock_version", input.expected_lock_version),
                ),
            )
            .await?
            != 1
        {
            return Err(StoreError::ClaimConflict("归档 compare-and-set 失败".to_owned()));
        }
        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, :run_id, 'task.archived', :actor, '{}', :created_at)",
                (
                    (":event_id", input.event_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":run_id", event_run_id.as_deref()),
                    (":actor", actor.as_str()),
                    (":created_at", input.now),
                ),
            )
            .await?;
        let archived = task_from_row(
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
        Ok(archived)
    }
}

fn validate_archive_input(task_id: &str, input: &ArchiveTaskInput) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id 必须以 t_ 开头".to_owned(),
        ));
    }
    if input.expected_lock_version < 0 || input.now < 0 {
        return Err(StoreError::InvalidInput(
            "lock_version 和 now 不能为负数".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor 不能为空".to_owned()));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id 必须以 e_ 开头".to_owned(),
        ));
    }
    Ok(())
}
