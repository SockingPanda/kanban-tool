use kanban_core::{
    Clock, KanbanError, ReadinessFacts, Result, SystemClock, TaskStatus, initial_status,
};

use crate::{
    ApplicationHealth, ApplicationStore, BoardColumnRecord, BoardRecord, CreateTaskCommand,
    CreateTaskRecord, TaskListOptions, TaskListPage, TaskRecord,
};

const MAX_TASK_LIST_LIMIT: usize = 1_000;
const MAX_TASK_QUERY_CHARS: usize = 1_024;
const MAX_TASK_ASSIGNEE_CHARS: usize = 128;

/// The canonical command/query entry point shared by the HTTP handlers and the
/// in-process dispatcher.
#[derive(Debug, Clone)]
pub struct ApplicationService<S, C = SystemClock> {
    store: S,
    clock: C,
}

impl<S> ApplicationService<S, SystemClock>
where
    S: ApplicationStore,
{
    pub fn new(store: S) -> Self {
        Self {
            store,
            clock: SystemClock,
        }
    }
}

impl<S, C> ApplicationService<S, C>
where
    S: ApplicationStore,
    C: Clock,
{
    pub fn with_clock(store: S, clock: C) -> Self {
        Self { store, clock }
    }

    pub async fn health(&self) -> Result<ApplicationHealth> {
        // A real store query proves that the initialized canonical database is
        // still reachable without exposing a raw connection to the handler.
        self.store.list_boards(true).await?;
        Ok(ApplicationHealth { ok: true })
    }

    pub async fn list_boards(&self, include_archived: bool) -> Result<Vec<BoardRecord>> {
        self.store.list_boards(include_archived).await
    }

    pub async fn list_board_columns(&self, board: &str) -> Result<Vec<BoardColumnRecord>> {
        self.store.list_board_columns(board).await
    }

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

    pub async fn list_tasks(
        &self,
        board: &str,
        mut options: TaskListOptions,
    ) -> Result<TaskListPage> {
        let board = board.trim();
        if board.is_empty() {
            return Err(KanbanError::InvalidInput("board is required".to_owned()));
        }
        if options.limit > MAX_TASK_LIST_LIMIT {
            return Err(KanbanError::InvalidInput(format!(
                "limit must be <= {MAX_TASK_LIST_LIMIT}"
            )));
        }
        if options
            .assignee
            .as_deref()
            .is_some_and(|value| value.chars().count() > MAX_TASK_ASSIGNEE_CHARS)
        {
            return Err(KanbanError::InvalidInput(format!(
                "assignee exceeds {MAX_TASK_ASSIGNEE_CHARS} characters"
            )));
        }
        if options
            .query
            .as_deref()
            .is_some_and(|value| value.chars().count() > MAX_TASK_QUERY_CHARS)
        {
            return Err(KanbanError::InvalidInput(format!(
                "query exceeds {MAX_TASK_QUERY_CHARS} characters"
            )));
        }
        if options
            .priorities
            .iter()
            .any(|value| !(0..=3).contains(value))
        {
            return Err(KanbanError::InvalidInput(
                "priority filters must be between 0 and 3".to_owned(),
            ));
        }
        options.assignee = trimmed_optional(options.assignee);
        options.query = trimmed_optional(options.query);
        self.store.list_tasks(board, options).await
    }

    pub async fn get_task(&self, task_id: &str) -> Result<TaskRecord> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        self.store.get_task(task_id).await
    }
}

