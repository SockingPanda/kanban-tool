use crate::operations::shared::validate_task_id;
use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

impl TursoStore {
    pub async fn list_steps(&self, task_id: &str) -> Result<TaskStepsRecord, StoreError> {
        validate_task_id(task_id)?;
        let connection = self.connection().await?;
        let task = first_row(
            connection
                .query(
                    "SELECT board_id FROM tasks WHERE id = :task_id LIMIT 1",
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
        let mut rows = connection
                .query(
                    "SELECT id, board_id, parent_task_id, position, title, body, linked_task_id, required, status, resolution_note, resolved_by, resolved_at, created_by, created_at, updated_by, updated_at FROM task_steps WHERE board_id = :board_id AND parent_task_id = :task_id ORDER BY position ASC, created_at ASC, id ASC",
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?;
        let mut steps = Vec::new();
        while let Some(row) = rows.next().await? {
            steps.push(step_from_row(&connection, row).await?);
        }
        let plan = first_row(
                connection
                    .query(
                        "SELECT board_id, task_id, state, reason, updated_by, updated_at FROM task_execution_plans WHERE board_id = :board_id AND task_id = :task_id LIMIT 1",
                        [
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                        ],
                    )
                    .await?,
            )
            .await;
        let execution_plan = match plan {
            Ok(row) => TaskExecutionPlanRecord {
                board_id: text_value(row.get_value(0)?, "task_execution_plans.board_id")?,
                task_id: text_value(row.get_value(1)?, "task_execution_plans.task_id")?,
                state: text_value(row.get_value(2)?, "task_execution_plans.state")?,
                reason: optional_text_value(row.get_value(3)?, "task_execution_plans.reason")?,
                updated_by: text_value(row.get_value(4)?, "task_execution_plans.updated_by")?,
                updated_at: integer_value(row.get_value(5)?, "task_execution_plans.updated_at")?,
            },
            Err(turso::Error::QueryReturnedNoRows) => TaskExecutionPlanRecord {
                board_id: board_id.clone(),
                task_id: task_id.to_owned(),
                state: "unplanned".to_owned(),
                reason: None,
                updated_by: "system".to_owned(),
                updated_at: 0,
            },
            Err(error) => return Err(StoreError::Turso(error)),
        };
        Ok(TaskStepsRecord {
            task_id: task_id.to_owned(),
            steps,
            execution_plan,
        })
    }
}
