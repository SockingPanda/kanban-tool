use std::collections::BTreeMap;

use kanban_core::{Clock, KanbanError, ReadinessFacts, Result, TaskStatus, initial_status};

use crate::{KanbanService, TaskRecord};

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTaskCommand {
    pub task_id: String,
    pub board: String,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub requested_status: Option<TaskStatus>,
    pub assignee: Option<String>,
    pub priority: i64,
    pub scheduled_at: Option<i64>,
    pub due_at: Option<i64>,
    pub max_retries: Option<i64>,
    pub metadata: BTreeMap<String, serde_json::Value>,
    /// 已存在的看板标签引用（名称或全局 label id）。
    pub labels: Vec<String>,
    /// 新任务创建时要挂接的父任务全局 id。
    pub depends_on: Vec<String>,
    pub actor: String,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn create_task(&self, command: CreateTaskCommand) -> Result<TaskRecord> {
        validate_create_task(&command)?;
        let status = initial_task_status(&command, self.clock.now_ms())?;
        let metadata_json = serde_json::to_string(&command.metadata)
            .map_err(|error| KanbanError::InvalidInput(format!("invalid metadata: {error}")))?;
        let board = command.board.trim().to_owned();
        let _mutation = self.mutation_gate.lock().await;
        self.application
            .store
            .store
            .create_task(
                &board,
                crate::store_operations::CreateTaskInput {
                    id: command.task_id,
                    idempotency_key: command.idempotency_key,
                    title: command.title.trim().to_owned(),
                    description: command.description,
                    status: status.as_str().to_owned(),
                    assignee: command.assignee,
                    priority: command.priority,
                    scheduled_at: command.scheduled_at,
                    due_at: command.due_at,
                    max_retries: command.max_retries,
                    metadata_json,
                    labels: command
                        .labels
                        .into_iter()
                        .map(|label| label.trim().to_owned())
                        .filter(|label| !label.is_empty())
                        .collect(),
                    depends_on: command
                        .depends_on
                        .into_iter()
                        .map(|task_id| task_id.trim().to_owned())
                        .filter(|task_id| !task_id.is_empty())
                        .collect(),
                    created_by: command.actor.trim().to_owned(),
                },
            )
            .await
            .map_err(crate::adapter::store_error)
            .and_then(super::application_task)
    }
}

fn initial_task_status(command: &CreateTaskCommand, now: i64) -> Result<TaskStatus> {
    let candidate = initial_status(
        command.requested_status,
        ReadinessFacts {
            title: &command.title,
            description: command.description.as_deref(),
            scheduled_at: command.scheduled_at,
            dependencies_done: true,
        },
        now,
    )?;
    // 每个新任务都从 unplanned execution plan 开始。因此，候选状态为 ready 的任务
    // 会保持 todo，直到提供计划或显式标记为 not_required。
    Ok(if candidate == TaskStatus::Ready {
        TaskStatus::Todo
    } else {
        candidate
    })
}

fn validate_create_task(command: &CreateTaskCommand) -> Result<()> {
    if command.board.trim().is_empty() {
        return Err(KanbanError::InvalidInput("board is required".to_owned()));
    }
    if !command.task_id.starts_with("t_") || command.task_id.len() <= 2 {
        return Err(KanbanError::InvalidInput(
            "task_id must start with t_".to_owned(),
        ));
    }
    if command.title.trim().is_empty() {
        return Err(KanbanError::InvalidInput("title is required".to_owned()));
    }
    if !(0..=3).contains(&command.priority) {
        return Err(KanbanError::InvalidInput(
            "priority must be between 0 and 3".to_owned(),
        ));
    }
    if command.max_retries.is_some_and(|value| value < 0) {
        return Err(KanbanError::InvalidInput(
            "max_retries must be non-negative".to_owned(),
        ));
    }
    if command.actor.trim().is_empty() {
        return Err(KanbanError::InvalidInput("actor is required".to_owned()));
    }
    if command.labels.iter().any(|label| label.trim().is_empty()) {
        return Err(KanbanError::InvalidInput(
            "labels must not contain empty values".to_owned(),
        ));
    }
    if command.depends_on.iter().any(|task_id| {
        let task_id = task_id.trim();
        !task_id.starts_with("t_") || task_id.len() <= 2
    }) {
        return Err(KanbanError::InvalidInput(
            "depends_on must contain global t_... ids".to_owned(),
        ));
    }
    if command
        .idempotency_key
        .as_deref()
        .is_some_and(|key| key.trim().is_empty())
    {
        return Err(KanbanError::InvalidInput(
            "idempotency_key must not be empty".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use kanban_core::TaskStatus;

    use super::{CreateTaskCommand, initial_task_status};

    fn command(
        requested_status: Option<TaskStatus>,
        scheduled_at: Option<i64>,
    ) -> CreateTaskCommand {
        CreateTaskCommand {
            task_id: "t_test".into(),
            board: "default".into(),
            idempotency_key: Some("retry-1".into()),
            title: " Ship ".into(),
            description: Some("ready spec".into()),
            requested_status,
            assignee: None,
            priority: 2,
            scheduled_at,
            due_at: None,
            max_retries: Some(3),
            metadata: BTreeMap::from([("source".into(), serde_json::json!("test"))]),
            labels: Vec::new(),
            depends_on: Vec::new(),
            actor: "tester".into(),
        }
    }

    #[test]
    fn create_task_applies_unplanned_guard() {
        let command = command(Some(TaskStatus::Ready), None);
        let status = initial_task_status(&command, 100).unwrap();
        assert_eq!(status, TaskStatus::Todo);
    }

    #[test]
    fn create_task_preserves_valid_scheduled_status() {
        let command = command(Some(TaskStatus::Scheduled), Some(200));
        let status = initial_task_status(&command, 100).unwrap();
        assert_eq!(status, TaskStatus::Scheduled);
    }

    #[test]
    fn create_task_rejects_invalid_command_before_serialization() {
        let mut command = command(None, None);
        command.priority = 4;
        assert!(super::validate_create_task(&command).is_err());

        let mut command = command(None, None);
        command.metadata = BTreeMap::new();
        command.labels = vec![" ".into()];
        assert!(super::validate_create_task(&command).is_err());
    }
}