fn trimmed_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_owned())
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
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use kanban_core::{Board, Clock, Result, TaskStatus};

    use super::*;
    use crate::ExecutionPlanState;

    #[derive(Clone)]
    struct StubStore {
        calls: Arc<AtomicUsize>,
    }

    impl ApplicationStore for StubStore {
        async fn list_boards(&self, include_archived: bool) -> Result<Vec<BoardRecord>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert!(include_archived);
            Ok(vec![Board {
                id: "b_default".into(),
                slug: "default".into(),
                name: "Default".into(),
                description: None,
                created_at: 1,
                updated_at: 1,
                archived_at: None,
            }])
        }

        async fn list_board_columns(&self, board: &str) -> Result<Vec<BoardColumnRecord>> {
            assert_eq!(board, "default");
            Ok(vec![BoardColumnRecord {
                id: "col_default_todo".into(),
                board_id: "b_default".into(),
                status: TaskStatus::Todo,
                title: "Todo".into(),
                position: 20,
                hidden: false,
                wip_limit: None,
                created_at: 1,
                updated_at: 1,
            }])
        }

        async fn create_task(&self, board: &str, input: CreateTaskRecord) -> Result<TaskRecord> {
            assert_eq!(board, "default");
            Ok(task_record(input))
        }

        async fn list_tasks(&self, board: &str, options: TaskListOptions) -> Result<TaskListPage> {
            assert_eq!(board, "default");
            assert_eq!(options.assignee.as_deref(), Some("worker"));
            assert_eq!(options.query.as_deref(), Some("needle"));
            Ok(TaskListPage {
                tasks: Vec::new(),
                total: 0,
            })
        }

        async fn get_task(&self, task_id: &str) -> Result<TaskRecord> {
            assert_eq!(task_id, "t_show");
            Ok(task_record(CreateTaskRecord {
                id: task_id.to_owned(),
                idempotency_key: None,
                title: "Shown".into(),
                description: Some("task details".into()),
                status: TaskStatus::Todo,
                assignee: None,
                priority: 2,
                scheduled_at: None,
                due_at: None,
                max_retries: None,
                metadata_json: "{}".into(),
                created_by: "tester".into(),
            }))
        }
    }

    #[derive(Clone, Copy)]
    struct FixedClock(i64);

    impl Clock for FixedClock {
        fn now_ms(&self) -> i64 {
            self.0
        }
    }

    #[tokio::test]
    async fn health_and_board_queries_share_the_application_store() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = ApplicationService::new(StubStore {
            calls: calls.clone(),
        });

        assert!(service.health().await.unwrap().ok);
        assert_eq!(service.list_boards(true).await.unwrap().len(), 1);
        assert_eq!(
            service.list_board_columns("default").await.unwrap()[0].status,
            TaskStatus::Todo
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
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

    #[tokio::test]
    async fn list_tasks_validates_and_normalizes_query_options() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let page = service
            .list_tasks(
                " default ",
                TaskListOptions {
                    statuses: vec![TaskStatus::Todo],
                    priorities: vec![1],
                    plan_filters: Vec::new(),
                    assignee: Some(" worker ".into()),
                    query: Some(" needle ".into()),
                    include_archived: false,
                    limit: 25,
                    offset: 0,
                    sort: crate::TaskListSort::UpdatedAtDesc,
                },
            )
            .await
            .unwrap();
        assert_eq!(page.total, 0);

        let error = service
            .list_tasks(
                "default",
                TaskListOptions {
                    statuses: Vec::new(),
                    priorities: Vec::new(),
                    plan_filters: Vec::new(),
                    assignee: None,
                    query: None,
                    include_archived: false,
                    limit: 1_001,
                    offset: 0,
                    sort: crate::TaskListSort::default(),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, KanbanError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn get_task_accepts_only_global_task_ids() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let task = service.get_task(" t_show ").await.unwrap();
        assert_eq!(task.id, "t_show");

        let error = service.get_task("default#1").await.unwrap_err();
        assert!(matches!(error, KanbanError::InvalidInput(_)));
    }

    fn task_record(input: CreateTaskRecord) -> TaskRecord {
        TaskRecord {
            id: input.id,
            board_id: "b_default".into(),
            board_slug: "default".into(),
            task_ref: "default#1".into(),
            seq: 1,
            title: input.title,
            description: input.description,
            status: input.status,
            status_reason: None,
            assignee: input.assignee,
            priority: input.priority,
            position: 1024,
            scheduled_at: input.scheduled_at,
            due_at: input.due_at,
            created_by: input.created_by,
            created_at: 100,
            updated_at: 100,
            started_at: None,
            completed_at: None,
            archived_at: None,
            claim_owner: None,
            claim_expires_at: None,
            last_heartbeat_at: None,
            current_run_id: None,
            retry_count: 0,
            max_retries: input.max_retries,
            result_summary: None,
            result_json: None,
            metadata_json: input.metadata_json,
            lock_version: 0,
            dependency_blocked: false,
            unfinished_parent_count: 0,
            execution_plan_state: ExecutionPlanState::Unplanned,
            required_step_count: 0,
            completed_required_step_count: 0,
            optional_step_count: 0,
        }
    }
}
