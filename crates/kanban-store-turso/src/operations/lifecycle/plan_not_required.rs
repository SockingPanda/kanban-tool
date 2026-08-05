use turso::transaction::TransactionBehavior;

use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkExecutionPlanNotRequiredInput {
    pub reason: String,
    pub actor: String,
    pub event_id: String,
    pub updated_at: i64,
}
impl TursoStore {
    pub async fn mark_execution_plan_not_required(
        &self,
        task_id: &str,
        input: MarkExecutionPlanNotRequiredInput,
    ) -> Result<TaskExecutionPlanRecord, StoreError> {
        validate_plan_not_required_input(task_id, &input)?;
        let reason = input.reason.trim().to_owned();
        let actor = input.actor.trim().to_owned();
        let event_id = input.event_id.trim().to_owned();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let task = first_row(
                transaction
                    .query(
                        "SELECT t.board_id, t.status, t.archived_at, b.archived_at FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
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
        let archived_at = optional_integer_value(task.get_value(2)?, "tasks.archived_at")?;
        let board_archived_at = optional_integer_value(task.get_value(3)?, "boards.archived_at")?;
        if status == "archived" || archived_at.is_some() || board_archived_at.is_some() {
            return Err(StoreError::InvalidInput(
                "archived tasks or boards cannot be marked not_required".to_owned(),
            ));
        }

        let steps = first_row(
                transaction
                    .query(
                        "SELECT id FROM task_steps WHERE board_id = :board_id AND parent_task_id = :task_id LIMIT 1",
                        [
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                        ],
                    )
                    .await?,
            )
            .await;
        match steps {
            Ok(_) => {
                return Err(StoreError::InvalidInput(
                    "tasks with steps cannot be marked not_required".to_owned(),
                ));
            }
            Err(turso::Error::QueryReturnedNoRows) => {}
            Err(error) => return Err(StoreError::Turso(error)),
        }

        let previous_state = first_row(
                transaction
                    .query(
                        "SELECT state FROM task_execution_plans WHERE board_id = :board_id AND task_id = :task_id LIMIT 1",
                        [
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                        ],
                    )
                    .await?,
            )
            .await;
        let previous_state = match previous_state {
            Ok(row) => Some(text_value(row.get_value(0)?, "task_execution_plans.state")?),
            Err(turso::Error::QueryReturnedNoRows) => None,
            Err(error) => return Err(StoreError::Turso(error)),
        };

        if previous_state.is_some() {
            transaction
                    .execute(
                        "UPDATE task_execution_plans SET state = 'not_required', reason = :reason, updated_by = :updated_by, updated_at = :updated_at WHERE board_id = :board_id AND task_id = :task_id",
                        (
                            (":reason", reason.as_str()),
                            (":updated_by", actor.as_str()),
                            (":updated_at", input.updated_at),
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                        ),
                    )
                    .await?;
        } else {
            transaction
                    .execute(
                        "INSERT INTO task_execution_plans(board_id, task_id, state, reason, updated_by, updated_at) VALUES (:board_id, :task_id, 'not_required', :reason, :updated_by, :updated_at)",
                        (
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                            (":reason", reason.as_str()),
                            (":updated_by", actor.as_str()),
                            (":updated_at", input.updated_at),
                        ),
                    )
                    .await?;
        }

        if previous_state.as_deref() != Some("not_required") {
            transaction
                    .execute(
                        "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.execution_plan.not_required', :actor, '{\"state\":\"not_required\"}', :created_at)",
                        (
                            (":event_id", event_id.as_str()),
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                            (":actor", actor.as_str()),
                            (":created_at", input.updated_at),
                        ),
                    )
                    .await?;
        }

        let plan = first_row(
                transaction
                    .query(
                        "SELECT board_id, task_id, state, reason, updated_by, updated_at FROM task_execution_plans WHERE board_id = :board_id AND task_id = :task_id LIMIT 1",
                        [
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                        ],
                    )
                    .await?,
            )
            .await?;
        let result = TaskExecutionPlanRecord {
            board_id: text_value(plan.get_value(0)?, "task_execution_plans.board_id")?,
            task_id: text_value(plan.get_value(1)?, "task_execution_plans.task_id")?,
            state: text_value(plan.get_value(2)?, "task_execution_plans.state")?,
            reason: optional_text_value(plan.get_value(3)?, "task_execution_plans.reason")?,
            updated_by: text_value(plan.get_value(4)?, "task_execution_plans.updated_by")?,
            updated_at: integer_value(plan.get_value(5)?, "task_execution_plans.updated_at")?,
        };

        transaction.commit().await?;
        Ok(result)
    }
}

pub(crate) fn validate_plan_not_required_input(
    task_id: &str,
    input: &MarkExecutionPlanNotRequiredInput,
) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    if input.reason.trim().is_empty() {
        return Err(StoreError::InvalidInput("reason is required".to_owned()));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id must start with e_".to_owned(),
        ));
    }
    if input.updated_at < 0 {
        return Err(StoreError::InvalidInput(
            "updated_at must be non-negative".to_owned(),
        ));
    }
    Ok(())
}
