use turso::transaction::TransactionBehavior;

use crate::{
    db::TursoStore, domain::*, error::StoreError, shared::*,
    store_operations::shared::canonical_ready_status,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnblockTaskInput {
    pub expected_lock_version: i64,
    pub actor: String,
    pub event_id: String,
    pub now: i64,
}

impl TursoStore {
    pub async fn unblock_task(
        &self,
        task_id: &str,
        input: UnblockTaskInput,
    ) -> Result<TaskRecord, StoreError> {
        validate_unblock_input(task_id, &input)?;
        let task_id = task_id.trim();
        let actor = input.actor.trim().to_owned();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let row = first_row(
            transaction
                .query(
                    "SELECT t.board_id, t.status, t.archived_at, b.archived_at, t.lock_version, t.title, t.description, t.scheduled_at FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
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
        if optional_integer_value(row.get_value(2)?, "tasks.archived_at")?.is_some()
            || optional_integer_value(row.get_value(3)?, "boards.archived_at")?.is_some()
        {
            return Err(StoreError::InvalidTransition(
                "已归档 task 或 board 不能 unblock".to_owned(),
            ));
        }
        if text_value(row.get_value(1)?, "tasks.status")? != "blocked" {
            return Err(StoreError::InvalidTransition(
                "unblock 只能用于 blocked 任务".to_owned(),
            ));
        }
        if integer_value(row.get_value(4)?, "tasks.lock_version")? != input.expected_lock_version {
            return Err(StoreError::ClaimConflict("lock_version 不匹配".to_owned()));
        }
        let title = text_value(row.get_value(5)?, "tasks.title")?;
        let description = optional_text_value(row.get_value(6)?, "tasks.description")?;
        let scheduled_at = optional_integer_value(row.get_value(7)?, "tasks.scheduled_at")?;
        let dep = first_row(
            transaction
                .query(
                    "SELECT EXISTS (SELECT 1 FROM task_dependencies AS d JOIN tasks AS p ON p.id = d.parent_task_id AND p.board_id = d.board_id WHERE d.board_id = :board_id AND d.child_task_id = :task_id AND p.status NOT IN ('done', 'archived'))",
                    [(":board_id", board_id.as_str()), (":task_id", task_id)],
                )
                .await?,
        )
        .await?;
        let dependencies_done = integer_value(dep.get_value(0)?, "task_dependencies.blocked")? == 0;
        let mut status = canonical_ready_status(
            &title,
            description.as_deref(),
            scheduled_at,
            dependencies_done,
            input.now,
        );
        if status == "ready" {
            let plan = first_row(
                transaction
                    .query(
                        "SELECT EXISTS (SELECT 1 FROM task_steps WHERE board_id = :board_id AND parent_task_id = :task_id) OR EXISTS (SELECT 1 FROM task_execution_plans WHERE board_id = :board_id AND task_id = :task_id AND state = 'not_required')",
                        [(":board_id", board_id.as_str()), (":task_id", task_id)],
                    )
                    .await?,
            )
            .await?;
            if integer_value(plan.get_value(0)?, "task_execution_plans.ready")? == 0 {
                status = "todo";
            }
        }
        if transaction
            .execute(
                "UPDATE tasks SET status = :status, status_reason = NULL, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = 'blocked' AND lock_version = :expected_lock_version",
                (
                    (":status", status),
                    (":updated_at", input.now),
                    (":task_id", task_id),
                    (":board_id", board_id.as_str()),
                    (":expected_lock_version", input.expected_lock_version),
                ),
            )
            .await?
            != 1
        {
            return Err(StoreError::ClaimConflict("unblock compare-and-set 失败".to_owned()));
        }
        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.unblocked', :actor, json_object('to_status', :to_status), :created_at)",
                (
                    (":event_id", input.event_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":actor", actor.as_str()),
                    (":to_status", status),
                    (":created_at", input.now),
                ),
            )
            .await?;
        let task = task_from_row(
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
        Ok(task)
    }
}

fn validate_unblock_input(task_id: &str, input: &UnblockTaskInput) -> Result<(), StoreError> {
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
