use std::future::Future;

use kanban_core::{Clock, KanbanError, ReadinessFacts, Result, TaskStatus, initial_status};

use crate::{ApplicationService, ApplicationStore, TaskRecord};

use std::collections::BTreeMap;

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
    pub actor: String,
}

/// Canonicalized input passed from the application service to persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTaskRecord {
    pub id: String,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub assignee: Option<String>,
    pub priority: i64,
    pub scheduled_at: Option<i64>,
    pub due_at: Option<i64>,
    pub max_retries: Option<i64>,
    pub metadata_json: String,
    pub created_by: String,
}

pub trait TaskCreate: ApplicationStore {
    fn create_task(
        &self,
        board: &str,
        input: CreateTaskRecord,
    ) -> impl Future<Output = Result<TaskRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: TaskCreate,
    C: Clock,
{
    pub async fn create_task(&self, command: CreateTaskCommand) -> Result<TaskRecord> {
        validate_create_task(&command)?;
        let candidate = initial_status(
            command.requested_status,
            ReadinessFacts {
                title: &command.title,
                description: command.description.as_deref(),
                scheduled_at: command.scheduled_at,
                dependencies_done: true,
            },
            self.clock.now_ms(),
        )?;
        // Every new task starts with an unplanned execution plan. A candidate
        // ready task therefore remains todo until a plan is supplied or
        // explicitly marked not required.
        let status = if candidate == TaskStatus::Ready {
            TaskStatus::Todo
        } else {
            candidate
        };
        let metadata_json = serde_json::to_string(&command.metadata)
            .map_err(|error| KanbanError::InvalidInput(format!("invalid metadata: {error}")))?;
        let board = command.board.trim().to_owned();
        let _mutation = self.mutation_gate.lock().await;
        self.store
            .create_task(
                &board,
                CreateTaskRecord {
                    id: command.task_id,
                    idempotency_key: command.idempotency_key,
                    title: command.title.trim().to_owned(),
                    description: command.description,
                    status,
                    assignee: command.assignee,
                    priority: command.priority,
                    scheduled_at: command.scheduled_at,
                    due_at: command.due_at,
                    max_retries: command.max_retries,
                    metadata_json,
                    created_by: command.actor.trim().to_owned(),
                },
            )
            .await
    }
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
    use std::sync::{Arc, atomic::AtomicUsize};

    use kanban_core::{Result, TaskStatus};

    use crate::operations::test_support::{FixedClock, StubStore};
    use crate::*;

    impl TaskCreate for StubStore {
        async fn create_task(&self, board: &str, input: CreateTaskRecord) -> Result<TaskRecord> {
            assert_eq!(board, "default");
            Ok(crate::operations::test_support::task_record(input))
        }
    }
    #[tokio::test]
    async fn create_task_validates_status_and_applies_unplanned_guard() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let command = CreateTaskCommand {
            task_id: "t_test".into(),
            board: "default".into(),
            idempotency_key: Some("retry-1".into()),
            title: " Ship ".into(),
            description: Some("ready spec".into()),
            requested_status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 2,
            scheduled_at: None,
            due_at: None,
            max_retries: Some(3),
            metadata: BTreeMap::from([("source".into(), serde_json::json!("test"))]),
            actor: "tester".into(),
        };

        let task = service.create_task(command).await.unwrap();
        assert_eq!(task.status, TaskStatus::Todo);
        assert_eq!(task.title, "Ship");
        assert_eq!(task.execution_plan_state, ExecutionPlanState::Unplanned);
    }

    #[tokio::test]
    async fn create_task_preserves_valid_scheduled_status() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let task = service
            .create_task(CreateTaskCommand {
                task_id: "t_scheduled".into(),
                board: "default".into(),
                idempotency_key: None,
                title: "Later".into(),
                description: Some("ready spec".into()),
                requested_status: Some(TaskStatus::Scheduled),
                assignee: None,
                priority: 3,
                scheduled_at: Some(200),
                due_at: None,
                max_retries: None,
                metadata: BTreeMap::new(),
                actor: "tester".into(),
            })
            .await
            .unwrap();
        assert_eq!(task.status, TaskStatus::Scheduled);
    }
}
