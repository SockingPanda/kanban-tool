use turso::transaction::TransactionBehavior;

use crate::{
    db::TursoStore, domain::*, error::StoreError, store_operations::shared::canonical_ready_status,
    shared::*,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateTaskInput {
    pub expected_lock_version: i64,
    pub actor: String,
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub assignee: Option<Option<String>>,
    pub priority: Option<i64>,
    pub scheduled_at: Option<Option<i64>>,
    pub due_at: Option<Option<i64>>,
    pub max_retries: Option<Option<i64>>,
    pub metadata_json: Option<String>,
    pub event_id: String,
    pub now: i64,
}

impl TursoStore {
    pub async fn update_task(
        &self,
        task_id: &str,
        input: UpdateTaskInput,
    ) -> Result<TaskRecord, StoreError> {
        validate_update_task_input(task_id, &input)?;
        let task_id = task_id.trim();
        let actor = input.actor.trim().to_owned();
        let event_id = input.event_id.trim().to_owned();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        if let Some(metadata_json) = input.metadata_json.as_deref() {
            let valid = first_row(
                transaction
                    .query(
                        "SELECT json_valid(:metadata_json)",
                        [(":metadata_json", metadata_json)],
                    )
                    .await?,
            )
            .await?;
            if integer_value(valid.get_value(0)?, "tasks.metadata_json_valid")? == 0 {
                return Err(StoreError::InvalidInput(
                    "metadata_json 必须是有效 JSON".to_owned(),
                ));
            }
        }

        let row = first_row(
            transaction
                .query(
                    "SELECT t.board_id, t.status, t.archived_at, b.archived_at, t.lock_version, t.title, t.description, t.assignee, t.priority, t.scheduled_at, t.due_at, t.max_retries FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
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
        if optional_integer_value(row.get_value(2)?, "tasks.archived_at")?.is_some()
            || optional_integer_value(row.get_value(3)?, "boards.archived_at")?.is_some()
        {
            return Err(StoreError::InvalidTransition(
                "已归档 task 或 board 不能 update".to_owned(),
            ));
        }
        let lock_version = integer_value(row.get_value(4)?, "tasks.lock_version")?;
        if lock_version != input.expected_lock_version {
            return Err(StoreError::ClaimConflict("lock_version 不匹配".to_owned()));
        }
        let current_title = text_value(row.get_value(5)?, "tasks.title")?;
        let current_description = optional_text_value(row.get_value(6)?, "tasks.description")?;
        let current_assignee = optional_text_value(row.get_value(7)?, "tasks.assignee")?;
        let current_priority = integer_value(row.get_value(8)?, "tasks.priority")?;
        let current_scheduled_at = optional_integer_value(row.get_value(9)?, "tasks.scheduled_at")?;
        let current_due_at = optional_integer_value(row.get_value(10)?, "tasks.due_at")?;
        let current_max_retries = optional_integer_value(row.get_value(11)?, "tasks.max_retries")?;
        let title = input.title.clone().unwrap_or(current_title);
        let description = input.description.clone().unwrap_or(current_description);
        let assignee = input.assignee.clone().unwrap_or(current_assignee);
        let priority = input.priority.unwrap_or(current_priority);
        let scheduled_at = input.scheduled_at.unwrap_or(current_scheduled_at);
        let due_at = input.due_at.unwrap_or(current_due_at);
        let max_retries = input.max_retries.unwrap_or(current_max_retries);

        let target_status = if matches!(status.as_str(), "triage" | "todo" | "scheduled" | "ready")
        {
            let dependency_blocked = first_row(
                transaction
                    .query(
                        "SELECT EXISTS (SELECT 1 FROM task_dependencies AS d JOIN tasks AS p ON p.id = d.parent_task_id AND p.board_id = d.board_id WHERE d.board_id = :board_id AND d.child_task_id = :task_id AND p.status NOT IN ('done', 'archived'))",
                        [(":board_id", board_id.as_str()), (":task_id", task_id)],
                    )
                    .await?,
            )
            .await?;
            let dependencies_done = integer_value(
                dependency_blocked.get_value(0)?,
                "task_dependencies.blocked",
            )? == 0;
            let recomputed = canonical_ready_status(
                &title,
                description.as_deref(),
                scheduled_at,
                dependencies_done,
                input.now,
            );
            let executable_plan = first_row(
                transaction
                    .query(
                        "SELECT EXISTS (SELECT 1 FROM task_steps WHERE board_id = :board_id AND parent_task_id = :task_id) OR EXISTS (SELECT 1 FROM task_execution_plans WHERE board_id = :board_id AND task_id = :task_id AND state = 'not_required')",
                        [(":board_id", board_id.as_str()), (":task_id", task_id)],
                    )
                    .await?,
            )
            .await?;
            if recomputed == "ready"
                && integer_value(executable_plan.get_value(0)?, "task_execution_plans.ready")? == 0
            {
                "todo"
            } else {
                recomputed
            }
        } else {
            status.as_str()
        };

        let mut sets = vec![
            "updated_at = :updated_at".to_owned(),
            "lock_version = lock_version + 1".to_owned(),
        ];
        let mut params: Vec<(String, Value)> = vec![
            (":updated_at".to_owned(), Value::Integer(input.now)),
            (":task_id".to_owned(), Value::Text(task_id.to_owned())),
            (":board_id".to_owned(), Value::Text(board_id.clone())),
            (
                ":expected_lock_version".to_owned(),
                Value::Integer(input.expected_lock_version),
            ),
        ];
        if input.title.is_some() {
            sets.push("title = :title".to_owned());
            params.push((":title".to_owned(), Value::Text(title)));
        }
        if input.description.is_some() {
            sets.push("description = :description".to_owned());
            params.push((
                ":description".to_owned(),
                description.map_or(Value::Null, Value::Text),
            ));
        }
        if input.assignee.is_some() {
            sets.push("assignee = :assignee".to_owned());
            params.push((
                ":assignee".to_owned(),
                assignee.map_or(Value::Null, Value::Text),
            ));
        }
        if input.priority.is_some() {
            sets.push("priority = :priority".to_owned());
            params.push((":priority".to_owned(), Value::Integer(priority)));
        }
        if input.scheduled_at.is_some() {
            sets.push("scheduled_at = :scheduled_at".to_owned());
            params.push((
                ":scheduled_at".to_owned(),
                scheduled_at.map_or(Value::Null, Value::Integer),
            ));
        }
        if input.due_at.is_some() {
            sets.push("due_at = :due_at".to_owned());
            params.push((
                ":due_at".to_owned(),
                due_at.map_or(Value::Null, Value::Integer),
            ));
        }
        if input.max_retries.is_some() {
            sets.push("max_retries = :max_retries".to_owned());
            params.push((
                ":max_retries".to_owned(),
                max_retries.map_or(Value::Null, Value::Integer),
            ));
        }
        if let Some(metadata_json) = input.metadata_json {
            sets.push("metadata_json = :metadata_json".to_owned());
            params.push((":metadata_json".to_owned(), Value::Text(metadata_json)));
        }
        if target_status != status {
            sets.push("status = :status".to_owned());
            sets.push("status_reason = NULL".to_owned());
            params.push((":status".to_owned(), Value::Text(target_status.to_owned())));
        }
        let sql = format!(
            "UPDATE tasks SET {} WHERE id = :task_id AND board_id = :board_id AND lock_version = :expected_lock_version",
            sets.join(", ")
        );
        if transaction.execute(&sql, params).await? != 1 {
            return Err(StoreError::ClaimConflict(
                "任务 update compare-and-set 失败".to_owned(),
            ));
        }
        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.updated', :actor, '{}', :created_at)",
                vec![
                    (":event_id".to_owned(), Value::Text(event_id.clone())),
                    (":board_id".to_owned(), Value::Text(board_id.clone())),
                    (":task_id".to_owned(), Value::Text(task_id.to_owned())),
                    (":actor".to_owned(), Value::Text(actor.clone())),
                    (":created_at".to_owned(), Value::Integer(input.now)),
                ],
            )
            .await?;
        let updated = task_from_row(
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
        Ok(updated)
    }
}

fn validate_update_task_input(task_id: &str, input: &UpdateTaskInput) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id 必须以 t_ 开头".to_owned(),
        ));
    }
    if input.expected_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_lock_version 不能为负数".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor 不能为空".to_owned()));
    }
    if input
        .title
        .as_deref()
        .is_some_and(|title| title.trim().is_empty())
    {
        return Err(StoreError::InvalidInput("title 不能为空".to_owned()));
    }
    if input
        .priority
        .is_some_and(|priority| !(0..=3).contains(&priority))
    {
        return Err(StoreError::InvalidInput(
            "priority 必须在 0 到 3 之间".to_owned(),
        ));
    }
    if input.max_retries.flatten().is_some_and(|value| value <= 0) {
        return Err(StoreError::InvalidInput(
            "max_retries 必须大于 0".to_owned(),
        ));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id 必须以 e_ 开头".to_owned(),
        ));
    }
    if input.now < 0 {
        return Err(StoreError::InvalidInput("now 不能为负数".to_owned()));
    }
    Ok(())
}
