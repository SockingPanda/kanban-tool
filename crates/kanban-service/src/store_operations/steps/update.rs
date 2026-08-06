use turso::transaction::TransactionBehavior;

use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

use super::update_support::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateStepInput {
    pub title: Option<String>,
    pub body: Option<String>,
    pub linked_task_id: Option<String>,
    pub unlink_task: bool,
    pub position: Option<i64>,
    pub required: Option<bool>,
    pub updated_by: String,
    pub event_id: String,
    pub updated_at: i64,
    pub expected_lock_version: i64,
}

impl TursoStore {
    /// 更新可编辑的 execution-plan 字段，但不改变 step status 或父任务 status。父任务
    /// lock-version CAS 与 step/event 写入共享同一个 immediate transaction，因此过期调用方
    /// 无法覆盖并发的计划变更，事件冲突也会让全部操作回滚。
    pub async fn update_step(
        &self,
        task_id: &str,
        step_id: &str,
        input: UpdateStepInput,
    ) -> Result<TaskStepRecord, StoreError> {
        validate_update_step_input(task_id, step_id, &input)?;
        let title = input.title.as_deref().map(str::trim).map(str::to_owned);
        let body = input.body.as_deref().map(str::trim).map(str::to_owned);
        let updated_by = input.updated_by.trim().to_owned();
        let linked_task_id = input
            .linked_task_id
            .as_deref()
            .map(str::trim)
            .map(str::to_owned);
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let parent_row = first_row(
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
        })?;
        let parent = task_from_row(parent_row)?;
        if parent.archived_at.is_some() || parent.status == "archived" {
            return Err(StoreError::InvalidTransition(
                "archived parent task cannot receive step updates".to_owned(),
            ));
        }
        let board_row = first_row(
            transaction
                .query(
                    "SELECT archived_at FROM boards WHERE id = :board_id LIMIT 1",
                    [(":board_id", parent.board_id.as_str())],
                )
                .await?,
        )
        .await?;
        if optional_integer_value(board_row.get_value(0)?, "boards.archived_at")?.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived board cannot receive step updates".to_owned(),
            ));
        }

        let existing_row = first_row(
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
            })?;
        let existing = step_from_row(&transaction, existing_row).await?;
        let existing_linked_task_id = existing.linked_task.as_ref().map(|task| task.id.clone());
        let next_linked_task_id = if input.unlink_task {
            None
        } else {
            linked_task_id.or(existing_linked_task_id)
        };
        if let Some(linked_task_id) = next_linked_task_id.as_deref() {
            let linked_row = first_row(
                transaction
                    .query(
                        &format!("{TASK_SELECT} WHERE t.id = :task_id LIMIT 1"),
                        [(":task_id", linked_task_id)],
                    )
                    .await?,
            )
            .await
            .map_err(|error| match error {
                turso::Error::QueryReturnedNoRows => {
                    StoreError::TaskNotFound(linked_task_id.to_owned())
                }
                other => StoreError::Turso(other),
            })?;
            let linked = task_from_row(linked_row)?;
            if linked.board_id != parent.board_id {
                return Err(StoreError::InvalidInput(
                    "linked task must belong to the parent board".to_owned(),
                ));
            }
            if linked.id == parent.id {
                return Err(StoreError::InvalidInput(
                    "step cannot link to its parent task".to_owned(),
                ));
            }
            if linked.archived_at.is_some() || linked.status == "archived" {
                return Err(StoreError::InvalidInput(
                    "archived linked task is not allowed".to_owned(),
                ));
            }
        }

        if parent.lock_version != input.expected_lock_version {
            return Err(StoreError::InvalidTransition(
                "step update requires matching fresh parent task".to_owned(),
            ));
        }
        let changed_parent = transaction
                .execute(
                    "UPDATE tasks SET lock_version = lock_version + 1, updated_at = :updated_at WHERE id = :task_id AND board_id = :board_id AND archived_at IS NULL AND status != 'archived' AND lock_version = :lock_version",
                    (
                        (":updated_at", input.updated_at),
                        (":task_id", parent.id.as_str()),
                        (":board_id", parent.board_id.as_str()),
                        (":lock_version", input.expected_lock_version),
                    ),
                )
                .await?;
        if changed_parent != 1 {
            return Err(StoreError::InvalidTransition(
                "step update requires matching fresh parent task".to_owned(),
            ));
        }

        let next_title = title.as_deref().unwrap_or(existing.title.as_str());
        let next_body = body.as_deref().or(existing.body.as_deref());
        let next_position = input.position.unwrap_or(existing.position);
        let next_required = input.required.unwrap_or(existing.required);
        transaction
                .execute(
                    "UPDATE task_steps SET title = :title, body = :body, linked_task_id = :linked_task_id, position = :position, required = :required, updated_by = :updated_by, updated_at = :updated_at WHERE board_id = :board_id AND parent_task_id = :parent_task_id AND id = :step_id",
                    (
                        (":title", next_title),
                        (":body", next_body),
                        (":linked_task_id", next_linked_task_id.as_deref()),
                        (":position", next_position),
                        (":required", if next_required { 1_i64 } else { 0_i64 }),
                        (":updated_by", updated_by.as_str()),
                        (":updated_at", input.updated_at),
                        (":board_id", parent.board_id.as_str()),
                        (":parent_task_id", parent.id.as_str()),
                        (":step_id", step_id),
                    ),
                )
                .await?;

        transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.step.updated', :actor, json_object('step_id', :step_id, 'linked_task_id', :linked_task_id, 'position', :position, 'required', json(CASE WHEN :required = 1 THEN 'true' ELSE 'false' END), 'status', :status), :created_at)",
                    (
                        (":event_id", input.event_id.as_str()),
                        (":board_id", parent.board_id.as_str()),
                        (":task_id", parent.id.as_str()),
                        (":actor", updated_by.as_str()),
                        (":step_id", step_id),
                        (":linked_task_id", next_linked_task_id.as_deref()),
                        (":position", next_position),
                        (":required", if next_required { 1_i64 } else { 0_i64 }),
                        (":status", existing.status.as_str()),
                        (":created_at", input.updated_at),
                    ),
                )
                .await?;

        let updated = step_from_row(
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
                .await?,
            )
            .await?;
        transaction.commit().await?;
        Ok(updated)
    }
}
