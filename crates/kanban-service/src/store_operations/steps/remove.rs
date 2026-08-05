use turso::transaction::TransactionBehavior;

use crate::{
    db::TursoStore, domain::*, error::StoreError, shared::*,
    store_operations::shared::canonical_ready_status,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveStepInput {
    pub actor: String,
    pub event_id: String,
    pub recompute_event_id: String,
    pub updated_at: i64,
    pub expected_lock_version: i64,
}

impl TursoStore {
    /// 删除 step，并在同一事务中维护 execution plan、父任务状态和事件。
    pub async fn remove_step(
        &self,
        task_id: &str,
        step_id: &str,
        input: RemoveStepInput,
    ) -> Result<TaskStepRecord, StoreError> {
        validate_remove_step_input(task_id, step_id, &input)?;
        let task_id = task_id.trim();
        let step_id = step_id.trim();
        let actor = input.actor.trim();

        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let parent = first_row(
            transaction
                .query(
                    &format!("{TASK_SELECT} WHERE t.id = :task_id LIMIT 1"),
                    [(":task_id", task_id)],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(task_id.to_owned()),
            other => StoreError::Turso(other),
        })
        .and_then(task_from_row)?;

        let board = first_row(
            transaction
                .query(
                    "SELECT archived_at FROM boards WHERE id = :board_id LIMIT 1",
                    [(":board_id", parent.board_id.as_str())],
                )
                .await?,
        )
        .await?;
        if parent.archived_at.is_some() || parent.status == "archived" {
            return Err(StoreError::InvalidTransition(
                "已归档的父任务不能修改 step".to_owned(),
            ));
        }
        if optional_integer_value(board.get_value(0)?, "boards.archived_at")?.is_some() {
            return Err(StoreError::InvalidTransition(
                "已归档的 board 不能修改 step".to_owned(),
            ));
        }
        if parent.lock_version != input.expected_lock_version {
            return Err(StoreError::InvalidTransition(
                "删除 step 需要匹配的最新父任务".to_owned(),
            ));
        }

        let existing = step_from_row(
            &transaction,
            first_row(
                transaction
                    .query(
                        "SELECT id, board_id, parent_task_id, position, title, body, linked_task_id, required, status, resolution_note, resolved_by, resolved_at, created_by, created_at, updated_by, updated_at FROM task_steps WHERE board_id = :board_id AND parent_task_id = :parent_task_id AND id = :step_id LIMIT 1",
                        [
                            (":board_id", parent.board_id.as_str()),
                            (":parent_task_id", parent.id.as_str()),
                            (":step_id", step_id),
                        ],
                    )
                    .await?,
            )
            .await
            .map_err(|error| match error {
                turso::Error::QueryReturnedNoRows => StoreError::StepNotFound(step_id.to_owned()),
                other => StoreError::Turso(other),
            })?,
        )
        .await?;

        transaction
            .execute(
                "DELETE FROM task_steps WHERE board_id = :board_id AND parent_task_id = :parent_task_id AND id = :step_id",
                [
                    (":board_id", parent.board_id.as_str()),
                    (":parent_task_id", parent.id.as_str()),
                    (":step_id", step_id),
                ],
            )
            .await?;

        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.step.removed', :actor, json_object('step_id', :step_id, 'linked_task_id', :linked_task_id, 'position', :position, 'required', json(CASE WHEN :required = 1 THEN 'true' ELSE 'false' END), 'status', :status), :created_at)",
                (
                    (":event_id", input.event_id.trim()),
                    (":board_id", parent.board_id.as_str()),
                    (":task_id", parent.id.as_str()),
                    (":actor", actor),
                    (":step_id", existing.id.as_str()),
                    (":linked_task_id",
                        existing.linked_task.as_ref().map(|task| task.id.as_str()),
                    ),
                    (":position", existing.position),
                    (":required", if existing.required { 1_i64 } else { 0_i64 }),
                    (":status", existing.status.as_str()),
                    (":created_at", input.updated_at),
                ),
            )
            .await?;

        let remaining = first_row(
            transaction
                .query(
                    "SELECT COUNT(*) FROM task_steps WHERE board_id = :board_id AND parent_task_id = :task_id",
                    [
                        (":board_id", parent.board_id.as_str()),
                        (":task_id", parent.id.as_str()),
                    ],
                )
                .await?,
        )
        .await?;
        let remaining = integer_value(remaining.get_value(0)?, "task_steps.count")?;

        if remaining == 0 {
            transaction
                .execute(
                    "INSERT INTO task_execution_plans(board_id, task_id, state, reason, updated_by, updated_at) VALUES (:board_id, :task_id, 'unplanned', NULL, :updated_by, :updated_at) ON CONFLICT(task_id) DO UPDATE SET board_id = excluded.board_id, state = 'unplanned', reason = NULL, updated_by = excluded.updated_by, updated_at = excluded.updated_at",
                    (
                        (":board_id", parent.board_id.as_str()),
                        (":task_id", parent.id.as_str()),
                        (":updated_by", actor),
                        (":updated_at", input.updated_at),
                    ),
                )
                .await?;
        }

        let current_status = parent.status.as_str();
        let target_status = if remaining == 0
            && matches!(current_status, "triage" | "todo" | "scheduled" | "ready")
        {
            let dependencies_done = first_row(
                transaction
                    .query(
                        "SELECT NOT EXISTS (SELECT 1 FROM task_dependencies AS d JOIN tasks AS dependency ON dependency.id = d.parent_task_id AND dependency.board_id = d.board_id WHERE d.board_id = :board_id AND d.child_task_id = :task_id AND dependency.status NOT IN ('done', 'archived'))",
                        [
                            (":board_id", parent.board_id.as_str()),
                            (":task_id", parent.id.as_str()),
                        ],
                    )
                    .await?,
            )
            .await?;
            let dependencies_done =
                integer_value(dependencies_done.get_value(0)?, "task_dependencies.ready")? != 0;
            canonical_ready_status(
                &parent.title,
                parent.description.as_deref(),
                parent.scheduled_at,
                dependencies_done,
                input.updated_at,
            )
        } else {
            current_status
        };

        // 即使重算后状态不变，删除 step 仍是父任务 mutation，因此在同一 CAS 中推进锁版本。
        let changed = if target_status != current_status {
            transaction
                .execute(
                    "UPDATE tasks SET status = :target_status, status_reason = NULL, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = :current_status AND archived_at IS NULL AND lock_version = :lock_version",
                    (
                        (":target_status", target_status),
                        (":updated_at", input.updated_at),
                        (":task_id", parent.id.as_str()),
                        (":board_id", parent.board_id.as_str()),
                        (":current_status", current_status),
                        (":lock_version", input.expected_lock_version),
                    ),
                )
                .await?
        } else {
            transaction
                .execute(
                    "UPDATE tasks SET updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND archived_at IS NULL AND lock_version = :lock_version",
                    (
                        (":updated_at", input.updated_at),
                        (":task_id", parent.id.as_str()),
                        (":board_id", parent.board_id.as_str()),
                        (":lock_version", input.expected_lock_version),
                    ),
                )
                .await?
        };
        if changed != 1 {
            return Err(StoreError::InvalidTransition(
                "删除 step 需要匹配的最新父任务".to_owned(),
            ));
        }
        if target_status != current_status {
            transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.recomputed', :actor, json_object('to_status', :to_status), :created_at)",
                    (
                        (":event_id", input.recompute_event_id.trim()),
                        (":board_id", parent.board_id.as_str()),
                        (":task_id", parent.id.as_str()),
                        (":actor", actor),
                        (":to_status", target_status),
                        (":created_at", input.updated_at),
                    ),
                )
                .await?;
        }

        transaction.commit().await?;
        Ok(existing)
    }
}

fn validate_remove_step_input(
    task_id: &str,
    step_id: &str,
    input: &RemoveStepInput,
) -> Result<(), StoreError> {
    if !task_id.trim().starts_with("t_") || task_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id 必须以 t_ 开头".to_owned(),
        ));
    }
    if !step_id.trim().starts_with("step_") || step_id.trim().len() <= 5 {
        return Err(StoreError::InvalidInput(
            "step id 必须以 step_ 开头".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor 不能为空".to_owned()));
    }
    if input.expected_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_lock_version 不能为负数".to_owned(),
        ));
    }
    for (name, value) in [
        ("event_id", input.event_id.as_str()),
        ("recompute_event_id", input.recompute_event_id.as_str()),
    ] {
        if !value.trim().starts_with("e_") || value.trim().len() <= 2 {
            return Err(StoreError::InvalidInput(format!("{name} 必须以 e_ 开头")));
        }
    }
    if input.updated_at < 0 {
        return Err(StoreError::InvalidInput("updated_at 不能为负数".to_owned()));
    }
    Ok(())
}
