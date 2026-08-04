use std::sync::Arc;

use kanban_core::{
    Clock, KanbanError, ReadinessFacts, Result, SystemClock, TaskStatus, can_promote_from,
    initial_status, is_claimable_task, new_event_id, new_run_id, new_typed_id,
    recompute_ready_status, running_claim_is_present,
};
use tokio::sync::Mutex;

use crate::{
    ApplicationHealth, ApplicationStore, BoardColumnRecord, BoardRecord, ClaimRecord,
    ClaimTaskCommand, ClaimTaskRecord, CreateTaskCommand, CreateTaskRecord, ExecutionPlanRecord,
    ExecutionPlanState, HeartbeatTaskCommand, HeartbeatTaskRecord,
    MarkExecutionPlanNotRequiredCommand, MarkExecutionPlanNotRequiredRecord, PromoteTaskCommand,
    PromoteTaskRecord, ReleaseTaskCommand, ReleaseTaskRecord, TaskListOptions, TaskListPage,
    TaskRecord,
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
    mutation_gate: Arc<Mutex<()>>,
}

impl<S> ApplicationService<S, SystemClock>
where
    S: ApplicationStore,
{
    pub fn new(store: S) -> Self {
        Self {
            store,
            clock: SystemClock,
            mutation_gate: Arc::new(Mutex::new(())),
        }
    }
}

