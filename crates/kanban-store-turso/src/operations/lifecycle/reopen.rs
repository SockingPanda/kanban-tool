use turso::transaction::TransactionBehavior;

use crate::{
    db::TursoStore, domain::*, error::StoreError, operations::shared::canonical_ready_status,
    shared::*,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReopenTaskInput {
    pub expected_lock_version: i64,
    pub actor: String,
    pub reason: String,
    pub event_id: String,
    pub now: i64,
}

impl TursoStore {
    pub async fn reopen_task(
        &self,
        task_id: &str,
        input: ReopenTaskInput,
    ) -> Result<TaskRecord, StoreError> {
        validate_reopen_input(task_id, &input)?;
        let task_id = task_id.trim();
        let actor = input.actor.trim().to_owned();
        let reason = input.reason.trim().to_owned();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let row = first_row(
            transaction
                .query(
                    "SELECT t.board_id, t.status, t.archived_at, b.archived_at, t.lock_version, t.title, t.description, t.scheduled_at, t.completed_at FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
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
                "已归档 task 或 board 不能 reopen".to_owned(),
            ));
        }
        if text_value(row.get_value(1)?, "tasks.status")? != "done" {
            return Err(StoreError::InvalidTransition(
                "reopen 只能用于 done 任务".to_owned(),
            ));
        }
        if integer_value(row.get_value(4)?, "tasks.lock_version")? != input.expected_lock_version {
            return Err(StoreError::ClaimConflict("lock_version 不匹配".to_owned()));
        }
        let title = text_value(row.get_value(5)?, "tasks.title")?;
        let description = optional_text_value(row.get_value(6)?, "tasks.description")?;
        let scheduled_at = optional_integer_value(row.get_value(7)?, "tasks.scheduled_at")?;
        let original_completed_at =
            optional_integer_value(row.get_value(8)?, "tasks.completed_at")?;
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
        let mut target_status = canonical_ready_status(
            &title,
            description.as_deref(),
            scheduled_at,
            dependencies_done,
            input.now,
        );
        if target_status == "ready" {
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
                target_status = "todo";
            }
        }
        let changed = transaction
            .execute(
                "UPDATE tasks SET status = :status, status_reason = NULL, completed_at = NULL, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = 'done' AND lock_version = :expected_lock_version",
                (
                    (":status", target_status),
                    (":updated_at", input.now),
                    (":task_id", task_id),
                    (":board_id", board_id.as_str()),
                    (":expected_lock_version", input.expected_lock_version),
                ),
            )
            .await?;
        if changed != 1 {
            return Err(StoreError::ClaimConflict(
                "reopen compare-and-set 失败".to_owned(),
            ));
        }

        // 父任务从 done 离开后，直接依赖子任务重新计算其 active 状态。
        let mut children = transaction
            .query(
                "SELECT t.id, t.status, t.title, t.description, t.scheduled_at FROM task_dependencies AS d JOIN tasks AS t ON t.id = d.child_task_id AND t.board_id = d.board_id WHERE d.board_id = :board_id AND d.parent_task_id = :task_id AND t.archived_at IS NULL",
                [(":board_id", board_id.as_str()), (":task_id", task_id)],
            )
            .await?;
        while let Some(child) = children.next().await? {
            let child_id = text_value(child.get_value(0)?, "tasks.id")?;
            let child_status = text_value(child.get_value(1)?, "tasks.status")?;
            if !matches!(
                child_status.as_str(),
                "triage" | "todo" | "scheduled" | "ready"
            ) {
                continue;
            }
            let child_title = text_value(child.get_value(2)?, "tasks.title")?;
            let child_description = optional_text_value(child.get_value(3)?, "tasks.description")?;
            let child_scheduled =
                optional_integer_value(child.get_value(4)?, "tasks.scheduled_at")?;
            let child_target = canonical_ready_status(
                &child_title,
                child_description.as_deref(),
                child_scheduled,
                false,
                input.now,
            );
            if child_target != child_status {
                transaction
                    .execute(
                        "UPDATE tasks SET status = :status, status_reason = NULL, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = :current_status AND archived_at IS NULL",
                        (
                            (":status", child_target),
                            (":updated_at", input.now),
                            (":task_id", child_id.as_str()),
                            (":board_id", board_id.as_str()),
                            (":current_status", child_status.as_str()),
                        ),
                    )
                    .await?;
            }
        }
        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.reopened', :actor, json_object('from', 'done', 'to', :to_status, 'reason', :reason, 'original_completed_at', :original_completed_at), :created_at)",
                (
                    (":event_id", input.event_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":actor", actor.as_str()),
                    (":to_status", target_status),
                    (":reason", reason.as_str()),
                    (":original_completed_at", original_completed_at),
                    (":created_at", input.now),
                ),
            )
            .await?;
        let reopened = task_from_row(
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
        Ok(reopened)
    }
}

fn validate_reopen_input(task_id: &str, input: &ReopenTaskInput) -> Result<(), StoreError> {
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
