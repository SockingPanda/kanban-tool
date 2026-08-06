use turso::transaction::TransactionBehavior;

use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteStepInput {
    pub note: String,
    pub actor: String,
    pub event_id: String,
    pub updated_at: i64,
    pub expected_lock_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipStepInput {
    pub reason: String,
    pub actor: String,
    pub event_id: String,
    pub updated_at: i64,
    pub expected_lock_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReopenStepInput {
    pub reason: String,
    pub actor: String,
    pub event_id: String,
    pub updated_at: i64,
    pub expected_lock_version: i64,
}

struct StepResolution<'a> {
    task_id: &'a str,
    step_id: &'a str,
    note: &'a str,
    actor: &'a str,
    event_id: &'a str,
    status: &'a str,
    event_kind: &'a str,
    updated_at: i64,
    expected_lock_version: i64,
}

impl TursoStore {
    pub async fn complete_step(
        &self,
        task_id: &str,
        step_id: &str,
        input: CompleteStepInput,
    ) -> Result<TaskStepRecord, StoreError> {
        validate_resolution_input(
            task_id,
            step_id,
            &input.note,
            &input.actor,
            &input.event_id,
            input.updated_at,
            input.expected_lock_version,
        )?;
        self.resolve_step(StepResolution {
            task_id,
            step_id,
            note: &input.note,
            actor: &input.actor,
            event_id: &input.event_id,
            status: "done",
            event_kind: "task.step.done",
            updated_at: input.updated_at,
            expected_lock_version: input.expected_lock_version,
        })
        .await
    }

    pub async fn skip_step(
        &self,
        task_id: &str,
        step_id: &str,
        input: SkipStepInput,
    ) -> Result<TaskStepRecord, StoreError> {
        validate_resolution_input(
            task_id,
            step_id,
            &input.reason,
            &input.actor,
            &input.event_id,
            input.updated_at,
            input.expected_lock_version,
        )?;
        self.resolve_step(StepResolution {
            task_id,
            step_id,
            note: &input.reason,
            actor: &input.actor,
            event_id: &input.event_id,
            status: "skipped",
            event_kind: "task.step.skipped",
            updated_at: input.updated_at,
            expected_lock_version: input.expected_lock_version,
        })
        .await
    }

    pub async fn reopen_step(
        &self,
        task_id: &str,
        step_id: &str,
        input: ReopenStepInput,
    ) -> Result<TaskStepRecord, StoreError> {
        validate_resolution_input(
            task_id,
            step_id,
            &input.reason,
            &input.actor,
            &input.event_id,
            input.updated_at,
            input.expected_lock_version,
        )?;
        self.resolve_step(StepResolution {
            task_id,
            step_id,
            note: &input.reason,
            actor: &input.actor,
            event_id: &input.event_id,
            status: "todo",
            event_kind: "task.step.reopened",
            updated_at: input.updated_at,
            expected_lock_version: input.expected_lock_version,
        })
        .await
    }

    async fn resolve_step(&self, input: StepResolution<'_>) -> Result<TaskStepRecord, StoreError> {
        let StepResolution {
            task_id,
            step_id,
            note,
            actor,
            event_id,
            status,
            event_kind,
            updated_at,
            expected_lock_version,
        } = input;
        let task_id = task_id.trim();
        let step_id = step_id.trim();
        let actor = actor.trim();
        let note = note.trim();
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
        if parent.archived_at.is_some() || parent.status == "archived" {
            return Err(StoreError::InvalidTransition(
                "已归档的父任务不能修改 step".to_owned(),
            ));
        }
        let board = first_row(
            transaction
                .query(
                    "SELECT archived_at FROM boards WHERE id = :board_id LIMIT 1",
                    [(":board_id", parent.board_id.as_str())],
                )
                .await?,
        )
        .await?;
        if optional_integer_value(board.get_value(0)?, "boards.archived_at")?.is_some() {
            return Err(StoreError::InvalidTransition(
                "已归档的 board 不能修改 step".to_owned(),
            ));
        }
        if parent.lock_version != expected_lock_version {
            return Err(StoreError::InvalidTransition(
                "step resolution 需要匹配的最新父任务".to_owned(),
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
        let (resolution_note, resolved_by, resolved_at): (Option<&str>, Option<&str>, Option<i64>) =
            if status == "todo" {
                (None, None, None)
            } else {
                (Some(note), Some(actor), Some(updated_at))
            };

        transaction
            .execute(
                "UPDATE task_steps SET status = :status, resolution_note = :resolution_note, resolved_by = :resolved_by, resolved_at = :resolved_at, updated_by = :updated_by, updated_at = :updated_at WHERE board_id = :board_id AND parent_task_id = :parent_task_id AND id = :step_id",
                (
                    (":status", status),
                    (":resolution_note", resolution_note),
                    (":resolved_by", resolved_by),
                    (":resolved_at", resolved_at),
                    (":updated_by", actor),
                    (":updated_at", updated_at),
                    (":board_id", parent.board_id.as_str()),
                    (":parent_task_id", parent.id.as_str()),
                    (":step_id", step_id),
                ),
            )
            .await?;

        let changed = transaction
            .execute(
                "UPDATE tasks SET updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND archived_at IS NULL AND status != 'archived' AND lock_version = :lock_version",
                (
                    (":updated_at", updated_at),
                    (":task_id", parent.id.as_str()),
                    (":board_id", parent.board_id.as_str()),
                    (":lock_version", expected_lock_version),
                ),
            )
            .await?;
        if changed != 1 {
            return Err(StoreError::InvalidTransition(
                "step resolution 需要匹配的最新父任务".to_owned(),
            ));
        }

        transaction
            .execute(
                &format!("INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, '{event_kind}', :actor, json_object('step_id', :step_id, 'linked_task_id', :linked_task_id, 'position', :position, 'required', json(CASE WHEN :required = 1 THEN 'true' ELSE 'false' END), 'status', :status), :created_at)"),
                (
                    (":event_id", event_id.trim()),
                    (":board_id", parent.board_id.as_str()),
                    (":task_id", parent.id.as_str()),
                    (":actor", actor),
                    (":step_id", existing.id.as_str()),
                    (":linked_task_id", existing.linked_task.as_ref().map(|task| task.id.as_str())),
                    (":position", existing.position),
                    (":required", if existing.required { 1_i64 } else { 0_i64 }),
                    (":status", status),
                    (":created_at", updated_at),
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

fn validate_resolution_input(
    task_id: &str,
    step_id: &str,
    note: &str,
    actor: &str,
    event_id: &str,
    updated_at: i64,
    expected_lock_version: i64,
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
    if note.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "step resolution note/reason 不能为空".to_owned(),
        ));
    }
    if actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor 不能为空".to_owned()));
    }
    if !event_id.trim().starts_with("e_") || event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id 必须以 e_ 开头".to_owned(),
        ));
    }
    if updated_at < 0 {
        return Err(StoreError::InvalidInput("updated_at 不能为负数".to_owned()));
    }
    if expected_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_lock_version 不能为负数".to_owned(),
        ));
    }
    Ok(())
}