impl<S, C> ApplicationService<S, C>
where
    S: ApplicationStore,
    C: Clock,
{
    pub fn with_clock(store: S, clock: C) -> Self {
        Self {
            store,
            clock,
            mutation_gate: Arc::new(Mutex::new(())),
        }
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

    pub async fn mark_execution_plan_not_required(
        &self,
        command: MarkExecutionPlanNotRequiredCommand,
    ) -> Result<ExecutionPlanRecord> {
        let task_id = command.task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        let reason = command.reason.trim();
        if reason.is_empty() {
            return Err(KanbanError::InvalidInput(
                "execution plan not_required reason is required".to_owned(),
            ));
        }
        let actor = command.actor.trim();
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("actor is required".to_owned()));
        }
        let _mutation = self.mutation_gate.lock().await;
        self.store
            .mark_execution_plan_not_required(
                task_id,
                MarkExecutionPlanNotRequiredRecord {
                    reason: reason.to_owned(),
                    actor: actor.to_owned(),
                    event_id: new_event_id(),
                    updated_at: self.clock.now_ms(),
                },
            )
            .await
    }

    pub async fn promote_task(&self, command: PromoteTaskCommand) -> Result<TaskRecord> {
        let task_id = command.task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        let actor = command.actor.trim();
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("actor is required".to_owned()));
        }
        let _mutation = self.mutation_gate.lock().await;
        let task = self.store.get_task(task_id).await?;
        if !can_promote_from(task.status) {
            return Err(KanbanError::InvalidTransition(format!(
                "cannot promote from {}",
                task.status.as_str()
            )));
        }
        let now = self.clock.now_ms();
        let target = recompute_ready_status(
            ReadinessFacts {
                title: &task.title,
                description: task.description.as_deref(),
                scheduled_at: task.scheduled_at,
                dependencies_done: !task.dependency_blocked,
            },
            now,
        );
        if target != TaskStatus::Ready {
            return Err(KanbanError::InvalidTransition(match target {
                TaskStatus::Todo => "dependency blocked".to_owned(),
                TaskStatus::Scheduled => "scheduled_at is in the future".to_owned(),
                TaskStatus::Triage => "task spec is incomplete".to_owned(),
                _ => format!("cannot promote to {}", target.as_str()),
            }));
        }
        if task.execution_plan_state == crate::ExecutionPlanState::Unplanned {
            return Err(KanbanError::ExecutionPlanRequired(
                "add steps or mark execution plan not_required before promoting task".to_owned(),
            ));
        }
        self.store
            .promote_task(
                task_id,
                PromoteTaskRecord {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    event_id: new_event_id(),
                    updated_at: now,
                },
            )
            .await
    }

    pub async fn claim_task(&self, command: ClaimTaskCommand) -> Result<ClaimRecord> {
        let task_id = command.task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        let actor = command.actor.trim();
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("actor is required".to_owned()));
        }
        if command.ttl_ms <= 0 {
            return Err(KanbanError::InvalidInput(
                "ttl_ms must be positive".to_owned(),
            ));
        }
        let worker_profile = command
            .worker_profile
            .unwrap_or_else(|| "manual".to_owned());
        let metadata_json = serde_json::to_string(&command.metadata)
            .map_err(|error| KanbanError::InvalidInput(format!("invalid metadata: {error}")))?;
        let _mutation = self.mutation_gate.lock().await;
        let task = self.store.get_task(task_id).await?;
        if !is_claimable_task(task.status, task.has_claim_token) {
            let message = if task.has_claim_token {
                "claim conflict: task is already claimed"
            } else {
                "task is not claimable"
            };
            return Err(KanbanError::InvalidTransition(message.to_owned()));
        }
        if task.dependency_blocked {
            return Err(KanbanError::InvalidTransition(
                "dependency blocked".to_owned(),
            ));
        }
        if task.execution_plan_state == ExecutionPlanState::Unplanned {
            return Err(KanbanError::ExecutionPlanRequired(
                "add steps or mark execution plan not_required before claiming task".to_owned(),
            ));
        }
        let now = self.clock.now_ms();
        let claim_expires_at = now.checked_add(command.ttl_ms).ok_or_else(|| {
            KanbanError::InvalidInput("ttl_ms produces an invalid claim expiry".to_owned())
        })?;
        self.store
            .claim_task(
                task_id,
                ClaimTaskRecord {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    claim_token: new_typed_id("claim"),
                    run_id: new_run_id(),
                    event_id: new_event_id(),
                    worker_profile,
                    metadata_json,
                    now,
                    claim_expires_at,
                },
            )
            .await
    }

    pub async fn heartbeat_task(&self, command: HeartbeatTaskCommand) -> Result<TaskRecord> {
        let task_id = command.task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        let actor = command.actor.trim();
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("actor is required".to_owned()));
        }
        if command.claim_token.trim().is_empty() {
            return Err(KanbanError::InvalidInput(
                "claim_token is required".to_owned(),
            ));
        }
        if command.ttl_ms <= 0 {
            return Err(KanbanError::InvalidInput(
                "ttl_ms must be positive".to_owned(),
            ));
        }
        let _mutation = self.mutation_gate.lock().await;
        let task = self.store.get_task(task_id).await?;
        if !running_claim_is_present(
            task.status,
            task.has_claim_token,
            task.current_run_id.is_some(),
        ) {
            return Err(KanbanError::InvalidTransition(
                "heartbeat requires an active running claim".to_owned(),
            ));
        }
        if task.claim_owner.as_deref() != Some(actor) {
            return Err(KanbanError::InvalidTransition(
                "claim owner mismatch".to_owned(),
            ));
        }
        let now = self.clock.now_ms();
        let claim_expires_at = now.checked_add(command.ttl_ms).ok_or_else(|| {
            KanbanError::InvalidInput("ttl_ms produces an invalid claim expiry".to_owned())
        })?;
        self.store
            .heartbeat_task(
                task_id,
                HeartbeatTaskRecord {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    claim_token: command.claim_token,
                    event_id: new_event_id(),
                    note: command.note,
                    now,
                    claim_expires_at,
                },
            )
            .await
    }

    pub async fn release_task(&self, command: ReleaseTaskCommand) -> Result<TaskRecord> {
        let task_id = command.task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        let actor = command.actor.trim();
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("actor is required".to_owned()));
        }
        if command.claim_token.trim().is_empty() {
            return Err(KanbanError::InvalidInput(
                "claim_token is required".to_owned(),
            ));
        }
        let _mutation = self.mutation_gate.lock().await;
        let task = self.store.get_task(task_id).await?;
        if !running_claim_is_present(
            task.status,
            task.has_claim_token,
            task.current_run_id.is_some(),
        ) {
            return Err(KanbanError::InvalidTransition(
                "release requires an active running claim".to_owned(),
            ));
        }
        if task.claim_owner.as_deref() != Some(actor) {
            return Err(KanbanError::InvalidTransition(
                "claim owner mismatch".to_owned(),
            ));
        }
        if task.execution_plan_state == ExecutionPlanState::Unplanned {
            return Err(KanbanError::ExecutionPlanRequired(
                "add steps or mark execution plan not_required before releasing task".to_owned(),
            ));
        }
        let now = self.clock.now_ms();
        let target = recompute_ready_status(
            ReadinessFacts {
                title: &task.title,
                description: task.description.as_deref(),
                scheduled_at: task.scheduled_at,
                dependencies_done: !task.dependency_blocked,
            },
            now,
        );
        if target != TaskStatus::Ready {
            return Err(KanbanError::InvalidTransition(match target {
                TaskStatus::Todo => "dependency blocked".to_owned(),
                TaskStatus::Scheduled => "scheduled_at is in the future".to_owned(),
                TaskStatus::Triage => "task spec is incomplete".to_owned(),
                _ => format!("cannot release to {}", target.as_str()),
            }));
        }
        self.store
            .release_task(
                task_id,
                ReleaseTaskRecord {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    claim_token: command.claim_token,
                    event_id: new_event_id(),
                    now,
                },
            )
            .await
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
    use crate::{ExecutionPlanState, RunRecord, RunStatus};

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
            Ok(task_for_id(task_id))
        }

        async fn mark_execution_plan_not_required(
            &self,
            task_id: &str,
            input: MarkExecutionPlanNotRequiredRecord,
        ) -> Result<ExecutionPlanRecord> {
            assert_eq!(task_id, "t_show");
            assert_eq!(input.reason, "small task");
            assert_eq!(input.actor, "tester");
            assert!(input.event_id.starts_with("e_"));
            assert_eq!(input.updated_at, 100);
            Ok(ExecutionPlanRecord {
                board_id: "b_default".into(),
                task_id: task_id.to_owned(),
                state: ExecutionPlanState::NotRequired,
                reason: Some(input.reason),
                updated_by: input.actor,
                updated_at: input.updated_at,
            })
        }

        async fn promote_task(
            &self,
            task_id: &str,
            input: PromoteTaskRecord,
        ) -> Result<TaskRecord> {
            assert_eq!(task_id, "t_promote");
            assert_eq!(input.expected_lock_version, 0);
            assert_eq!(input.actor, "promoter");
            assert!(input.event_id.starts_with("e_"));
            assert_eq!(input.updated_at, 100);
            let mut task = task_for_id(task_id);
            task.status = TaskStatus::Ready;
            task.lock_version += 1;
            task.updated_at = input.updated_at;
            Ok(task)
        }

        async fn claim_task(&self, task_id: &str, input: ClaimTaskRecord) -> Result<ClaimRecord> {
            assert_eq!(task_id, "t_claim");
            assert_eq!(input.expected_lock_version, 0);
            assert_eq!(input.actor, "worker");
            assert!(input.claim_token.starts_with("claim_"));
            assert!(input.run_id.starts_with("r_"));
            assert!(input.event_id.starts_with("e_"));
            assert_eq!(input.worker_profile, "manual");
            assert_eq!(input.metadata_json, r#"{"source":"test"}"#);
            assert_eq!(input.now, 100);
            assert_eq!(input.claim_expires_at, 400);
            let claim_expires_at = input.claim_expires_at;
            let mut task = task_for_id(task_id);
            task.status = TaskStatus::Running;
            task.has_claim_token = true;
            task.claim_owner = Some(input.actor.clone());
            task.claim_expires_at = Some(claim_expires_at);
            task.last_heartbeat_at = Some(input.now);
            task.current_run_id = Some(input.run_id.clone());
            task.started_at = Some(input.now);
            task.updated_at = input.now;
            task.lock_version += 1;
            Ok(ClaimRecord {
                task,
                run: RunRecord {
                    id: input.run_id,
                    board_id: "b_default".into(),
                    task_id: task_id.to_owned(),
                    status: RunStatus::Running,
                    worker_profile: Some(input.worker_profile),
                    worker_pid: None,
                    claim_owner: input.actor,
                    claim_expires_at,
                    started_at: input.now,
                    last_heartbeat_at: Some(input.now),
                    finished_at: None,
                    exit_code: None,
                    summary: None,
                    error: None,
                    log_path: None,
                    metadata_json: input.metadata_json,
                },
                claim_token: input.claim_token,
                claim_expires_at,
            })
        }

        async fn heartbeat_task(
            &self,
            task_id: &str,
            input: HeartbeatTaskRecord,
        ) -> Result<TaskRecord> {
            assert_eq!(task_id, "t_heartbeat");
            assert_eq!(input.expected_lock_version, 2);
            assert_eq!(input.actor, "worker");
            assert!(input.event_id.starts_with("e_"));
            assert_eq!(input.now, 100);
            assert_eq!(input.claim_expires_at, 400);
            if input.claim_token != "claim_valid" {
                return Err(KanbanError::InvalidTransition(
                    "claim token mismatch".to_owned(),
                ));
            }
            assert_eq!(input.note.as_deref(), Some(" alive "));
            let mut task = task_for_id(task_id);
            task.claim_expires_at = Some(input.claim_expires_at);
            task.last_heartbeat_at = Some(input.now);
            task.updated_at = input.now;
            task.lock_version += 1;
            Ok(task)
        }

        async fn release_task(
            &self,
            task_id: &str,
            input: ReleaseTaskRecord,
        ) -> Result<TaskRecord> {
            assert_eq!(task_id, "t_release");
            assert_eq!(input.expected_lock_version, 2);
            assert_eq!(input.actor, "worker");
            assert!(input.event_id.starts_with("e_"));
            assert_eq!(input.now, 100);
            if input.claim_token != "claim_valid" {
                return Err(KanbanError::InvalidTransition(
                    "claim token mismatch".to_owned(),
                ));
            }
            let mut task = task_for_id(task_id);
            task.status = TaskStatus::Ready;
            task.has_claim_token = false;
            task.claim_owner = None;
            task.claim_expires_at = None;
            task.last_heartbeat_at = None;
            task.current_run_id = None;
            task.updated_at = input.now;
            task.lock_version += 1;
            Ok(task)
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

    #[tokio::test]
    async fn mark_execution_plan_not_required_canonicalizes_command() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let plan = service
            .mark_execution_plan_not_required(MarkExecutionPlanNotRequiredCommand {
                task_id: " t_show ".into(),
                reason: " small task ".into(),
                actor: " tester ".into(),
            })
            .await
            .unwrap();
        assert_eq!(plan.state, ExecutionPlanState::NotRequired);
        assert_eq!(plan.reason.as_deref(), Some("small task"));

        for command in [
            MarkExecutionPlanNotRequiredCommand {
                task_id: "default#1".into(),
                reason: "small".into(),
                actor: "tester".into(),
            },
            MarkExecutionPlanNotRequiredCommand {
                task_id: "t_show".into(),
                reason: " ".into(),
                actor: "tester".into(),
            },
            MarkExecutionPlanNotRequiredCommand {
                task_id: "t_show".into(),
                reason: "small".into(),
                actor: " ".into(),
            },
        ] {
            assert!(matches!(
                service.mark_execution_plan_not_required(command).await,
                Err(KanbanError::InvalidInput(_))
            ));
        }
    }

    #[tokio::test]
    async fn promote_task_uses_core_readiness_and_plan_guards() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let promoted = service
            .promote_task(PromoteTaskCommand {
                task_id: "t_promote".into(),
                actor: " promoter ".into(),
            })
            .await
            .unwrap();
        assert_eq!(promoted.status, TaskStatus::Ready);
        assert_eq!(promoted.lock_version, 1);

        let unplanned = service
            .promote_task(PromoteTaskCommand {
                task_id: "t_unplanned".into(),
                actor: "promoter".into(),
            })
            .await
            .unwrap_err();
        assert!(matches!(unplanned, KanbanError::ExecutionPlanRequired(_)));

        let future = service
            .promote_task(PromoteTaskCommand {
                task_id: "t_future".into(),
                actor: "promoter".into(),
            })
            .await
            .unwrap_err();
        assert!(matches!(future, KanbanError::InvalidTransition(_)));

        let running = service
            .promote_task(PromoteTaskCommand {
                task_id: "t_running".into(),
                actor: "promoter".into(),
            })
            .await
            .unwrap_err();
        assert!(matches!(running, KanbanError::InvalidTransition(_)));
    }

    #[tokio::test]
    async fn claim_task_uses_core_guard_and_canonicalizes_lease_input() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let claim = service
            .claim_task(ClaimTaskCommand {
                task_id: " t_claim ".into(),
                actor: " worker ".into(),
                ttl_ms: 300,
                worker_profile: None,
                metadata: serde_json::json!({"source": "test"}),
            })
            .await
            .unwrap();
        assert_eq!(claim.task.status, TaskStatus::Running);
        assert_eq!(claim.run.status, RunStatus::Running);
        assert!(claim.claim_token.starts_with("claim_"));
        assert_eq!(claim.claim_expires_at, 400);

        let claimed = service
            .claim_task(ClaimTaskCommand {
                task_id: "t_claimed".into(),
                actor: "worker".into(),
                ttl_ms: 300,
                worker_profile: None,
                metadata: serde_json::json!({}),
            })
            .await
            .unwrap_err();
        assert!(matches!(
            claimed,
            KanbanError::InvalidTransition(message) if message.contains("claim conflict")
        ));
        let dependency = service
            .claim_task(ClaimTaskCommand {
                task_id: "t_claim_dependency".into(),
                actor: "worker".into(),
                ttl_ms: 300,
                worker_profile: None,
                metadata: serde_json::json!({}),
            })
            .await
            .unwrap_err();
        assert!(matches!(
            dependency,
            KanbanError::InvalidTransition(message) if message.contains("dependency blocked")
        ));
        let unplanned = service
            .claim_task(ClaimTaskCommand {
                task_id: "t_claim_unplanned".into(),
                actor: "worker".into(),
                ttl_ms: 300,
                worker_profile: None,
                metadata: serde_json::json!({}),
            })
            .await
            .unwrap_err();
        assert!(matches!(unplanned, KanbanError::ExecutionPlanRequired(_)));
    }

    #[tokio::test]
    async fn claim_task_rejects_invalid_identity_and_lease() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(i64::MAX - 10),
        );
        for command in [
            ClaimTaskCommand {
                task_id: "default#1".into(),
                actor: "worker".into(),
                ttl_ms: 300,
                worker_profile: None,
                metadata: serde_json::json!({}),
            },
            ClaimTaskCommand {
                task_id: "t_claim".into(),
                actor: " ".into(),
                ttl_ms: 300,
                worker_profile: None,
                metadata: serde_json::json!({}),
            },
            ClaimTaskCommand {
                task_id: "t_claim".into(),
                actor: "worker".into(),
                ttl_ms: 0,
                worker_profile: None,
                metadata: serde_json::json!({}),
            },
            ClaimTaskCommand {
                task_id: "t_claim".into(),
                actor: "worker".into(),
                ttl_ms: 20,
                worker_profile: None,
                metadata: serde_json::json!({}),
            },
        ] {
            assert!(matches!(
                service.claim_task(command).await,
                Err(KanbanError::InvalidInput(_))
            ));
        }
    }

    #[tokio::test]
    async fn heartbeat_task_validates_running_claim_owner_and_lease() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let heartbeat = service
            .heartbeat_task(HeartbeatTaskCommand {
                task_id: " t_heartbeat ".into(),
                actor: " worker ".into(),
                claim_token: "claim_valid".into(),
                ttl_ms: 300,
                note: Some(" alive ".into()),
            })
            .await
            .unwrap();
        assert_eq!(heartbeat.status, TaskStatus::Running);
        assert_eq!(heartbeat.claim_expires_at, Some(400));
        assert_eq!(heartbeat.last_heartbeat_at, Some(100));
        assert_eq!(heartbeat.lock_version, 3);

        let padded_token = service
            .heartbeat_task(HeartbeatTaskCommand {
                task_id: "t_heartbeat".into(),
                actor: "worker".into(),
                claim_token: " claim_valid ".into(),
                ttl_ms: 300,
                note: None,
            })
            .await
            .unwrap_err();
        assert!(matches!(
            padded_token,
            KanbanError::InvalidTransition(message) if message.contains("claim token mismatch")
        ));

        let wrong_token = service
            .heartbeat_task(HeartbeatTaskCommand {
                task_id: "t_heartbeat".into(),
                actor: "worker".into(),
                claim_token: "wrong".into(),
                ttl_ms: 300,
                note: None,
            })
            .await
            .unwrap_err();
        assert!(matches!(
            wrong_token,
            KanbanError::InvalidTransition(message) if message.contains("claim token mismatch")
        ));

        let wrong_owner = service
            .heartbeat_task(HeartbeatTaskCommand {
                task_id: "t_heartbeat".into(),
                actor: "other".into(),
                claim_token: "claim_valid".into(),
                ttl_ms: 300,
                note: None,
            })
            .await
            .unwrap_err();
        assert!(matches!(
            wrong_owner,
            KanbanError::InvalidTransition(message) if message.contains("claim owner mismatch")
        ));

        let inactive = service
            .heartbeat_task(HeartbeatTaskCommand {
                task_id: "t_claim".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
                ttl_ms: 300,
                note: None,
            })
            .await
            .unwrap_err();
        assert!(matches!(inactive, KanbanError::InvalidTransition(_)));
    }

    #[tokio::test]
    async fn heartbeat_task_rejects_invalid_identity_token_and_lease() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(i64::MAX - 10),
        );
        for command in [
            HeartbeatTaskCommand {
                task_id: "default#1".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
                ttl_ms: 300,
                note: None,
            },
            HeartbeatTaskCommand {
                task_id: "t_heartbeat".into(),
                actor: " ".into(),
                claim_token: "claim_valid".into(),
                ttl_ms: 300,
                note: None,
            },
            HeartbeatTaskCommand {
                task_id: "t_heartbeat".into(),
                actor: "worker".into(),
                claim_token: " ".into(),
                ttl_ms: 300,
                note: None,
            },
            HeartbeatTaskCommand {
                task_id: "t_heartbeat".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
                ttl_ms: 0,
                note: None,
            },
            HeartbeatTaskCommand {
                task_id: "t_heartbeat".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
                ttl_ms: 20,
                note: None,
            },
        ] {
            assert!(matches!(
                service.heartbeat_task(command).await,
                Err(KanbanError::InvalidInput(_))
            ));
        }
    }

    #[tokio::test]
    async fn release_task_validates_readiness_owner_and_exact_token() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let released = service
            .release_task(ReleaseTaskCommand {
                task_id: " t_release ".into(),
                actor: " worker ".into(),
                claim_token: "claim_valid".into(),
            })
            .await
            .unwrap();
        assert_eq!(released.status, TaskStatus::Ready);
        assert!(!released.has_claim_token);
        assert_eq!(released.claim_owner, None);
        assert_eq!(released.claim_expires_at, None);
        assert_eq!(released.last_heartbeat_at, None);
        assert_eq!(released.current_run_id, None);
        assert_eq!(released.lock_version, 3);

        let padded_token = service
            .release_task(ReleaseTaskCommand {
                task_id: "t_release".into(),
                actor: "worker".into(),
                claim_token: " claim_valid ".into(),
            })
            .await
            .unwrap_err();
        assert!(matches!(
            padded_token,
            KanbanError::InvalidTransition(message) if message.contains("claim token mismatch")
        ));

        for command in [
            ReleaseTaskCommand {
                task_id: "t_release".into(),
                actor: "other".into(),
                claim_token: "claim_valid".into(),
            },
            ReleaseTaskCommand {
                task_id: "t_claim".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
            },
            ReleaseTaskCommand {
                task_id: "t_release_dependency".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
            },
            ReleaseTaskCommand {
                task_id: "t_release_future".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
            },
        ] {
            assert!(matches!(
                service.release_task(command).await,
                Err(KanbanError::InvalidTransition(_))
            ));
        }

        let unplanned = service
            .release_task(ReleaseTaskCommand {
                task_id: "t_release_unplanned".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
            })
            .await
            .unwrap_err();
        assert!(matches!(unplanned, KanbanError::ExecutionPlanRequired(_)));
    }

    #[tokio::test]
    async fn release_task_rejects_invalid_identity_and_token() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        for command in [
            ReleaseTaskCommand {
                task_id: "default#1".into(),
                actor: "worker".into(),
                claim_token: "claim_valid".into(),
            },
            ReleaseTaskCommand {
                task_id: "t_release".into(),
                actor: " ".into(),
                claim_token: "claim_valid".into(),
            },
            ReleaseTaskCommand {
                task_id: "t_release".into(),
                actor: "worker".into(),
                claim_token: " ".into(),
            },
        ] {
            assert!(matches!(
                service.release_task(command).await,
                Err(KanbanError::InvalidInput(_))
            ));
        }
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
            has_claim_token: false,
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

    fn task_for_id(task_id: &str) -> TaskRecord {
        let mut task = task_record(CreateTaskRecord {
            id: task_id.to_owned(),
            idempotency_key: None,
            title: "Promote".into(),
            description: Some("ready spec".into()),
            status: TaskStatus::Todo,
            assignee: None,
            priority: 1,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
            created_by: "tester".into(),
        });
        match task_id {
            "t_promote" => task.execution_plan_state = ExecutionPlanState::NotRequired,
            "t_claim" => {
                task.status = TaskStatus::Ready;
                task.execution_plan_state = ExecutionPlanState::NotRequired;
            }
            "t_claimed" => {
                task.status = TaskStatus::Ready;
                task.execution_plan_state = ExecutionPlanState::NotRequired;
                task.has_claim_token = true;
            }
            "t_claim_dependency" => {
                task.status = TaskStatus::Ready;
                task.execution_plan_state = ExecutionPlanState::NotRequired;
                task.dependency_blocked = true;
                task.unfinished_parent_count = 1;
            }
            "t_claim_unplanned" => task.status = TaskStatus::Ready,
            "t_heartbeat" => {
                task.status = TaskStatus::Running;
                task.execution_plan_state = ExecutionPlanState::NotRequired;
                task.has_claim_token = true;
                task.claim_owner = Some("worker".into());
                task.claim_expires_at = Some(200);
                task.last_heartbeat_at = Some(100);
                task.current_run_id = Some("r_heartbeat".into());
                task.started_at = Some(50);
                task.lock_version = 2;
            }
            "t_release" | "t_release_unplanned" | "t_release_dependency" | "t_release_future" => {
                task.status = TaskStatus::Running;
                task.execution_plan_state = if task_id == "t_release_unplanned" {
                    ExecutionPlanState::Unplanned
                } else {
                    ExecutionPlanState::NotRequired
                };
                task.has_claim_token = true;
                task.claim_owner = Some("worker".into());
                task.claim_expires_at = Some(200);
                task.last_heartbeat_at = Some(100);
                task.current_run_id = Some("r_release".into());
                task.started_at = Some(50);
                task.lock_version = 2;
                if task_id == "t_release_dependency" {
                    task.dependency_blocked = true;
                    task.unfinished_parent_count = 1;
                }
                if task_id == "t_release_future" {
                    task.scheduled_at = Some(200);
                }
            }
            "t_future" => {
                task.status = TaskStatus::Scheduled;
                task.scheduled_at = Some(200);
                task.execution_plan_state = ExecutionPlanState::NotRequired;
            }
            "t_running" => {
                task.status = TaskStatus::Running;
                task.execution_plan_state = ExecutionPlanState::NotRequired;
                task.has_claim_token = true;
                task.claim_owner = Some("worker".into());
                task.claim_expires_at = Some(200);
                task.current_run_id = Some("r_running".into());
            }
            _ => {}
        }
        task
    }
}
