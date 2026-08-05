mod schema;

use std::{
    collections::HashSet,
    error::Error,
    fmt::{Display, Formatter},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use turso::{
    Builder, Connection, Database, Row, Rows, Value,
    transaction::{Transaction, TransactionBehavior},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardRecord {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardColumnRecord {
    pub id: String,
    pub board_id: String,
    pub status: String,
    pub title: String,
    pub position: i64,
    pub hidden: bool,
    pub wip_limit: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTaskInput {
    pub id: String,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub status: String,
    pub description: Option<String>,
    pub assignee: Option<String>,
    pub priority: i64,
    pub scheduled_at: Option<i64>,
    pub due_at: Option<i64>,
    pub max_retries: Option<i64>,
    pub metadata_json: String,
    pub created_by: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateCommentInput {
    pub id: String,
    pub idempotency_key: Option<String>,
    pub author: String,
    pub author_type: String,
    pub agent_type: Option<String>,
    pub body: String,
    pub kind: String,
    pub metadata_json: String,
    pub event_id: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddDependencyInput {
    pub expected_child_lock_version: i64,
    pub target_child_status: String,
    pub actor: String,
    pub event_id: String,
    pub recompute_event_id: String,
    pub now: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyEdgeRecord {
    pub parent: TaskRecord,
    pub child: TaskRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencySnapshotRecord {
    pub task: TaskRecord,
    pub parents: Vec<TaskRecord>,
    pub children: Vec<TaskRecord>,
    pub edges: Vec<DependencyEdgeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddDependencyRecord {
    pub added: bool,
    pub dependencies: DependencySnapshotRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveDependencyInput {
    pub actor: String,
    pub event_id: String,
    pub now: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveDependencyRecord {
    pub removed: bool,
    pub dependencies: DependencySnapshotRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskListSort {
    Seq,
    SeqDesc,
    Title,
    TitleDesc,
    Status,
    StatusDesc,
    Position,
    PositionDesc,
    Priority,
    PriorityDesc,
    Assignee,
    AssigneeDesc,
    ScheduledAt,
    ScheduledAtDesc,
    CreatedAt,
    CreatedAtDesc,
    UpdatedAt,
    UpdatedAtDesc,
    DueAt,
    DueAtDesc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPlanFilter {
    PlanNeeded,
    HasSteps,
    IncompleteRequiredSteps,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListOptions {
    pub statuses: Vec<String>,
    pub priorities: Vec<i64>,
    pub include_archived: bool,
    pub assignee: Option<String>,
    pub q: Option<String>,
    pub plan_filters: Vec<TaskPlanFilter>,
    pub sort: TaskListSort,
    pub limit: usize,
    pub offset: usize,
}

impl Default for TaskListOptions {
    fn default() -> Self {
        Self {
            statuses: Vec::new(),
            priorities: Vec::new(),
            include_archived: false,
            assignee: None,
            q: None,
            plan_filters: Vec::new(),
            sort: TaskListSort::Position,
            limit: 100,
            offset: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListPage {
    pub tasks: Vec<TaskRecord>,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentRecord {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub idempotency_key: Option<String>,
    pub author: String,
    pub author_type: String,
    pub agent_type: Option<String>,
    pub body: String,
    pub kind: String,
    pub metadata_json: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateStepInput {
    pub id: String,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub body: Option<String>,
    pub linked_task_id: Option<String>,
    pub position: Option<i64>,
    pub required: bool,
    pub created_by: String,
    pub event_id: String,
    pub plan_event_id: String,
    pub recompute_event_id: String,
    pub created_at: i64,
    pub expected_lock_version: i64,
    pub expected_plan_state: String,
    pub target_status: String,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStepRecord {
    pub id: String,
    pub board_id: String,
    pub parent_task_id: String,
    pub title: String,
    pub body: Option<String>,
    pub linked_task: Option<TaskRecord>,
    pub position: i64,
    pub required: bool,
    pub status: String,
    pub resolution_note: Option<String>,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<i64>,
    pub created_by: String,
    pub created_at: i64,
    pub updated_by: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStepsRecord {
    pub task_id: String,
    pub steps: Vec<TaskStepRecord>,
    pub execution_plan: TaskExecutionPlanRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkExecutionPlanNotRequiredInput {
    pub reason: String,
    pub actor: String,
    pub event_id: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskExecutionPlanRecord {
    pub board_id: String,
    pub task_id: String,
    pub state: String,
    pub reason: Option<String>,
    pub updated_by: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromoteTaskInput {
    pub expected_lock_version: i64,
    pub actor: String,
    pub event_id: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimTaskInput {
    pub expected_lock_version: i64,
    pub owner: String,
    pub claim_token: String,
    pub run_id: String,
    pub event_id: String,
    pub worker_profile: String,
    pub metadata_json: String,
    pub log_path: Option<String>,
    pub now: i64,
    pub claim_expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRunRecord {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub status: String,
    pub worker_profile: Option<String>,
    pub worker_pid: Option<i64>,
    pub claim_token: String,
    pub claim_owner: String,
    pub claim_expires_at: i64,
    pub started_at: i64,
    pub last_heartbeat_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub exit_code: Option<i64>,
    pub summary: Option<String>,
    pub error: Option<String>,
    pub log_path: Option<String>,
    pub metadata_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimTaskRecord {
    pub task: TaskRecord,
    pub run: TaskRunRecord,
    pub claim_token: String,
    pub claim_expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatTaskInput {
    pub expected_lock_version: i64,
    pub actor: String,
    pub claim_token: String,
    pub event_id: String,
    pub note: Option<String>,
    pub now: i64,
    pub claim_expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseTaskInput {
    pub expected_lock_version: i64,
    pub actor: String,
    pub claim_token: String,
    pub event_id: String,
    pub now: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReclaimExpiredTaskInput {
    pub expected_lock_version: i64,
    pub actor: String,
    pub event_id: String,
    pub target_status: String,
    pub retry_count: i64,
    pub reason: String,
    pub now: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitReviewTaskInput {
    pub expected_lock_version: i64,
    pub actor: String,
    pub claim_token: Option<String>,
    pub force: bool,
    pub summary: Option<String>,
    pub now: i64,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteTaskInput {
    pub expected_lock_version: i64,
    pub actor: String,
    pub claim_token: Option<String>,
    pub force: bool,
    pub summary: Option<String>,
    pub result_json: Option<String>,
    pub now: i64,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTaskInput {
    pub expected_lock_version: i64,
    pub actor: String,
    pub claim_token: Option<String>,
    pub force: bool,
    pub reason: String,
    pub now: i64,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRecord {
    pub id: String,
    pub board_id: String,
    pub board_slug: String,
    pub task_ref: String,
    pub seq: i64,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub status_reason: Option<String>,
    pub assignee: Option<String>,
    pub priority: i64,
    pub position: i64,
    pub scheduled_at: Option<i64>,
    pub due_at: Option<i64>,
    pub created_by: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub archived_at: Option<i64>,
    pub claim_token: Option<String>,
    pub claim_owner: Option<String>,
    pub claim_expires_at: Option<i64>,
    pub last_heartbeat_at: Option<i64>,
    pub current_run_id: Option<String>,
    pub retry_count: i64,
    pub max_retries: Option<i64>,
    pub result_summary: Option<String>,
    pub result_json: Option<String>,
    pub metadata_json: String,
    pub lock_version: i64,
    pub dependency_blocked: bool,
    pub unfinished_parent_count: i64,
    pub execution_plan_state: String,
    pub required_step_count: i64,
    pub completed_required_step_count: i64,
    pub optional_step_count: i64,
    pub labels: Vec<String>,
}

#[derive(Debug)]
pub enum StoreError {
    Turso(turso::Error),
    InvalidPath,
    InvalidInput(String),
    InvalidTransition(String),
    StepsIncomplete(String),
    ClaimConflict(String),
    ClaimTokenMismatch,
    InvalidStoredValue {
        field: &'static str,
    },
    BoardNotFound(String),
    TaskNotFound(String),
    StepNotFound(String),
    DependencyCycle(String),
    IdempotencyConflict {
        board_id: String,
        key: String,
        existing_task_id: String,
    },
}

impl Display for StoreError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Turso(error) => write!(formatter, "turso error: {error}"),
            Self::InvalidPath => write!(formatter, "database path must be valid non-empty UTF-8"),
            Self::InvalidInput(message) => write!(formatter, "invalid task input: {message}"),
            Self::InvalidTransition(message) => {
                write!(formatter, "invalid task transition: {message}")
            }
            Self::StepsIncomplete(message) => write!(formatter, "steps incomplete: {message}"),
            Self::ClaimConflict(message) => write!(formatter, "claim conflict: {message}"),
            Self::ClaimTokenMismatch => write!(formatter, "claim token mismatch"),
            Self::InvalidStoredValue { field } => {
                write!(formatter, "invalid stored value for {field}")
            }
            Self::BoardNotFound(selector) => write!(formatter, "board not found: {selector}"),
            Self::TaskNotFound(task_id) => write!(formatter, "task not found: {task_id}"),
            Self::StepNotFound(step_id) => write!(formatter, "step not found: {step_id}"),
            Self::DependencyCycle(message) => write!(formatter, "dependency cycle: {message}"),
            Self::IdempotencyConflict {
                board_id,
                key,
                existing_task_id,
            } => write!(
                formatter,
                "idempotency conflict for board {board_id}, key {key}, existing task {existing_task_id}"
            ),
        }
    }
}

impl Error for StoreError {}

impl From<turso::Error> for StoreError {
    fn from(error: turso::Error) -> Self {
        Self::Turso(error)
    }
}

#[derive(Clone)]
pub struct TursoStore {
    database: Database,
}

impl TursoStore {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_str().ok_or(StoreError::InvalidPath)?;
        if path.is_empty() {
            return Err(StoreError::InvalidPath);
        }
        let database = Builder::new_local(path).build().await?;
        Ok(Self { database })
    }

    pub async fn initialize(&self) -> Result<(), StoreError> {
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        transaction.execute_batch(schema::CANONICAL_SCHEMA).await?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at) VALUES (?1, ?2, '', ?3)",
                (schema::SCHEMA_VERSION, schema::SCHEMA_NAME, now_ms()),
            )
            .await?;

        transaction
            .execute(
                "INSERT OR IGNORE INTO boards(id, slug, name, description, created_at, updated_at, archived_at) VALUES ('b_default', 'default', 'Default', NULL, ?1, ?1, NULL)",
                [now_ms()],
            )
            .await?;
        let board_id = first_row(
            transaction
                .query("SELECT id FROM boards WHERE slug = 'default'", ())
                .await?,
        )
        .await?
        .get_value(0)
        .map_err(StoreError::from)
        .and_then(|value| text_value(value, "boards.id"))?;

        for (status, title, position, hidden) in schema::DEFAULT_COLUMNS {
            let id = format!("col_{}_{}", board_id.trim_start_matches("b_"), status);
            transaction
                .execute(
                    "INSERT OR IGNORE INTO board_columns(id, board_id, status, title, position, hidden, wip_limit, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?7)",
                    (id, board_id.as_str(), status, title, position, hidden, now_ms()),
                )
                .await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    pub async fn create_task(
        &self,
        board_selector: &str,
        input: CreateTaskInput,
    ) -> Result<TaskRecord, StoreError> {
        validate_create_task_input(&input)?;
        let title = input.title.trim().to_owned();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let board = first_row(
            transaction
                .query(
                    "SELECT id, slug FROM boards WHERE id = ?1 OR slug = ?1 LIMIT 1",
                    [board_selector],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => {
                StoreError::BoardNotFound(board_selector.to_owned())
            }
            other => StoreError::Turso(other),
        })?;
        let board_id = text_value(board.get_value(0)?, "boards.id")?;
        let board_slug = text_value(board.get_value(1)?, "boards.slug")?;

        if let Some(idempotency_key) = input.idempotency_key.as_deref() {
            let existing = first_row(
                transaction
                    .query(
                        &format!(
                            "{TASK_SELECT} WHERE t.board_id = ?1 AND t.idempotency_key = ?2 LIMIT 1"
                        ),
                        (board_id.as_str(), idempotency_key),
                    )
                    .await?,
            )
            .await;
            match existing {
                Ok(row) => {
                    let existing = task_from_row(row)?;
                    if canonical_payload_matches(&existing, &input, &title) {
                        transaction.commit().await?;
                        return Ok(existing);
                    }
                    return Err(StoreError::IdempotencyConflict {
                        board_id,
                        key: idempotency_key.to_owned(),
                        existing_task_id: existing.id,
                    });
                }
                Err(turso::Error::QueryReturnedNoRows) => {}
                Err(error) => return Err(StoreError::Turso(error)),
            }
        }

        let seq = first_row(
            transaction
                .query(
                    "SELECT COALESCE(MAX(seq), 0) + 1 FROM tasks WHERE board_id = ?1",
                    [board_id.as_str()],
                )
                .await?,
        )
        .await?
        .get_value(0)
        .map_err(StoreError::from)
        .and_then(|value| integer_value(value, "tasks.seq"))?;
        let position = seq
            .checked_mul(1024)
            .ok_or_else(|| StoreError::InvalidInput("task sequence is too large".to_owned()))?;
        let now = now_ms();
        transaction
            .execute(
                "INSERT INTO tasks(id, board_id, seq, idempotency_key, title, description, status, assignee, priority, position, scheduled_at, due_at, created_by, created_at, updated_at, max_retries, metadata_json, lock_version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14, ?15, ?16, 0)",
                (
                    input.id.as_str(),
                    board_id.as_str(),
                    seq,
                    input.idempotency_key.as_deref(),
                    title.as_str(),
                    input.description.as_deref(),
                    input.status.as_str(),
                    input.assignee.as_deref(),
                    input.priority,
                    position,
                    input.scheduled_at,
                    input.due_at,
                    input.created_by.as_str(),
                    now,
                    input.max_retries,
                    input.metadata_json.as_str(),
                ),
            )
            .await?;
        transaction
            .execute(
                "INSERT INTO task_execution_plans(board_id, task_id, state, reason, updated_by, updated_at) VALUES (?1, ?2, 'unplanned', NULL, ?3, ?4)",
                (board_id.as_str(), input.id.as_str(), input.created_by.as_str(), now),
            )
            .await?;
        let event_id = format!("e_{}_created", input.id.trim_start_matches("t_"));
        let event_payload = format!(r#"{{"status":"{}"}}"#, input.status);
        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, ?2, ?3, NULL, 'task.created', ?4, ?5, ?6)",
                (
                    event_id.as_str(),
                    board_id.as_str(),
                    input.id.as_str(),
                    input.created_by.as_str(),
                    event_payload.as_str(),
                    now,
                ),
            )
            .await?;
        let task = task_from_row(
            first_row(
                transaction
                    .query(
                        &format!("{TASK_SELECT} WHERE t.board_id = ?1 AND t.id = ?2 LIMIT 1"),
                        (board_id.as_str(), input.id.as_str()),
                    )
                    .await?,
            )
            .await?,
        )?;

        transaction.commit().await?;
        debug_assert_eq!(task.board_id, board_id);
        debug_assert_eq!(task.board_slug, board_slug);
        Ok(task)
    }

    pub async fn create_comment(
        &self,
        task_id: &str,
        input: CreateCommentInput,
    ) -> Result<CommentRecord, StoreError> {
        validate_create_comment_input(task_id, &input)?;
        let id = input.id.trim().to_owned();
        let idempotency_key = input
            .idempotency_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let author = input.author.trim().to_owned();
        let author_type = input.author_type.trim().to_owned();
        let agent_type = input
            .agent_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let body = input.body.trim().to_owned();
        let kind = input.kind.trim().to_owned();
        let metadata_json = input.metadata_json.trim();
        let metadata_json = if metadata_json.is_empty() {
            "{}".to_owned()
        } else {
            metadata_json.to_owned()
        };
        let event_id = input.event_id.trim().to_owned();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let metadata_valid = first_row(
            transaction
                .query(
                    "SELECT json_valid(:metadata_json)",
                    [(":metadata_json", metadata_json.as_str())],
                )
                .await?,
        )
        .await?;
        if integer_value(
            metadata_valid.get_value(0)?,
            "task_comments.metadata_json_valid",
        )? == 0
        {
            return Err(StoreError::InvalidInput(
                "metadata_json must be valid JSON".to_owned(),
            ));
        }
        let metadata_object = first_row(
            transaction
                .query(
                    "SELECT json_type(:metadata_json) = 'object'",
                    [(":metadata_json", metadata_json.as_str())],
                )
                .await?,
        )
        .await?;
        if integer_value(
            metadata_object.get_value(0)?,
            "task_comments.metadata_json_object",
        )? == 0
        {
            return Err(StoreError::InvalidInput(
                "metadata_json must be a JSON object".to_owned(),
            ));
        }
        let decision_metadata_valid = first_row(
            transaction
                .query(
                    r#"SELECT CASE
                        WHEN :kind != 'decision' THEN 1
                        WHEN COALESCE(json_type(:metadata_json, '$.options'), '') != 'array'
                          OR json_array_length(json_extract(:metadata_json, '$.options')) <= 0 THEN 0
                        WHEN COALESCE(json_type(:metadata_json, '$.selected'), '') != 'text'
                          OR length(trim(json_extract(:metadata_json, '$.selected'))) = 0 THEN 0
                        WHEN COALESCE(json_type(:metadata_json, '$.reason'), '') != 'text'
                          OR length(trim(json_extract(:metadata_json, '$.reason'))) = 0 THEN 0
                        WHEN json_type(:metadata_json, '$.risk') IS NOT NULL
                          AND (COALESCE(json_type(:metadata_json, '$.risk'), '') != 'text'
                            OR length(trim(json_extract(:metadata_json, '$.risk'))) = 0) THEN 0
                        WHEN json_type(:metadata_json, '$.verification') IS NOT NULL
                          AND (COALESCE(json_type(:metadata_json, '$.verification'), '') != 'text'
                            OR length(trim(json_extract(:metadata_json, '$.verification'))) = 0) THEN 0
                        WHEN EXISTS (
                            SELECT 1 FROM json_each(json_extract(:metadata_json, '$.options')) AS option
                            WHERE COALESCE(json_type(option.value), '') != 'object'
                              OR COALESCE(json_type(option.value, '$.slug'), '') != 'text'
                              OR length(trim(json_extract(option.value, '$.slug'))) = 0
                              OR json_extract(option.value, '$.slug') GLOB '*[^a-z0-9-]*'
                              OR substr(json_extract(option.value, '$.slug'), 1, 1) GLOB '[^a-z0-9]'
                              OR COALESCE(json_type(option.value, '$.title'), '') != 'text'
                              OR length(trim(json_extract(option.value, '$.title'))) = 0
                              OR COALESCE(json_type(option.value, '$.detail'), '') != 'text'
                              OR length(trim(json_extract(option.value, '$.detail'))) = 0
                        ) THEN 0
                        WHEN (SELECT COUNT(*) FROM json_each(json_extract(:metadata_json, '$.options')))
                          != (SELECT COUNT(DISTINCT json_extract(option.value, '$.slug'))
                              FROM json_each(json_extract(:metadata_json, '$.options')) AS option) THEN 0
                        WHEN NOT EXISTS (
                            SELECT 1 FROM json_each(json_extract(:metadata_json, '$.options')) AS option
                            WHERE json_extract(option.value, '$.slug') = json_extract(:metadata_json, '$.selected')
                        ) THEN 0
                        ELSE 1
                    END"#,
                    [
                        (":kind", kind.as_str()),
                        (":metadata_json", metadata_json.as_str()),
                    ],
                )
                .await?,
        )
        .await?;
        if integer_value(
            decision_metadata_valid.get_value(0)?,
            "task_comments.decision_metadata_valid",
        )? == 0
        {
            return Err(StoreError::InvalidInput(
                "invalid decision comment metadata".to_owned(),
            ));
        }

        let task = first_row(
            transaction
                .query(
                    "SELECT t.board_id, t.archived_at, b.archived_at FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
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
        let task_archived_at = optional_integer_value(task.get_value(1)?, "tasks.archived_at")?;
        let board_archived_at = optional_integer_value(task.get_value(2)?, "boards.archived_at")?;
        if task_archived_at.is_some() || board_archived_at.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived task or board cannot receive comments".to_owned(),
            ));
        }

        if let Some(idempotency_key) = idempotency_key.as_deref() {
            let existing = first_row(
                transaction
                    .query(
                        "SELECT id, board_id, task_id, idempotency_key, author, author_type, agent_type, body, kind, metadata_json, created_at FROM task_comments WHERE board_id = :board_id AND task_id = :task_id AND idempotency_key = :idempotency_key LIMIT 1",
                        [
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                            (":idempotency_key", idempotency_key),
                        ],
                    )
                    .await?,
            )
            .await;
            match existing {
                Ok(row) => {
                    let existing = comment_from_row(row)?;
                    if comment_payload_matches(
                        &existing,
                        idempotency_key,
                        &author,
                        &author_type,
                        agent_type.as_deref(),
                        &body,
                        &kind,
                        &metadata_json,
                    ) {
                        transaction.commit().await?;
                        return Ok(existing);
                    }
                    return Err(StoreError::IdempotencyConflict {
                        board_id,
                        key: idempotency_key.to_owned(),
                        existing_task_id: task_id.to_owned(),
                    });
                }
                Err(turso::Error::QueryReturnedNoRows) => {}
                Err(error) => return Err(StoreError::Turso(error)),
            }
        }

        transaction
            .execute(
                "INSERT INTO task_comments(id, board_id, task_id, idempotency_key, author, author_type, agent_type, body, kind, metadata_json, created_at) VALUES (:id, :board_id, :task_id, :idempotency_key, :author, :author_type, :agent_type, :body, :kind, :metadata_json, :created_at)",
                (
                    (":id", id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":idempotency_key", idempotency_key.as_deref()),
                    (":author", author.as_str()),
                    (":author_type", author_type.as_str()),
                    (":agent_type", agent_type.as_deref()),
                    (":body", body.as_str()),
                    (":kind", kind.as_str()),
                    (":metadata_json", metadata_json.as_str()),
                    (":created_at", input.created_at),
                ),
            )
            .await?;
        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.comment.created', :actor, json_object('comment_id', :comment_id, 'kind', :kind, 'author_type', :author_type, 'agent_type', :agent_type), :created_at)",
                (
                    (":event_id", event_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":actor", author.as_str()),
                    (":comment_id", id.as_str()),
                    (":kind", kind.as_str()),
                    (":author_type", author_type.as_str()),
                    (":agent_type", agent_type.as_deref()),
                    (":created_at", input.created_at),
                ),
            )
            .await?;

        let comment = comment_from_row(
            first_row(
                transaction
                    .query(
                        "SELECT id, board_id, task_id, idempotency_key, author, author_type, agent_type, body, kind, metadata_json, created_at FROM task_comments WHERE board_id = :board_id AND id = :id LIMIT 1",
                        [(":board_id", board_id.as_str()), (":id", id.as_str())],
                    )
                    .await?,
            )
            .await?,
        )?;

        transaction.commit().await?;
        Ok(comment)
    }

    pub async fn list_comments(&self, task_id: &str) -> Result<Vec<CommentRecord>, StoreError> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(StoreError::InvalidInput(
                "task id must start with t_".to_owned(),
            ));
        }
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
                "SELECT id, board_id, task_id, idempotency_key, author, author_type, agent_type, body, kind, metadata_json, created_at FROM task_comments WHERE board_id = :board_id AND task_id = :task_id ORDER BY created_at ASC, id ASC",
                [
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                ],
            )
            .await?;
        let mut comments = Vec::new();
        while let Some(row) = rows.next().await? {
            comments.push(comment_from_row(row)?);
        }
        Ok(comments)
    }

    /// Create one execution-plan step and apply the associated plan/status
    /// changes in a single immediate transaction. The application service
    /// supplies the expected parent facts; this method re-reads them and
    /// refuses stale writes before touching any canonical row.
    pub async fn create_step(
        &self,
        task_id: &str,
        input: CreateStepInput,
    ) -> Result<TaskStepRecord, StoreError> {
        validate_create_step_input(task_id, &input)?;
        let title = input.title.trim().to_owned();
        let body = input.body.map(|body| body.trim().to_owned());
        let idempotency_key = input
            .idempotency_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
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
        let board_id = parent.board_id.clone();
        if parent.archived_at.is_some() || parent.status == "archived" {
            return Err(StoreError::InvalidTransition(
                "archived parent task cannot receive steps".to_owned(),
            ));
        }
        let board_row = first_row(
            transaction
                .query(
                    "SELECT archived_at FROM boards WHERE id = :board_id LIMIT 1",
                    [(":board_id", board_id.as_str())],
                )
                .await?,
        )
        .await?;
        if optional_integer_value(board_row.get_value(0)?, "boards.archived_at")?.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived board cannot receive steps".to_owned(),
            ));
        }
        if let Some(idempotency_key) = idempotency_key.as_deref() {
            let existing = first_row(
                transaction
                    .query(
                        "SELECT id, board_id, parent_task_id, position, title, body, linked_task_id, required, status, resolution_note, resolved_by, resolved_at, created_by, created_at, updated_by, updated_at FROM task_steps WHERE board_id = :board_id AND parent_task_id = :parent_task_id AND idempotency_key = :idempotency_key LIMIT 1",
                        [
                            (":board_id", board_id.as_str()),
                            (":parent_task_id", parent.id.as_str()),
                            (":idempotency_key", idempotency_key),
                        ],
                    )
                    .await?,
            )
            .await;
            match existing {
                Ok(row) => {
                    let existing = step_from_row(&transaction, row).await?;
                    let effective_position = input.position.unwrap_or(existing.position);
                    if step_payload_matches(
                        &existing,
                        &title,
                        body.as_deref(),
                        input.linked_task_id.as_deref(),
                        effective_position,
                        input.required,
                        &input.created_by,
                    ) {
                        transaction.commit().await?;
                        return Ok(existing);
                    }
                    return Err(StoreError::IdempotencyConflict {
                        board_id,
                        key: idempotency_key.to_owned(),
                        existing_task_id: existing.id,
                    });
                }
                Err(turso::Error::QueryReturnedNoRows) => {}
                Err(error) => return Err(StoreError::Turso(error)),
            }
        }

        if input.expected_lock_version != parent.lock_version {
            return Err(StoreError::InvalidTransition(
                "step create requires matching fresh parent task".to_owned(),
            ));
        }
        if input.expected_plan_state.trim() != parent.execution_plan_state {
            return Err(StoreError::InvalidTransition(
                "step create requires matching execution plan".to_owned(),
            ));
        }
        if !matches!(
            parent.status.as_str(),
            "triage" | "todo" | "scheduled" | "ready" | "running" | "blocked" | "review"
        ) {
            return Err(StoreError::InvalidTransition(format!(
                "cannot create a step for {} task",
                parent.status
            )));
        }

        if let Some(linked_task_id) = input.linked_task_id.as_deref() {
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
            let linked_task = task_from_row(linked_row)?;
            if linked_task.board_id != board_id {
                return Err(StoreError::InvalidInput(
                    "linked task must belong to the parent board".to_owned(),
                ));
            }
            if linked_task.id == parent.id {
                return Err(StoreError::InvalidInput(
                    "step cannot link to its parent task".to_owned(),
                ));
            }
            if linked_task.archived_at.is_some() || linked_task.status == "archived" {
                return Err(StoreError::InvalidInput(
                    "archived linked task is not allowed".to_owned(),
                ));
            }
        }

        let position = match input.position {
            Some(position) => position,
            None => {
                let row = first_row(
                    transaction
                        .query(
                            "SELECT COALESCE(MAX(position), 0) FROM task_steps WHERE board_id = :board_id AND parent_task_id = :parent_task_id",
                            [
                                (":board_id", board_id.as_str()),
                                (":parent_task_id", parent.id.as_str()),
                            ],
                        )
                        .await?,
                )
                .await?;
                integer_value(row.get_value(0)?, "task_steps.max_position")?
                    .checked_add(1024)
                    .ok_or_else(|| {
                        StoreError::InvalidInput("step position is too large".to_owned())
                    })?
            }
        };

        transaction
            .execute(
                "INSERT INTO task_steps(id, board_id, parent_task_id, idempotency_key, position, title, body, linked_task_id, required, status, created_by, created_at, updated_by, updated_at) VALUES (:id, :board_id, :parent_task_id, :idempotency_key, :position, :title, :body, :linked_task_id, :required, 'todo', :created_by, :created_at, :created_by, :created_at)",
                (
                    (":id", input.id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":parent_task_id", parent.id.as_str()),
                    (":idempotency_key", idempotency_key.as_deref()),
                    (":position", position),
                    (":title", title.as_str()),
                    (":body", body.as_deref()),
                    (":linked_task_id", input.linked_task_id.as_deref()),
                    (":required", if input.required { 1_i64 } else { 0_i64 }),
                    (":created_by", input.created_by.as_str()),
                    (":created_at", input.created_at),
                ),
            )
            .await?;

        let plan_changed = parent.execution_plan_state != "planned";
        if plan_changed {
            transaction
                .execute(
                    "INSERT INTO task_execution_plans(board_id, task_id, state, reason, updated_by, updated_at) VALUES (:board_id, :task_id, 'planned', NULL, :actor, :updated_at) ON CONFLICT(task_id) DO UPDATE SET board_id = excluded.board_id, state = excluded.state, reason = NULL, updated_by = excluded.updated_by, updated_at = excluded.updated_at",
                    (
                        (":board_id", board_id.as_str()),
                        (":task_id", parent.id.as_str()),
                        (":actor", input.created_by.as_str()),
                        (":updated_at", input.created_at),
                    ),
                )
                .await?;
            transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.execution_plan.planned', :actor, '{\"state\":\"planned\"}', :created_at)",
                    (
                        (":event_id", input.plan_event_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", parent.id.as_str()),
                        (":actor", input.created_by.as_str()),
                        (":created_at", input.created_at),
                    ),
                )
                .await?;
        }

        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.step.created', :actor, json_object('step_id', :step_id, 'linked_task_id', :linked_task_id, 'position', :position, 'required', json(CASE WHEN :required = 1 THEN 'true' ELSE 'false' END), 'status', 'todo'), :created_at)",
                (
                    (":event_id", input.event_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", parent.id.as_str()),
                    (":actor", input.created_by.as_str()),
                    (":step_id", input.id.as_str()),
                    (":linked_task_id", input.linked_task_id.as_deref()),
                    (":position", position),
                    (":required", if input.required { 1_i64 } else { 0_i64 }),
                    (":created_at", input.created_at),
                ),
            )
            .await?;

        if matches!(
            parent.status.as_str(),
            "triage" | "todo" | "scheduled" | "ready"
        ) {
            let dependencies_done = first_row(
                transaction
                    .query(
                        "SELECT NOT EXISTS (SELECT 1 FROM task_dependencies AS d JOIN tasks AS dependency ON dependency.id = d.parent_task_id AND dependency.board_id = d.board_id WHERE d.board_id = :board_id AND d.child_task_id = :task_id AND dependency.status NOT IN ('done', 'archived'))",
                        (
                            (":board_id", board_id.as_str()),
                            (":task_id", parent.id.as_str()),
                        ),
                    )
                    .await?,
            )
            .await?;
            let dependencies_done =
                integer_value(dependencies_done.get_value(0)?, "task_dependencies.ready")? != 0;
            let computed_target = canonical_ready_status(
                &parent.title,
                parent.description.as_deref(),
                parent.scheduled_at,
                dependencies_done,
                input.created_at,
            );
            if computed_target != input.target_status.trim() {
                return Err(StoreError::InvalidTransition(
                    "step create readiness decision is stale".to_owned(),
                ));
            }
            if computed_target != parent.status {
                let changed = transaction
                    .execute(
                        "UPDATE tasks SET status = :target_status, status_reason = NULL, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = :current_status AND lock_version = :lock_version",
                        (
                            (":target_status", computed_target),
                            (":updated_at", input.created_at),
                            (":task_id", parent.id.as_str()),
                            (":board_id", board_id.as_str()),
                            (":current_status", parent.status.as_str()),
                            (":lock_version", parent.lock_version),
                        ),
                    )
                    .await?;
                if changed != 1 {
                    return Err(StoreError::InvalidTransition(
                        "step create requires matching fresh parent task".to_owned(),
                    ));
                }
                let payload = format!(r#"{{"to_status":"{computed_target}"}}"#);
                transaction
                    .execute(
                        "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.recomputed', :actor, :payload, :created_at)",
                        (
                            (":event_id", input.recompute_event_id.as_str()),
                            (":board_id", board_id.as_str()),
                            (":task_id", parent.id.as_str()),
                            (":actor", input.created_by.as_str()),
                            (":payload", payload.as_str()),
                            (":created_at", input.created_at),
                        ),
                    )
                    .await?;
            }
        } else if input.target_status.trim() != parent.status {
            return Err(StoreError::InvalidTransition(
                "step create cannot recompute this parent status".to_owned(),
            ));
        }

        let step = step_from_row(
            &transaction,
            first_row(
                transaction
                    .query(
                        "SELECT id, board_id, parent_task_id, position, title, body, linked_task_id, required, status, resolution_note, resolved_by, resolved_at, created_by, created_at, updated_by, updated_at FROM task_steps WHERE board_id = :board_id AND parent_task_id = :parent_task_id AND id = :id LIMIT 1",
                        [
                            (":board_id", board_id.as_str()),
                            (":parent_task_id", parent.id.as_str()),
                            (":id", input.id.as_str()),
                        ],
                    )
                    .await?,
            )
            .await?,
        )
        .await?;
        transaction.commit().await?;
        Ok(step)
    }

    /// Update editable execution-plan fields without changing the step status
    /// or parent task status. Parent lock-version CAS and the step/event write
    /// share one immediate transaction so stale callers cannot overwrite a
    /// concurrent plan mutation and an event conflict rolls everything back.
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

    /// Add one parent -> child dependency and return the post-mutation
    /// snapshot. The edge, optional child recomputation and event are guarded
    /// by one immediate transaction so a stale caller cannot observe a
    /// partially-applied dependency.
    pub async fn add_dependency(
        &self,
        child_task_id: &str,
        parent_task_id: &str,
        input: AddDependencyInput,
    ) -> Result<AddDependencyRecord, StoreError> {
        validate_add_dependency_input(child_task_id, parent_task_id, &input)?;
        let child_task_id = child_task_id.trim();
        let parent_task_id = parent_task_id.trim();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let child = dependency_task_in_transaction(&transaction, child_task_id).await?;
        let parent = dependency_task_in_transaction(&transaction, parent_task_id).await?;
        if child.board_id != parent.board_id {
            return Err(StoreError::InvalidInput(
                "cross-board dependency is not allowed".to_owned(),
            ));
        }
        let board = first_row(
            transaction
                .query(
                    "SELECT archived_at FROM boards WHERE id = :board_id LIMIT 1",
                    [(":board_id", child.board_id.as_str())],
                )
                .await?,
        )
        .await?;
        if optional_integer_value(board.get_value(0)?, "boards.archived_at")?.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived board cannot receive dependencies".to_owned(),
            ));
        }
        if child.archived_at.is_some() || child.status == "archived" {
            return Err(StoreError::InvalidTransition(
                "archived child task cannot receive dependencies".to_owned(),
            ));
        }

        let existing = first_row(
            transaction
                .query(
                    "SELECT 1 FROM task_dependencies WHERE board_id = :board_id AND parent_task_id = :parent_task_id AND child_task_id = :child_task_id LIMIT 1",
                    [
                        (":board_id", child.board_id.as_str()),
                        (":parent_task_id", parent.id.as_str()),
                        (":child_task_id", child.id.as_str()),
                    ],
                )
                .await?,
        )
        .await;
        match existing {
            Ok(_) => {
                let dependencies = dependency_snapshot_in_transaction(
                    &transaction,
                    child.board_id.as_str(),
                    child.id.as_str(),
                )
                .await?;
                transaction.commit().await?;
                return Ok(AddDependencyRecord {
                    added: false,
                    dependencies,
                });
            }
            Err(turso::Error::QueryReturnedNoRows) => {}
            Err(error) => return Err(StoreError::Turso(error)),
        }

        if dependency_path_exists(
            &transaction,
            child.board_id.as_str(),
            child.id.as_str(),
            parent.id.as_str(),
        )
        .await?
        {
            return Err(StoreError::DependencyCycle(
                "dependency cycle detected".to_owned(),
            ));
        }
        if child.status == "running" && !dependency_parent_satisfied(&parent) {
            return Err(StoreError::InvalidTransition(
                "cannot add incomplete dependency to running task".to_owned(),
            ));
        }
        if input.expected_child_lock_version != child.lock_version {
            return Err(StoreError::InvalidTransition(
                "dependency add requires matching fresh child task".to_owned(),
            ));
        }

        let target_status = if matches!(
            child.status.as_str(),
            "triage" | "todo" | "scheduled" | "ready"
        ) {
            let existing_dependencies_done = !child.dependency_blocked;
            let dependencies_done =
                existing_dependencies_done && dependency_parent_satisfied(&parent);
            let computed = canonical_ready_status(
                &child.title,
                child.description.as_deref(),
                child.scheduled_at,
                dependencies_done,
                input.now,
            );
            if computed == "ready" {
                child.status.clone()
            } else {
                computed.to_owned()
            }
        } else {
            child.status.clone()
        };
        if target_status != input.target_child_status.trim() {
            return Err(StoreError::InvalidTransition(
                "dependency add readiness decision is stale".to_owned(),
            ));
        }

        transaction
            .execute(
                "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES (:board_id, :parent_task_id, :child_task_id, :created_at) ON CONFLICT(parent_task_id, child_task_id) DO NOTHING",
                (
                    (":board_id", child.board_id.as_str()),
                    (":parent_task_id", parent.id.as_str()),
                    (":child_task_id", child.id.as_str()),
                    (":created_at", input.now),
                ),
            )
            .await?;

        if target_status != child.status {
            let changed = transaction
                .execute(
                    "UPDATE tasks SET status = :target_status, status_reason = NULL, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = :current_status AND archived_at IS NULL AND lock_version = :lock_version",
                    (
                        (":target_status", target_status.as_str()),
                        (":updated_at", input.now),
                        (":task_id", child.id.as_str()),
                        (":board_id", child.board_id.as_str()),
                        (":current_status", child.status.as_str()),
                        (":lock_version", input.expected_child_lock_version),
                    ),
                )
                .await?;
            if changed != 1 {
                return Err(StoreError::InvalidTransition(
                    "dependency add requires matching fresh child task".to_owned(),
                ));
            }
            let payload = format!(r#"{{"to_status":"{}"}}"#, target_status);
            transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.recomputed', :actor, :payload_json, :created_at)",
                    (
                        (":event_id", input.recompute_event_id.as_str()),
                        (":board_id", child.board_id.as_str()),
                        (":task_id", child.id.as_str()),
                        (":actor", input.actor.trim()),
                        (":payload_json", payload.as_str()),
                        (":created_at", input.now),
                    ),
                )
                .await?;
        }

        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'dependency.added', :actor, json_object('parent_task_id', :parent_task_id), :created_at)",
                (
                    (":event_id", input.event_id.as_str()),
                    (":board_id", child.board_id.as_str()),
                    (":task_id", child.id.as_str()),
                    (":actor", input.actor.trim()),
                    (":parent_task_id", parent.id.as_str()),
                    (":created_at", input.now),
                ),
            )
            .await?;

        let dependencies = dependency_snapshot_in_transaction(
            &transaction,
            child.board_id.as_str(),
            child.id.as_str(),
        )
        .await?;
        transaction.commit().await?;
        Ok(AddDependencyRecord {
            added: true,
            dependencies,
        })
    }

    /// Remove one parent -> child dependency and return the post-mutation
    /// snapshot. The edge delete and its event are guarded by one immediate
    /// transaction; a missing edge is a successful no-op with no event.
    pub async fn remove_dependency(
        &self,
        child_task_id: &str,
        parent_task_id: &str,
        input: RemoveDependencyInput,
    ) -> Result<RemoveDependencyRecord, StoreError> {
        validate_remove_dependency_input(child_task_id, parent_task_id, &input)?;
        let child_task_id = child_task_id.trim();
        let parent_task_id = parent_task_id.trim();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let child = dependency_task_in_transaction(&transaction, child_task_id).await?;
        let parent = dependency_task_in_transaction(&transaction, parent_task_id).await?;
        if child.board_id != parent.board_id {
            return Err(StoreError::InvalidInput(
                "cross-board dependency is not allowed".to_owned(),
            ));
        }
        let board = first_row(
            transaction
                .query(
                    "SELECT archived_at FROM boards WHERE id = :board_id LIMIT 1",
                    [(":board_id", child.board_id.as_str())],
                )
                .await?,
        )
        .await?;
        if optional_integer_value(board.get_value(0)?, "boards.archived_at")?.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived board cannot remove dependencies".to_owned(),
            ));
        }
        if child.archived_at.is_some() || child.status == "archived" {
            return Err(StoreError::InvalidTransition(
                "archived child task cannot remove dependencies".to_owned(),
            ));
        }

        let deleted = transaction
            .execute(
                "DELETE FROM task_dependencies WHERE board_id = :board_id AND parent_task_id = :parent_task_id AND child_task_id = :child_task_id",
                [
                    (":board_id", child.board_id.as_str()),
                    (":parent_task_id", parent.id.as_str()),
                    (":child_task_id", child.id.as_str()),
                ],
            )
            .await?;
        if deleted == 1 {
            transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'dependency.removed', :actor, json_object('parent_task_id', :parent_task_id), :created_at)",
                    (
                        (":event_id", input.event_id.as_str()),
                        (":board_id", child.board_id.as_str()),
                        (":task_id", child.id.as_str()),
                        (":actor", input.actor.trim()),
                        (":parent_task_id", parent.id.as_str()),
                        (":created_at", input.now),
                    ),
                )
                .await?;
        }

        let dependencies = dependency_snapshot_in_transaction(
            &transaction,
            child.board_id.as_str(),
            child.id.as_str(),
        )
        .await?;
        transaction.commit().await?;
        Ok(RemoveDependencyRecord {
            removed: deleted == 1,
            dependencies,
        })
    }

    pub async fn list_dependencies(
        &self,
        task_id: &str,
    ) -> Result<DependencySnapshotRecord, StoreError> {
        validate_task_id(task_id)?;
        let task_id = task_id.trim();
        let connection = self.connection().await?;
        let task = dependency_task_in_connection(&connection, task_id).await?;
        dependency_snapshot_in_connection(&connection, task.board_id.as_str(), task.id.as_str())
            .await
    }

    pub async fn list_tasks(
        &self,
        board_selector: &str,
        options: TaskListOptions,
    ) -> Result<TaskListPage, StoreError> {
        validate_task_list_options(&options)?;
        let connection = self.connection().await?;
        let board = first_row(
            connection
                .query(
                    "SELECT id, slug FROM boards WHERE id = ?1 OR slug = ?1 LIMIT 1",
                    [board_selector],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => {
                StoreError::BoardNotFound(board_selector.to_owned())
            }
            other => StoreError::Turso(other),
        })?;
        let board_id = text_value(board.get_value(0)?, "boards.id")?;
        let board_slug = text_value(board.get_value(1)?, "boards.slug")?;
        let (where_sql, params) = task_list_where(&board_id, &board_slug, &options);

        let total_row = first_row(
            connection
                .query(
                    &format!("SELECT COUNT(*) {TASK_FROM} {where_sql}"),
                    params.clone(),
                )
                .await?,
        )
        .await?;
        let total = integer_value(total_row.get_value(0)?, "tasks.total")?;
        let total = usize::try_from(total).map_err(|_| StoreError::InvalidStoredValue {
            field: "tasks.total",
        })?;

        let limit = i64::try_from(options.limit)
            .map_err(|_| StoreError::InvalidInput("limit is too large".to_owned()))?;
        let offset = i64::try_from(options.offset)
            .map_err(|_| StoreError::InvalidInput("offset is too large".to_owned()))?;
        let mut page_params = params;
        page_params.push((":limit".to_owned(), Value::Integer(limit)));
        page_params.push((":offset".to_owned(), Value::Integer(offset)));
        let mut rows = connection
            .query(
                &format!(
                    "{TASK_SELECT} {where_sql} ORDER BY {} LIMIT :limit OFFSET :offset",
                    task_order_by(options.sort)
                ),
                page_params,
            )
            .await?;
        let mut tasks = Vec::new();
        while let Some(row) = rows.next().await? {
            tasks.push(task_from_row(row)?);
        }
        Ok(TaskListPage { tasks, total })
    }

    pub async fn get_task_global(&self, task_id: &str) -> Result<TaskRecord, StoreError> {
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(StoreError::InvalidInput(
                "task id must start with t_".to_owned(),
            ));
        }
        let connection = self.connection().await?;
        let row = first_row(
            connection
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
        task_from_row(row)
    }

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

    pub async fn promote_task(
        &self,
        task_id: &str,
        input: PromoteTaskInput,
    ) -> Result<TaskRecord, StoreError> {
        validate_promote_task_input(task_id, &input)?;
        let actor = input.actor.trim().to_owned();
        let event_id = input.event_id.trim().to_owned();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let task = first_row(
            transaction
                .query(
                    "SELECT t.board_id, t.status, t.archived_at, b.archived_at, t.lock_version, t.title, t.description, t.scheduled_at FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
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
            return Err(StoreError::InvalidTransition(
                "archived task or board cannot be promoted".to_owned(),
            ));
        }

        let lock_version = integer_value(task.get_value(4)?, "tasks.lock_version")?;
        if lock_version != input.expected_lock_version {
            return Err(StoreError::InvalidTransition(
                "lock_version mismatch".to_owned(),
            ));
        }
        if !matches!(status.as_str(), "todo" | "scheduled") {
            return Err(StoreError::InvalidTransition(format!(
                "cannot promote from {status}"
            )));
        }

        let title = text_value(task.get_value(5)?, "tasks.title")?;
        let description = optional_text_value(task.get_value(6)?, "tasks.description")?;
        if title.trim().is_empty()
            || description
                .as_deref()
                .is_none_or(|description| description.trim().is_empty())
        {
            return Err(StoreError::InvalidTransition(
                "task spec is incomplete".to_owned(),
            ));
        }

        let scheduled_at = optional_integer_value(task.get_value(7)?, "tasks.scheduled_at")?;
        if status == "scheduled" && scheduled_at.is_none() {
            return Err(StoreError::InvalidTransition(
                "scheduled task requires scheduled_at".to_owned(),
            ));
        }
        if scheduled_at.is_some_and(|scheduled_at| scheduled_at > input.updated_at) {
            return Err(StoreError::InvalidTransition(
                "scheduled_at is in the future".to_owned(),
            ));
        }

        let dependency_blocked = first_row(
            transaction
                .query(
                    "SELECT EXISTS (SELECT 1 FROM task_dependencies AS d JOIN tasks AS p ON p.id = d.parent_task_id AND p.board_id = d.board_id WHERE d.board_id = :board_id AND d.child_task_id = :task_id AND p.status NOT IN ('done', 'archived'))",
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await?;
        if integer_value(
            dependency_blocked.get_value(0)?,
            "task_dependencies.unfinished_parent",
        )? != 0
        {
            return Err(StoreError::InvalidTransition(
                "dependency blocked".to_owned(),
            ));
        }

        let execution_plan_ready = first_row(
            transaction
                .query(
                    "SELECT EXISTS (SELECT 1 FROM task_steps AS s WHERE s.board_id = :board_id AND s.parent_task_id = :task_id) OR EXISTS (SELECT 1 FROM task_execution_plans AS ep WHERE ep.board_id = :board_id AND ep.task_id = :task_id AND ep.state = 'not_required')",
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await?;
        if integer_value(
            execution_plan_ready.get_value(0)?,
            "task_execution_plans.ready",
        )? == 0
        {
            return Err(StoreError::InvalidTransition(
                "execution plan is required".to_owned(),
            ));
        }

        let changed = transaction
            .execute(
                "UPDATE tasks SET status = 'ready', status_reason = NULL, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status IN ('todo', 'scheduled') AND lock_version = :expected_lock_version",
                (
                    (":updated_at", input.updated_at),
                    (":task_id", task_id),
                    (":board_id", board_id.as_str()),
                    (":expected_lock_version", input.expected_lock_version),
                ),
            )
            .await?;
        if changed != 1 {
            return Err(StoreError::InvalidTransition(
                "promote requires matching fresh task".to_owned(),
            ));
        }

        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.promoted', :actor, '{\"to_status\":\"ready\"}', :created_at)",
                (
                    (":event_id", event_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":actor", actor.as_str()),
                    (":created_at", input.updated_at),
                ),
            )
            .await?;

        let promoted = task_from_row(
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
        Ok(promoted)
    }

    pub async fn claim_task(
        &self,
        task_id: &str,
        input: ClaimTaskInput,
    ) -> Result<ClaimTaskRecord, StoreError> {
        validate_claim_task_input(task_id, &input)?;
        let owner = input.owner.trim().to_owned();
        let claim_token = input.claim_token.trim().to_owned();
        let run_id = input.run_id.trim().to_owned();
        let event_id = input.event_id.trim().to_owned();
        let worker_profile = input.worker_profile.trim().to_owned();
        let log_path = input.log_path.as_deref().map(str::trim).map(str::to_owned);
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let metadata_valid = first_row(
            transaction
                .query(
                    "SELECT json_valid(:metadata_json)",
                    [(":metadata_json", input.metadata_json.as_str())],
                )
                .await?,
        )
        .await?;
        if integer_value(
            metadata_valid.get_value(0)?,
            "task_runs.metadata_json_valid",
        )? == 0
        {
            return Err(StoreError::InvalidInput(
                "metadata_json must be valid JSON".to_owned(),
            ));
        }

        let task = first_row(
            transaction
                .query(
                    "SELECT t.board_id, t.status, t.archived_at, b.archived_at, t.lock_version, t.title, t.description, t.scheduled_at, t.claim_token, t.claim_owner, t.claim_expires_at, t.current_run_id FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
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
        let task_archived_at = optional_integer_value(task.get_value(2)?, "tasks.archived_at")?;
        let board_archived_at = optional_integer_value(task.get_value(3)?, "boards.archived_at")?;
        if status == "archived" || task_archived_at.is_some() || board_archived_at.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived task or board cannot be claimed".to_owned(),
            ));
        }

        let lock_version = integer_value(task.get_value(4)?, "tasks.lock_version")?;
        if lock_version != input.expected_lock_version {
            return Err(StoreError::ClaimConflict(
                "lock_version mismatch".to_owned(),
            ));
        }
        if status != "ready" {
            return Err(StoreError::InvalidTransition(
                "task is not ready".to_owned(),
            ));
        }
        let existing_claim_token = optional_text_value(task.get_value(8)?, "tasks.claim_token")?;
        let existing_claim_owner = optional_text_value(task.get_value(9)?, "tasks.claim_owner")?;
        let existing_claim_expires_at =
            optional_integer_value(task.get_value(10)?, "tasks.claim_expires_at")?;
        let existing_run_id = optional_text_value(task.get_value(11)?, "tasks.current_run_id")?;
        if existing_claim_token.is_some()
            || existing_claim_owner.is_some()
            || existing_claim_expires_at.is_some()
            || existing_run_id.is_some()
        {
            return Err(StoreError::ClaimConflict(
                "task is already claimed".to_owned(),
            ));
        }

        let title = text_value(task.get_value(5)?, "tasks.title")?;
        let description = optional_text_value(task.get_value(6)?, "tasks.description")?;
        if title.trim().is_empty()
            || description
                .as_deref()
                .is_none_or(|description| description.trim().is_empty())
        {
            return Err(StoreError::InvalidTransition(
                "task spec is incomplete".to_owned(),
            ));
        }
        let scheduled_at = optional_integer_value(task.get_value(7)?, "tasks.scheduled_at")?;
        if scheduled_at.is_some_and(|scheduled_at| scheduled_at > input.now) {
            return Err(StoreError::InvalidTransition(
                "scheduled_at is in the future".to_owned(),
            ));
        }

        let dependency_blocked = first_row(
            transaction
                .query(
                    "SELECT EXISTS (SELECT 1 FROM task_dependencies AS d JOIN tasks AS p ON p.id = d.parent_task_id AND p.board_id = d.board_id WHERE d.board_id = :board_id AND d.child_task_id = :task_id AND p.status NOT IN ('done', 'archived'))",
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await?;
        if integer_value(
            dependency_blocked.get_value(0)?,
            "task_dependencies.unfinished_parent",
        )? != 0
        {
            return Err(StoreError::InvalidTransition(
                "dependency blocked".to_owned(),
            ));
        }

        let execution_plan_ready = first_row(
            transaction
                .query(
                    "SELECT EXISTS (SELECT 1 FROM task_steps AS s WHERE s.board_id = :board_id AND s.parent_task_id = :task_id) OR EXISTS (SELECT 1 FROM task_execution_plans AS ep WHERE ep.board_id = :board_id AND ep.task_id = :task_id AND ep.state = 'not_required')",
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await?;
        if integer_value(
            execution_plan_ready.get_value(0)?,
            "task_execution_plans.ready",
        )? == 0
        {
            return Err(StoreError::InvalidTransition(
                "execution plan is required".to_owned(),
            ));
        }

        let changed = transaction
            .execute(
                "UPDATE tasks SET status = 'running', claim_owner = :claim_owner, claim_token = :claim_token, claim_expires_at = :claim_expires_at, last_heartbeat_at = :last_heartbeat_at, current_run_id = :current_run_id, started_at = COALESCE(started_at, :started_at), updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = 'ready' AND claim_token IS NULL AND claim_owner IS NULL AND claim_expires_at IS NULL AND current_run_id IS NULL AND lock_version = :expected_lock_version",
                (
                    (":claim_owner", owner.as_str()),
                    (":claim_token", claim_token.as_str()),
                    (":claim_expires_at", input.claim_expires_at),
                    (":last_heartbeat_at", input.now),
                    (":current_run_id", run_id.as_str()),
                    (":started_at", input.now),
                    (":updated_at", input.now),
                    (":task_id", task_id),
                    (":board_id", board_id.as_str()),
                    (":expected_lock_version", input.expected_lock_version),
                ),
            )
            .await?;
        if changed != 1 {
            return Err(StoreError::ClaimConflict(
                "claim compare-and-set failed".to_owned(),
            ));
        }

        transaction
            .execute(
                "INSERT INTO task_runs(id, board_id, task_id, status, worker_profile, worker_pid, claim_token, claim_owner, claim_expires_at, started_at, last_heartbeat_at, log_path, metadata_json) VALUES (:run_id, :board_id, :task_id, 'running', :worker_profile, NULL, :claim_token, :claim_owner, :claim_expires_at, :started_at, :last_heartbeat_at, :log_path, :metadata_json)",
                (
                    (":run_id", run_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":worker_profile", worker_profile.as_str()),
                    (":claim_token", claim_token.as_str()),
                    (":claim_owner", owner.as_str()),
                    (":claim_expires_at", input.claim_expires_at),
                    (":started_at", input.now),
                    (":last_heartbeat_at", input.now),
                    (":log_path", log_path.as_deref()),
                    (":metadata_json", input.metadata_json.as_str()),
                ),
            )
            .await?;

        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, :run_id, 'task.claimed', :actor, json_object('claim_owner', :claim_owner, 'metadata', json(:metadata_json)), :created_at)",
                (
                    (":event_id", event_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":run_id", run_id.as_str()),
                    (":actor", owner.as_str()),
                    (":claim_owner", owner.as_str()),
                    (":metadata_json", input.metadata_json.as_str()),
                    (":created_at", input.now),
                ),
            )
            .await?;

        let claimed_task = task_from_row(
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
        let run = run_from_row(
            first_row(
                transaction
                    .query(
                        "SELECT id, board_id, task_id, status, worker_profile, worker_pid, claim_token, claim_owner, claim_expires_at, started_at, last_heartbeat_at, finished_at, exit_code, summary, error, log_path, metadata_json FROM task_runs WHERE board_id = :board_id AND id = :run_id LIMIT 1",
                        [(":board_id", board_id.as_str()), (":run_id", run_id.as_str())],
                    )
                    .await?,
            )
            .await?,
        )?;

        transaction.commit().await?;
        Ok(ClaimTaskRecord {
            task: claimed_task,
            run,
            claim_token,
            claim_expires_at: input.claim_expires_at,
        })
    }

    pub async fn heartbeat_task(
        &self,
        task_id: &str,
        input: HeartbeatTaskInput,
    ) -> Result<TaskRecord, StoreError> {
        validate_heartbeat_task_input(task_id, &input)?;
        let actor = input.actor.trim().to_owned();
        let claim_token = input.claim_token.as_str();
        let event_id = input.event_id.trim().to_owned();
        let note = input.note.as_deref();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let task = first_row(
            transaction
                .query(
                    "SELECT t.board_id, t.status, t.archived_at, b.archived_at, t.lock_version, t.claim_token, t.claim_owner, t.claim_expires_at, t.current_run_id FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
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
        let task_archived_at = optional_integer_value(task.get_value(2)?, "tasks.archived_at")?;
        let board_archived_at = optional_integer_value(task.get_value(3)?, "boards.archived_at")?;
        if task_archived_at.is_some() || board_archived_at.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived task or board cannot be heartbeated".to_owned(),
            ));
        }
        if status != "running" {
            return Err(StoreError::InvalidTransition(
                "heartbeat requires a running task".to_owned(),
            ));
        }

        let lock_version = integer_value(task.get_value(4)?, "tasks.lock_version")?;
        if lock_version != input.expected_lock_version {
            return Err(StoreError::ClaimConflict(
                "lock_version mismatch".to_owned(),
            ));
        }
        let task_claim_token = optional_text_value(task.get_value(5)?, "tasks.claim_token")?;
        if task_claim_token.as_deref() != Some(claim_token) {
            return Err(StoreError::ClaimTokenMismatch);
        }
        let task_claim_owner = optional_text_value(task.get_value(6)?, "tasks.claim_owner")?;
        if task_claim_owner.as_deref() != Some(actor.as_str()) {
            return Err(StoreError::InvalidTransition(
                "claim owner mismatch".to_owned(),
            ));
        }
        if optional_integer_value(task.get_value(7)?, "tasks.claim_expires_at")?.is_none() {
            return Err(StoreError::InvalidTransition(
                "heartbeat requires an active claim".to_owned(),
            ));
        }
        let run_id = optional_text_value(task.get_value(8)?, "tasks.current_run_id")?
            .filter(|run_id| !run_id.trim().is_empty())
            .ok_or_else(|| {
                StoreError::InvalidTransition("heartbeat requires a current running run".to_owned())
            })?;

        let active_run_count = first_row(
            transaction
                .query(
                    "SELECT COUNT(*) FROM task_runs WHERE board_id = :board_id AND task_id = :task_id AND status = 'running'",
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await?;
        if integer_value(active_run_count.get_value(0)?, "task_runs.active_count")? != 1 {
            return Err(StoreError::InvalidTransition(
                "heartbeat requires exactly one running run".to_owned(),
            ));
        }

        let run = first_row(
            transaction
                .query(
                    "SELECT id, board_id, task_id, status, worker_profile, worker_pid, claim_token, claim_owner, claim_expires_at, started_at, last_heartbeat_at, finished_at, exit_code, summary, error, log_path, metadata_json FROM task_runs WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id LIMIT 1",
                    [
                        (":run_id", run_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::InvalidTransition(
                "heartbeat requires a matching running run".to_owned(),
            ),
            other => StoreError::Turso(other),
        })?;
        let run_status = text_value(run.get_value(3)?, "task_runs.status")?;
        if run_status != "running" {
            return Err(StoreError::InvalidTransition(
                "heartbeat requires a matching running run".to_owned(),
            ));
        }
        let run_claim_token = text_value(run.get_value(6)?, "task_runs.claim_token")?;
        let run_claim_owner = text_value(run.get_value(7)?, "task_runs.claim_owner")?;
        if task_claim_token.as_deref() != Some(run_claim_token.as_str())
            || task_claim_owner.as_deref() != Some(run_claim_owner.as_str())
        {
            return Err(StoreError::InvalidTransition(
                "active run claim is inconsistent".to_owned(),
            ));
        }

        let changed = transaction
            .execute(
                "UPDATE tasks SET claim_expires_at = :claim_expires_at, last_heartbeat_at = :last_heartbeat_at, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = 'running' AND claim_token = :claim_token AND claim_owner = :claim_owner AND current_run_id = :run_id AND lock_version = :expected_lock_version",
                (
                    (":claim_expires_at", input.claim_expires_at),
                    (":last_heartbeat_at", input.now),
                    (":updated_at", input.now),
                    (":task_id", task_id),
                    (":board_id", board_id.as_str()),
                    (":claim_token", claim_token),
                    (":claim_owner", actor.as_str()),
                    (":run_id", run_id.as_str()),
                    (":expected_lock_version", input.expected_lock_version),
                ),
            )
            .await?;
        if changed != 1 {
            return Err(StoreError::ClaimConflict(
                "heartbeat compare-and-set failed".to_owned(),
            ));
        }

        let changed = transaction
            .execute(
                "UPDATE task_runs SET claim_expires_at = :claim_expires_at, last_heartbeat_at = :last_heartbeat_at WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id AND status = 'running' AND claim_token = :claim_token AND claim_owner = :claim_owner",
                (
                    (":claim_expires_at", input.claim_expires_at),
                    (":last_heartbeat_at", input.now),
                    (":run_id", run_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":claim_token", claim_token),
                    (":claim_owner", actor.as_str()),
                ),
            )
            .await?;
        if changed != 1 {
            return Err(StoreError::InvalidTransition(
                "heartbeat requires a matching running run".to_owned(),
            ));
        }

        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, :run_id, 'task.heartbeat', :actor, json_object('note', :note), :created_at)",
                (
                    (":event_id", event_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":run_id", run_id.as_str()),
                    (":actor", actor.as_str()),
                    (":note", note),
                    (":created_at", input.now),
                ),
            )
            .await?;

        let heartbeated = task_from_row(
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
        Ok(heartbeated)
    }

    pub async fn release_task(
        &self,
        task_id: &str,
        input: ReleaseTaskInput,
    ) -> Result<TaskRecord, StoreError> {
        validate_release_task_input(task_id, &input)?;
        let actor = input.actor.trim().to_owned();
        let claim_token = input.claim_token.as_str();
        let event_id = input.event_id.trim().to_owned();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let task = first_row(
            transaction
                .query(
                    "SELECT t.board_id, t.status, t.archived_at, b.archived_at, t.lock_version, t.title, t.description, t.scheduled_at, t.claim_token, t.claim_owner, t.claim_expires_at, t.current_run_id FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
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
        let task_archived_at = optional_integer_value(task.get_value(2)?, "tasks.archived_at")?;
        let board_archived_at = optional_integer_value(task.get_value(3)?, "boards.archived_at")?;
        if task_archived_at.is_some() || board_archived_at.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived task or board cannot be released".to_owned(),
            ));
        }
        if status != "running" {
            return Err(StoreError::InvalidTransition(
                "release requires a running task".to_owned(),
            ));
        }

        let lock_version = integer_value(task.get_value(4)?, "tasks.lock_version")?;
        if lock_version != input.expected_lock_version {
            return Err(StoreError::ClaimConflict(
                "lock_version mismatch".to_owned(),
            ));
        }
        let title = text_value(task.get_value(5)?, "tasks.title")?;
        let description = optional_text_value(task.get_value(6)?, "tasks.description")?;
        let scheduled_at = optional_integer_value(task.get_value(7)?, "tasks.scheduled_at")?;
        let task_claim_token = optional_text_value(task.get_value(8)?, "tasks.claim_token")?;
        if task_claim_token.as_deref() != Some(claim_token) {
            return Err(StoreError::ClaimTokenMismatch);
        }
        let task_claim_owner = optional_text_value(task.get_value(9)?, "tasks.claim_owner")?;
        if task_claim_owner.as_deref() != Some(actor.as_str()) {
            return Err(StoreError::InvalidTransition(
                "claim owner mismatch".to_owned(),
            ));
        }
        if optional_integer_value(task.get_value(10)?, "tasks.claim_expires_at")?.is_none() {
            return Err(StoreError::InvalidTransition(
                "release requires an active claim".to_owned(),
            ));
        }
        let run_id = optional_text_value(task.get_value(11)?, "tasks.current_run_id")?
            .filter(|run_id| !run_id.trim().is_empty())
            .ok_or_else(|| {
                StoreError::InvalidTransition("release requires a current running run".to_owned())
            })?;

        if title.trim().is_empty()
            || description
                .as_deref()
                .is_none_or(|description| description.trim().is_empty())
        {
            return Err(StoreError::InvalidTransition(
                "task spec is incomplete".to_owned(),
            ));
        }
        if scheduled_at.is_some_and(|scheduled_at| scheduled_at > input.now) {
            return Err(StoreError::InvalidTransition(
                "scheduled_at is in the future".to_owned(),
            ));
        }

        let dependency_blocked = first_row(
            transaction
                .query(
                    "SELECT EXISTS (SELECT 1 FROM task_dependencies AS d JOIN tasks AS p ON p.id = d.parent_task_id AND p.board_id = d.board_id WHERE d.board_id = :board_id AND d.child_task_id = :task_id AND p.status NOT IN ('done', 'archived'))",
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await?;
        if integer_value(
            dependency_blocked.get_value(0)?,
            "task_dependencies.unfinished_parent",
        )? != 0
        {
            return Err(StoreError::InvalidTransition(
                "dependency blocked".to_owned(),
            ));
        }

        let execution_plan_ready = first_row(
            transaction
                .query(
                    "SELECT EXISTS (SELECT 1 FROM task_steps AS s WHERE s.board_id = :board_id AND s.parent_task_id = :task_id) OR EXISTS (SELECT 1 FROM task_execution_plans AS ep WHERE ep.board_id = :board_id AND ep.task_id = :task_id AND ep.state = 'not_required')",
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await?;
        if integer_value(
            execution_plan_ready.get_value(0)?,
            "task_execution_plans.ready",
        )? == 0
        {
            return Err(StoreError::InvalidTransition(
                "execution plan is required".to_owned(),
            ));
        }

        let active_run_count = first_row(
            transaction
                .query(
                    "SELECT COUNT(*) FROM task_runs WHERE board_id = :board_id AND task_id = :task_id AND status = 'running'",
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await?;
        if integer_value(active_run_count.get_value(0)?, "task_runs.active_count")? != 1 {
            return Err(StoreError::InvalidTransition(
                "release requires exactly one running run".to_owned(),
            ));
        }

        let run = first_row(
            transaction
                .query(
                    "SELECT status, claim_token, claim_owner FROM task_runs WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id LIMIT 1",
                    [
                        (":run_id", run_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::InvalidTransition(
                "release requires a matching running run".to_owned(),
            ),
            other => StoreError::Turso(other),
        })?;
        if text_value(run.get_value(0)?, "task_runs.status")? != "running" {
            return Err(StoreError::InvalidTransition(
                "release requires a matching running run".to_owned(),
            ));
        }
        let run_claim_token = text_value(run.get_value(1)?, "task_runs.claim_token")?;
        let run_claim_owner = text_value(run.get_value(2)?, "task_runs.claim_owner")?;
        if task_claim_token.as_deref() != Some(run_claim_token.as_str())
            || task_claim_owner.as_deref() != Some(run_claim_owner.as_str())
        {
            return Err(StoreError::InvalidTransition(
                "active run claim is inconsistent".to_owned(),
            ));
        }

        let changed = transaction
            .execute(
                "UPDATE task_runs SET status = 'canceled', finished_at = :finished_at WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id AND status = 'running' AND claim_token = :claim_token AND claim_owner = :claim_owner",
                (
                    (":finished_at", input.now),
                    (":run_id", run_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":claim_token", claim_token),
                    (":claim_owner", actor.as_str()),
                ),
            )
            .await?;
        if changed != 1 {
            return Err(StoreError::InvalidTransition(
                "release requires a matching running run".to_owned(),
            ));
        }

        let changed = transaction
            .execute(
                "UPDATE tasks SET status = 'ready', status_reason = NULL, claim_token = NULL, claim_owner = NULL, claim_expires_at = NULL, last_heartbeat_at = NULL, current_run_id = NULL, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = 'running' AND claim_token = :claim_token AND claim_owner = :claim_owner AND current_run_id = :run_id AND lock_version = :expected_lock_version",
                (
                    (":updated_at", input.now),
                    (":task_id", task_id),
                    (":board_id", board_id.as_str()),
                    (":claim_token", claim_token),
                    (":claim_owner", actor.as_str()),
                    (":run_id", run_id.as_str()),
                    (":expected_lock_version", input.expected_lock_version),
                ),
            )
            .await?;
        if changed != 1 {
            return Err(StoreError::ClaimConflict(
                "release compare-and-set failed".to_owned(),
            ));
        }

        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, :run_id, 'task.released', :actor, json_object('to_status', 'ready'), :created_at)",
                (
                    (":event_id", event_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":run_id", run_id.as_str()),
                    (":actor", actor.as_str()),
                    (":created_at", input.now),
                ),
            )
            .await?;

        let released = task_from_row(
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
        Ok(released)
    }

    pub async fn list_expired_claims(
        &self,
        board_selector: &str,
        now: i64,
    ) -> Result<Vec<TaskRecord>, StoreError> {
        if now < 0 {
            return Err(StoreError::InvalidInput(
                "now must be non-negative".to_owned(),
            ));
        }
        let board_selector = board_selector.trim();
        if board_selector.is_empty() {
            return Err(StoreError::InvalidInput("board is required".to_owned()));
        }

        let connection = self.connection().await?;
        let board = first_row(
            connection
                .query(
                    "SELECT id, archived_at FROM boards WHERE id = :board OR slug = :board LIMIT 1",
                    [(":board", board_selector)],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => {
                StoreError::BoardNotFound(board_selector.to_owned())
            }
            other => StoreError::Turso(other),
        })?;
        let board_id = text_value(board.get_value(0)?, "boards.id")?;
        if optional_integer_value(board.get_value(1)?, "boards.archived_at")?.is_some() {
            return Ok(Vec::new());
        }

        let mut rows = connection
            .query(
                &format!(
                    "{TASK_SELECT} WHERE t.board_id = :board_id AND b.archived_at IS NULL AND t.archived_at IS NULL AND t.status = 'running' AND t.claim_expires_at <= :now ORDER BY t.claim_expires_at ASC, t.id ASC"
                ),
                vec![
                    (":board_id".to_owned(), Value::Text(board_id.to_owned())),
                    (":now".to_owned(), Value::Integer(now)),
                ],
            )
            .await?;
        let mut tasks = Vec::new();
        while let Some(row) = rows.next().await? {
            tasks.push(task_from_row(row)?);
        }
        Ok(tasks)
    }

    pub async fn reclaim_expired_task(
        &self,
        task_id: &str,
        input: ReclaimExpiredTaskInput,
    ) -> Result<Option<TaskRecord>, StoreError> {
        validate_reclaim_expired_task_input(task_id, &input)?;
        let actor = input.actor.trim().to_owned();
        let event_id = input.event_id.trim().to_owned();
        let target_status = input.target_status.trim().to_owned();
        let reason = input.reason.trim().to_owned();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let task = first_row(
            transaction
                .query(
                    "SELECT t.board_id, t.status, t.archived_at, b.archived_at, t.lock_version, t.claim_token, t.claim_owner, t.claim_expires_at, t.last_heartbeat_at, t.current_run_id, t.retry_count, t.max_retries, t.title, t.description, t.scheduled_at FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
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
        let task_archived_at = optional_integer_value(task.get_value(2)?, "tasks.archived_at")?;
        let board_archived_at = optional_integer_value(task.get_value(3)?, "boards.archived_at")?;
        if status == "archived" || task_archived_at.is_some() || board_archived_at.is_some() {
            return Ok(None);
        }
        if status != "running" {
            return Ok(None);
        }

        let lock_version = integer_value(task.get_value(4)?, "tasks.lock_version")?;
        if lock_version != input.expected_lock_version {
            return Ok(None);
        }
        let claim_token = optional_text_value(task.get_value(5)?, "tasks.claim_token")?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                StoreError::InvalidTransition(
                    "reclaim requires a matching task claim token".to_owned(),
                )
            })?;
        let claim_owner = optional_text_value(task.get_value(6)?, "tasks.claim_owner")?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                StoreError::InvalidTransition(
                    "reclaim requires a matching task claim owner".to_owned(),
                )
            })?;
        let claim_expires_at = optional_integer_value(
            task.get_value(7)?,
            "tasks.claim_expires_at",
        )?
        .ok_or_else(|| {
            StoreError::InvalidTransition("reclaim requires an expiring task claim".to_owned())
        })?;
        if claim_expires_at > input.now {
            return Ok(None);
        }
        let run_id = optional_text_value(task.get_value(9)?, "tasks.current_run_id")?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                StoreError::InvalidTransition("reclaim requires a current running run".to_owned())
            })?;
        let retry_count = integer_value(task.get_value(10)?, "tasks.retry_count")?;
        let max_retries = optional_integer_value(task.get_value(11)?, "tasks.max_retries")?;
        let title = text_value(task.get_value(12)?, "tasks.title")?;
        let description = optional_text_value(task.get_value(13)?, "tasks.description")?;
        let scheduled_at = optional_integer_value(task.get_value(14)?, "tasks.scheduled_at")?;

        let active_run_count = first_row(
            transaction
                .query(
                    "SELECT COUNT(*) FROM task_runs WHERE board_id = :board_id AND task_id = :task_id AND status = 'running'",
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await?;
        if integer_value(active_run_count.get_value(0)?, "task_runs.active_count")? != 1 {
            return Err(StoreError::InvalidTransition(
                "reclaim requires exactly one running run".to_owned(),
            ));
        }

        let run = first_row(
            transaction
                .query(
                    "SELECT status, claim_token, claim_owner, claim_expires_at FROM task_runs WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id LIMIT 1",
                    [
                        (":run_id", run_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::InvalidTransition(
                "reclaim requires a matching running run".to_owned(),
            ),
            other => StoreError::Turso(other),
        })?;
        if text_value(run.get_value(0)?, "task_runs.status")? != "running" {
            return Err(StoreError::InvalidTransition(
                "reclaim requires a matching running run".to_owned(),
            ));
        }
        let run_claim_token = text_value(run.get_value(1)?, "task_runs.claim_token")?;
        let run_claim_owner = text_value(run.get_value(2)?, "task_runs.claim_owner")?;
        let run_claim_expires_at = integer_value(run.get_value(3)?, "task_runs.claim_expires_at")?;
        if run_claim_token != claim_token
            || run_claim_owner != claim_owner
            || run_claim_expires_at != claim_expires_at
        {
            return Err(StoreError::InvalidTransition(
                "active run claim is inconsistent".to_owned(),
            ));
        }
        if run_claim_expires_at > input.now {
            return Err(StoreError::InvalidTransition(
                "active run claim is not expired".to_owned(),
            ));
        }

        let dependency_blocked = first_row(
            transaction
                .query(
                    "SELECT EXISTS (SELECT 1 FROM task_dependencies AS d JOIN tasks AS p ON p.id = d.parent_task_id AND p.board_id = d.board_id WHERE d.board_id = :board_id AND d.child_task_id = :task_id AND p.status NOT IN ('done', 'archived'))",
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await?;
        let dependency_blocked = integer_value(
            dependency_blocked.get_value(0)?,
            "task_dependencies.unfinished_parent",
        )? != 0;
        let execution_plan_ready = first_row(
            transaction
                .query(
                    "SELECT EXISTS (SELECT 1 FROM task_steps AS s WHERE s.board_id = :board_id AND s.parent_task_id = :task_id) OR EXISTS (SELECT 1 FROM task_execution_plans AS ep WHERE ep.board_id = :board_id AND ep.task_id = :task_id AND ep.state = 'not_required')",
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await?;
        let execution_plan_ready = integer_value(
            execution_plan_ready.get_value(0)?,
            "task_execution_plans.ready",
        )? != 0;

        let next_retry_count = retry_count
            .checked_add(1)
            .ok_or_else(|| StoreError::InvalidTransition("retry_count overflow".to_owned()))?;
        if input.retry_count != next_retry_count {
            return Err(StoreError::InvalidTransition(
                "retry_count does not match canonical task state".to_owned(),
            ));
        }
        let canonical_status = if max_retries.is_some_and(|max| next_retry_count >= max) {
            "blocked"
        } else if title.trim().is_empty()
            || description
                .as_deref()
                .is_none_or(|description| description.trim().is_empty())
        {
            "triage"
        } else if scheduled_at.is_some_and(|scheduled_at| scheduled_at > input.now) {
            "scheduled"
        } else if dependency_blocked || !execution_plan_ready {
            "todo"
        } else {
            "ready"
        };
        if target_status != canonical_status {
            return Err(StoreError::InvalidTransition(
                "target_status does not match canonical task state".to_owned(),
            ));
        }

        let changed = transaction
            .execute(
                "UPDATE task_runs SET status = 'expired', finished_at = :finished_at, error = 'claim expired' WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id AND status = 'running' AND claim_token = :claim_token AND claim_owner = :claim_owner AND claim_expires_at = :claim_expires_at AND claim_expires_at <= :now",
                (
                    (":finished_at", input.now),
                    (":run_id", run_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":claim_token", claim_token.as_str()),
                    (":claim_owner", claim_owner.as_str()),
                    (":claim_expires_at", claim_expires_at),
                    (":now", input.now),
                ),
            )
            .await?;
        if changed != 1 {
            return Err(StoreError::InvalidTransition(
                "reclaim requires a matching expired running run".to_owned(),
            ));
        }

        let status_reason = (canonical_status == "blocked").then_some(reason.as_str());
        let changed = transaction
            .execute(
                "UPDATE tasks SET status = :status, status_reason = :status_reason, claim_token = NULL, claim_owner = NULL, claim_expires_at = NULL, last_heartbeat_at = NULL, current_run_id = NULL, retry_count = :retry_count, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = 'running' AND claim_token = :claim_token AND claim_owner = :claim_owner AND claim_expires_at = :claim_expires_at AND current_run_id = :run_id AND lock_version = :expected_lock_version",
                (
                    (":status", canonical_status),
                    (":status_reason", status_reason),
                    (":retry_count", input.retry_count),
                    (":updated_at", input.now),
                    (":task_id", task_id),
                    (":board_id", board_id.as_str()),
                    (":claim_token", claim_token.as_str()),
                    (":claim_owner", claim_owner.as_str()),
                    (":claim_expires_at", claim_expires_at),
                    (":run_id", run_id.as_str()),
                    (":expected_lock_version", input.expected_lock_version),
                ),
            )
            .await?;
        if changed != 1 {
            return Err(StoreError::ClaimConflict(
                "reclaim compare-and-set failed".to_owned(),
            ));
        }

        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, :run_id, 'task.reclaimed', :actor, json_object('retry_count', :retry_count, 'max_retries', :max_retries, 'to_status', :to_status, 'reason', :reason), :created_at)",
                (
                    (":event_id", event_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":run_id", run_id.as_str()),
                    (":actor", actor.as_str()),
                    (":retry_count", input.retry_count),
                    (":max_retries", max_retries),
                    (":to_status", canonical_status),
                    (":reason", reason.as_str()),
                    (":created_at", input.now),
                ),
            )
            .await?;

        let reclaimed = task_from_row(
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
        Ok(Some(reclaimed))
    }

    pub async fn submit_review_task(
        &self,
        task_id: &str,
        input: SubmitReviewTaskInput,
    ) -> Result<TaskRecord, StoreError> {
        validate_submit_review_task_input(task_id, &input)?;
        let actor = input.actor.trim().to_owned();
        let input_claim_token = input.claim_token.as_deref();
        let event_id = input.event_id.trim().to_owned();
        let summary = input.summary.as_deref();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let task = first_row(
            transaction
                .query(
                    "SELECT t.board_id, t.status, t.archived_at, b.archived_at, t.lock_version, t.claim_token, t.claim_owner, t.claim_expires_at, t.current_run_id, t.result_summary FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
                    [ (":task_id", task_id) ],
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
        let task_archived_at = optional_integer_value(task.get_value(2)?, "tasks.archived_at")?;
        let board_archived_at = optional_integer_value(task.get_value(3)?, "boards.archived_at")?;
        if task_archived_at.is_some() || board_archived_at.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived task or board cannot submit review".to_owned(),
            ));
        }
        if status != "running" {
            return Err(StoreError::InvalidTransition(
                "submit review requires a running task".to_owned(),
            ));
        }

        let lock_version = integer_value(task.get_value(4)?, "tasks.lock_version")?;
        if lock_version != input.expected_lock_version {
            return Err(StoreError::ClaimConflict(
                "lock_version mismatch".to_owned(),
            ));
        }
        let task_claim_token = optional_text_value(task.get_value(5)?, "tasks.claim_token")?;
        let task_claim_owner = optional_text_value(task.get_value(6)?, "tasks.claim_owner")?;
        if task_claim_token.is_none() || task_claim_owner.is_none() {
            return Err(StoreError::InvalidTransition(
                "submit review requires an active claim".to_owned(),
            ));
        }
        if !input.force {
            if input_claim_token != task_claim_token.as_deref() {
                return Err(StoreError::ClaimTokenMismatch);
            }
            if task_claim_owner.as_deref() != Some(actor.as_str()) {
                return Err(StoreError::InvalidTransition(
                    "claim owner mismatch".to_owned(),
                ));
            }
        }
        if optional_integer_value(task.get_value(7)?, "tasks.claim_expires_at")?.is_none() {
            return Err(StoreError::InvalidTransition(
                "submit review requires an active claim".to_owned(),
            ));
        }
        let run_id = optional_text_value(task.get_value(8)?, "tasks.current_run_id")?
            .filter(|run_id| !run_id.trim().is_empty())
            .ok_or_else(|| {
                StoreError::InvalidTransition(
                    "submit review requires a current running run".to_owned(),
                )
            })?;

        let active_run_count = first_row(
            transaction
                .query(
                    "SELECT COUNT(*) FROM task_runs WHERE board_id = :board_id AND task_id = :task_id AND status = 'running'",
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await?;
        if integer_value(active_run_count.get_value(0)?, "task_runs.active_count")? != 1 {
            return Err(StoreError::InvalidTransition(
                "submit review requires exactly one running run".to_owned(),
            ));
        }

        let run = first_row(
            transaction
                .query(
                    "SELECT status, claim_token, claim_owner FROM task_runs WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id LIMIT 1",
                    [
                        (":run_id", run_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::InvalidTransition(
                "submit review requires a matching running run".to_owned(),
            ),
            other => StoreError::Turso(other),
        })?;
        if text_value(run.get_value(0)?, "task_runs.status")? != "running" {
            return Err(StoreError::InvalidTransition(
                "submit review requires a matching running run".to_owned(),
            ));
        }
        let run_claim_token = text_value(run.get_value(1)?, "task_runs.claim_token")?;
        let run_claim_owner = text_value(run.get_value(2)?, "task_runs.claim_owner")?;
        if task_claim_token.as_deref() != Some(run_claim_token.as_str())
            || task_claim_owner.as_deref() != Some(run_claim_owner.as_str())
        {
            return Err(StoreError::InvalidTransition(
                "active run claim is inconsistent".to_owned(),
            ));
        }

        let changed = transaction
            .execute(
                "UPDATE task_runs SET status = 'succeeded', finished_at = :finished_at, exit_code = 0, summary = COALESCE(:summary, summary) WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id AND status = 'running' AND claim_token = :claim_token AND claim_owner = :claim_owner",
                (
                    (":finished_at", input.now),
                    (":summary", summary),
                    (":run_id", run_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":claim_token", run_claim_token.as_str()),
                    (":claim_owner", run_claim_owner.as_str()),
                ),
            )
            .await?;
        if changed != 1 {
            return Err(StoreError::InvalidTransition(
                "submit review requires a matching running run".to_owned(),
            ));
        }

        let changed = transaction
            .execute(
                "UPDATE tasks SET status = 'review', status_reason = NULL, claim_token = NULL, claim_owner = NULL, claim_expires_at = NULL, last_heartbeat_at = NULL, result_summary = COALESCE(:summary, result_summary), updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = 'running' AND claim_token = :claim_token AND claim_owner = :claim_owner AND current_run_id = :run_id AND lock_version = :expected_lock_version",
                (
                    (":summary", summary),
                    (":updated_at", input.now),
                    (":task_id", task_id),
                    (":board_id", board_id.as_str()),
                    (":claim_token", run_claim_token.as_str()),
                    (":claim_owner", run_claim_owner.as_str()),
                    (":run_id", run_id.as_str()),
                    (":expected_lock_version", input.expected_lock_version),
                ),
            )
            .await?;
        if changed != 1 {
            return Err(StoreError::ClaimConflict(
                "submit review compare-and-set failed".to_owned(),
            ));
        }

        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, :run_id, 'task.submitted_for_review', :actor, '{\"result\":null}', :created_at)",
                (
                    (":event_id", event_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":run_id", run_id.as_str()),
                    (":actor", actor.as_str()),
                    (":created_at", input.now),
                ),
            )
            .await?;

        let reviewed = task_from_row(
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
        Ok(reviewed)
    }

    pub async fn complete_task(
        &self,
        task_id: &str,
        input: CompleteTaskInput,
    ) -> Result<TaskRecord, StoreError> {
        validate_complete_task_input(task_id, &input)?;
        let actor = input.actor.trim().to_owned();
        let input_claim_token = input.claim_token.as_deref();
        let event_id = input.event_id.trim().to_owned();
        let summary = input.summary.as_deref();
        let result_json = input.result_json.as_deref();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        if let Some(result_json) = result_json {
            let valid = first_row(
                transaction
                    .query(
                        "SELECT json_valid(:result_json)",
                        [(":result_json", result_json)],
                    )
                    .await?,
            )
            .await?;
            if integer_value(valid.get_value(0)?, "tasks.result_json_valid")? == 0 {
                return Err(StoreError::InvalidInput(
                    "result_json must be valid JSON".to_owned(),
                ));
            }
        }

        let task = first_row(
            transaction
                .query(
                    "SELECT t.board_id, t.status, t.archived_at, b.archived_at, t.lock_version, t.claim_token, t.claim_owner, t.claim_expires_at, t.current_run_id FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
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
        let task_archived_at = optional_integer_value(task.get_value(2)?, "tasks.archived_at")?;
        let board_archived_at = optional_integer_value(task.get_value(3)?, "boards.archived_at")?;
        if task_archived_at.is_some() || board_archived_at.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived task or board cannot complete".to_owned(),
            ));
        }
        if status != "running" && status != "review" {
            return Err(StoreError::InvalidTransition(
                "complete requires running or review".to_owned(),
            ));
        }

        let lock_version = integer_value(task.get_value(4)?, "tasks.lock_version")?;
        if lock_version != input.expected_lock_version {
            return Err(StoreError::ClaimConflict(
                "lock_version mismatch".to_owned(),
            ));
        }

        let task_claim_token = optional_text_value(task.get_value(5)?, "tasks.claim_token")?;
        let task_claim_owner = optional_text_value(task.get_value(6)?, "tasks.claim_owner")?;
        let task_claim_expires_at =
            optional_integer_value(task.get_value(7)?, "tasks.claim_expires_at")?;
        let run_id = optional_text_value(task.get_value(8)?, "tasks.current_run_id")?;
        let mut run_claim_token = None;
        let mut run_claim_owner = None;
        let active_run_count = first_row(
            transaction
                .query(
                    "SELECT COUNT(*) FROM task_runs WHERE board_id = :board_id AND task_id = :task_id AND status = 'running'",
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await?;
        let active_run_count =
            integer_value(active_run_count.get_value(0)?, "task_runs.active_count")?;

        if status == "running" {
            let run_id = run_id
                .clone()
                .filter(|run_id| !run_id.trim().is_empty())
                .ok_or_else(|| {
                    StoreError::InvalidTransition(
                        "complete requires a current running run".to_owned(),
                    )
                })?;
            if active_run_count != 1 {
                return Err(StoreError::InvalidTransition(
                    "complete requires exactly one running run".to_owned(),
                ));
            }
            let run = first_row(
                transaction
                    .query(
                        "SELECT status, claim_token, claim_owner FROM task_runs WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id LIMIT 1",
                        [
                            (":run_id", run_id.as_str()),
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                        ],
                    )
                    .await?,
            )
            .await
            .map_err(|error| match error {
                turso::Error::QueryReturnedNoRows => StoreError::InvalidTransition(
                    "complete requires a matching running run".to_owned(),
                ),
                other => StoreError::Turso(other),
            })?;
            if text_value(run.get_value(0)?, "task_runs.status")? != "running" {
                return Err(StoreError::InvalidTransition(
                    "complete requires a matching running run".to_owned(),
                ));
            }
            let canonical_run_token = text_value(run.get_value(1)?, "task_runs.claim_token")?;
            let canonical_run_owner = text_value(run.get_value(2)?, "task_runs.claim_owner")?;
            if task_claim_token.as_deref() != Some(canonical_run_token.as_str())
                || task_claim_owner.as_deref() != Some(canonical_run_owner.as_str())
            {
                return Err(StoreError::InvalidTransition(
                    "active run claim is inconsistent".to_owned(),
                ));
            }
            if task_claim_expires_at.is_none() {
                return Err(StoreError::InvalidTransition(
                    "complete requires an active claim".to_owned(),
                ));
            }
            if !input.force {
                if input_claim_token != task_claim_token.as_deref() {
                    return Err(StoreError::ClaimTokenMismatch);
                }
                if task_claim_owner.as_deref() != Some(actor.as_str()) {
                    return Err(StoreError::InvalidTransition(
                        "claim owner mismatch".to_owned(),
                    ));
                }
            }
            run_claim_token = Some(canonical_run_token);
            run_claim_owner = Some(canonical_run_owner);
        } else {
            if active_run_count != 0 {
                return Err(StoreError::InvalidTransition(
                    "review task cannot have an active running run".to_owned(),
                ));
            }
            if let Some(run_id) = run_id.as_deref().filter(|run_id| !run_id.trim().is_empty()) {
                let run = first_row(
                    transaction
                        .query(
                            "SELECT status FROM task_runs WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id LIMIT 1",
                            [
                                (":run_id", run_id),
                                (":board_id", board_id.as_str()),
                                (":task_id", task_id),
                            ],
                        )
                        .await?,
                )
                .await
                .map_err(|error| match error {
                    turso::Error::QueryReturnedNoRows => StoreError::InvalidTransition(
                        "complete requires a succeeded current run".to_owned(),
                    ),
                    other => StoreError::Turso(other),
                })?;
                if text_value(run.get_value(0)?, "task_runs.status")? != "succeeded" {
                    return Err(StoreError::InvalidTransition(
                        "complete requires a succeeded current run".to_owned(),
                    ));
                }
            }
        }

        let incomplete_steps = first_row(
            transaction
                .query(
                    "SELECT COUNT(*) FROM task_steps WHERE board_id = :board_id AND parent_task_id = :task_id AND required = 1 AND status NOT IN ('done', 'skipped')",
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?,
        )
        .await?;
        let incomplete_steps = integer_value(
            incomplete_steps.get_value(0)?,
            "task_steps.incomplete_required_count",
        )?;
        if incomplete_steps != 0 {
            return Err(StoreError::StepsIncomplete(format!(
                "{incomplete_steps} required step(s) incomplete"
            )));
        }

        if let Some(run_id) = run_id.as_deref().filter(|run_id| !run_id.trim().is_empty())
            && let (Some(run_claim_token), Some(run_claim_owner)) =
                (run_claim_token.as_deref(), run_claim_owner.as_deref())
        {
            let changed = transaction
                .execute(
                    "UPDATE task_runs SET status = 'succeeded', finished_at = :finished_at, exit_code = 0, error = NULL, summary = COALESCE(:summary, summary) WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id AND status = 'running' AND claim_token = :claim_token AND claim_owner = :claim_owner",
                    (
                        (":finished_at", input.now),
                        (":summary", summary),
                        (":run_id", run_id),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":claim_token", run_claim_token),
                        (":claim_owner", run_claim_owner),
                    ),
                )
                .await?;
            if changed != 1 {
                return Err(StoreError::InvalidTransition(
                    "complete requires a matching running run".to_owned(),
                ));
            }
        }

        let changed = transaction
            .execute(
                "UPDATE tasks SET status = 'done', status_reason = NULL, completed_at = :completed_at, claim_token = NULL, claim_owner = NULL, claim_expires_at = NULL, last_heartbeat_at = NULL, result_summary = COALESCE(:summary, result_summary), result_json = COALESCE(:result_json, result_json), updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = :source_status AND lock_version = :expected_lock_version",
                (
                    (":completed_at", input.now),
                    (":summary", summary),
                    (":result_json", result_json),
                    (":updated_at", input.now),
                    (":task_id", task_id),
                    (":board_id", board_id.as_str()),
                    (":source_status", status.as_str()),
                    (":expected_lock_version", input.expected_lock_version),
                ),
            )
            .await?;
        if changed != 1 {
            return Err(StoreError::ClaimConflict(
                "complete compare-and-set failed".to_owned(),
            ));
        }

        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, :run_id, 'task.completed', :actor, json_object('result', json(:result_json)), :created_at)",
                (
                    (":event_id", event_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":run_id", run_id.as_deref()),
                    (":actor", actor.as_str()),
                    (":result_json", result_json),
                    (":created_at", input.now),
                ),
            )
            .await?;

        let completed = task_from_row(
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
        Ok(completed)
    }

    pub async fn block_task(
        &self,
        task_id: &str,
        input: BlockTaskInput,
    ) -> Result<TaskRecord, StoreError> {
        validate_block_task_input(task_id, &input)?;
        let actor = input.actor.trim().to_owned();
        let reason = input.reason.as_str();
        let input_claim_token = input.claim_token.as_deref();
        let event_id = input.event_id.trim().to_owned();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let task = first_row(
            transaction
                .query(
                    "SELECT t.board_id, t.status, t.archived_at, b.archived_at, t.lock_version, t.claim_token, t.claim_owner, t.claim_expires_at, t.current_run_id FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
                    [ (":task_id", task_id) ],
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
        let task_archived_at = optional_integer_value(task.get_value(2)?, "tasks.archived_at")?;
        let board_archived_at = optional_integer_value(task.get_value(3)?, "boards.archived_at")?;
        if task_archived_at.is_some() || board_archived_at.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived task or board cannot be blocked".to_owned(),
            ));
        }
        if !matches!(
            status.as_str(),
            "triage" | "todo" | "scheduled" | "ready" | "running" | "review"
        ) {
            return Err(StoreError::InvalidTransition(
                "cannot block task".to_owned(),
            ));
        }

        let lock_version = integer_value(task.get_value(4)?, "tasks.lock_version")?;
        if lock_version != input.expected_lock_version {
            return Err(StoreError::ClaimConflict(
                "lock_version mismatch".to_owned(),
            ));
        }

        let task_claim_token = optional_text_value(task.get_value(5)?, "tasks.claim_token")?;
        let task_claim_owner = optional_text_value(task.get_value(6)?, "tasks.claim_owner")?;
        let task_claim_expires_at =
            optional_integer_value(task.get_value(7)?, "tasks.claim_expires_at")?;
        let run_id = optional_text_value(task.get_value(8)?, "tasks.current_run_id")?;

        if status == "running" {
            let run_id = run_id
                .clone()
                .filter(|run_id| !run_id.trim().is_empty())
                .ok_or_else(|| {
                    StoreError::InvalidTransition("block requires a current running run".to_owned())
                })?;
            if task_claim_expires_at.is_none() {
                return Err(StoreError::InvalidTransition(
                    "block requires an active claim".to_owned(),
                ));
            }
            let active_run_count = first_row(
                transaction
                    .query(
                        "SELECT COUNT(*) FROM task_runs WHERE board_id = :board_id AND task_id = :task_id AND status = 'running'",
                        [
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                        ],
                    )
                    .await?,
            )
            .await?;
            if integer_value(active_run_count.get_value(0)?, "task_runs.active_count")? != 1 {
                return Err(StoreError::InvalidTransition(
                    "block requires exactly one running run".to_owned(),
                ));
            }

            let run = first_row(
                transaction
                    .query(
                        "SELECT status, claim_token, claim_owner FROM task_runs WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id LIMIT 1",
                        [
                            (":run_id", run_id.as_str()),
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                        ],
                    )
                    .await?,
            )
            .await
            .map_err(|error| match error {
                turso::Error::QueryReturnedNoRows => StoreError::InvalidTransition(
                    "block requires a matching running run".to_owned(),
                ),
                other => StoreError::Turso(other),
            })?;
            if text_value(run.get_value(0)?, "task_runs.status")? != "running" {
                return Err(StoreError::InvalidTransition(
                    "block requires a matching running run".to_owned(),
                ));
            }
            let run_claim_token = text_value(run.get_value(1)?, "task_runs.claim_token")?;
            let run_claim_owner = text_value(run.get_value(2)?, "task_runs.claim_owner")?;
            if task_claim_token.as_deref() != Some(run_claim_token.as_str())
                || task_claim_owner.as_deref() != Some(run_claim_owner.as_str())
            {
                return Err(StoreError::InvalidTransition(
                    "active run claim is inconsistent".to_owned(),
                ));
            }
            if !input.force {
                if input_claim_token != task_claim_token.as_deref() {
                    return Err(StoreError::ClaimTokenMismatch);
                }
                if task_claim_owner.as_deref() != Some(actor.as_str()) {
                    return Err(StoreError::InvalidTransition(
                        "claim owner mismatch".to_owned(),
                    ));
                }
            }

            let changed = transaction
                .execute(
                    "UPDATE task_runs SET status = 'failed', finished_at = :finished_at, exit_code = 1, error = :error WHERE id = :run_id AND board_id = :board_id AND task_id = :task_id AND status = 'running' AND claim_token = :claim_token AND claim_owner = :claim_owner",
                    (
                        (":finished_at", input.now),
                        (":error", reason),
                        (":run_id", run_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":claim_token", task_claim_token.as_deref()),
                        (":claim_owner", task_claim_owner.as_deref()),
                    ),
                )
                .await?;
            if changed != 1 {
                return Err(StoreError::InvalidTransition(
                    "block requires a matching running run".to_owned(),
                ));
            }

            let changed = transaction
                .execute(
                    "UPDATE tasks SET status = 'blocked', status_reason = :status_reason, claim_token = NULL, claim_owner = NULL, claim_expires_at = NULL, last_heartbeat_at = NULL, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = 'running' AND claim_token = :claim_token AND claim_owner = :claim_owner AND current_run_id = :run_id AND lock_version = :expected_lock_version",
                    (
                        (":status_reason", reason),
                        (":updated_at", input.now),
                        (":task_id", task_id),
                        (":board_id", board_id.as_str()),
                        (":claim_token", task_claim_token.as_deref()),
                        (":claim_owner", task_claim_owner.as_deref()),
                        (":run_id", run_id.as_str()),
                        (":expected_lock_version", input.expected_lock_version),
                    ),
                )
                .await?;
            if changed != 1 {
                return Err(StoreError::ClaimConflict(
                    "block compare-and-set failed".to_owned(),
                ));
            }

            transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, :run_id, 'task.blocked', :actor, json_object('reason', :reason), :created_at)",
                    (
                        (":event_id", event_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":run_id", run_id.as_str()),
                        (":actor", actor.as_str()),
                        (":reason", reason),
                        (":created_at", input.now),
                    ),
                )
                .await?;
        } else {
            let active_run_count = first_row(
                transaction
                    .query(
                        "SELECT COUNT(*) FROM task_runs WHERE board_id = :board_id AND task_id = :task_id AND status = 'running'",
                        [
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                        ],
                    )
                    .await?,
            )
            .await?;
            if integer_value(active_run_count.get_value(0)?, "task_runs.active_count")? != 0 {
                return Err(StoreError::InvalidTransition(
                    "block requires no active running run".to_owned(),
                ));
            }

            let changed = transaction
                .execute(
                    "UPDATE tasks SET status = 'blocked', status_reason = :status_reason, claim_token = NULL, claim_owner = NULL, claim_expires_at = NULL, last_heartbeat_at = NULL, updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = :source_status AND lock_version = :expected_lock_version",
                    (
                        (":status_reason", reason),
                        (":updated_at", input.now),
                        (":task_id", task_id),
                        (":board_id", board_id.as_str()),
                        (":source_status", status.as_str()),
                        (":expected_lock_version", input.expected_lock_version),
                    ),
                )
                .await?;
            if changed != 1 {
                return Err(StoreError::ClaimConflict(
                    "block compare-and-set failed".to_owned(),
                ));
            }

            transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.blocked', :actor, json_object('reason', :reason), :created_at)",
                    (
                        (":event_id", event_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":actor", actor.as_str()),
                        (":reason", reason),
                        (":created_at", input.now),
                    ),
                )
                .await?;
        }

        let blocked = task_from_row(
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
        Ok(blocked)
    }

    pub async fn list_boards(
        &self,
        include_archived: bool,
    ) -> Result<Vec<BoardRecord>, StoreError> {
        let connection = self.connection().await?;
        let sql = if include_archived {
            "SELECT id, slug, name, description, created_at, updated_at, archived_at FROM boards ORDER BY archived_at IS NOT NULL ASC, slug ASC, id ASC"
        } else {
            "SELECT id, slug, name, description, created_at, updated_at, archived_at FROM boards WHERE archived_at IS NULL ORDER BY slug ASC, id ASC"
        };
        let mut rows = connection.query(sql, ()).await?;
        let mut boards = Vec::new();
        while let Some(row) = rows.next().await? {
            boards.push(BoardRecord {
                id: text_value(row.get_value(0)?, "boards.id")?,
                slug: text_value(row.get_value(1)?, "boards.slug")?,
                name: text_value(row.get_value(2)?, "boards.name")?,
                description: optional_text_value(row.get_value(3)?, "boards.description")?,
                created_at: integer_value(row.get_value(4)?, "boards.created_at")?,
                updated_at: integer_value(row.get_value(5)?, "boards.updated_at")?,
                archived_at: optional_integer_value(row.get_value(6)?, "boards.archived_at")?,
            });
        }
        Ok(boards)
    }

    pub async fn list_board_columns(
        &self,
        selector: &str,
    ) -> Result<Vec<BoardColumnRecord>, StoreError> {
        let connection = self.connection().await?;
        let board = first_row(
            connection
                .query(
                    "SELECT id FROM boards WHERE id = ?1 OR slug = ?1 LIMIT 1",
                    [selector],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::BoardNotFound(selector.to_owned()),
            other => StoreError::Turso(other),
        })?;
        let board_id = text_value(board.get_value(0)?, "boards.id")?;
        let mut rows = connection
            .query(
                "SELECT id, board_id, status, title, position, hidden, wip_limit, created_at, updated_at FROM board_columns WHERE board_id = ?1 ORDER BY position ASC, id ASC",
                [board_id.as_str()],
            )
            .await?;
        let mut columns = Vec::new();
        while let Some(row) = rows.next().await? {
            columns.push(BoardColumnRecord {
                id: text_value(row.get_value(0)?, "board_columns.id")?,
                board_id: text_value(row.get_value(1)?, "board_columns.board_id")?,
                status: text_value(row.get_value(2)?, "board_columns.status")?,
                title: text_value(row.get_value(3)?, "board_columns.title")?,
                position: integer_value(row.get_value(4)?, "board_columns.position")?,
                hidden: integer_value(row.get_value(5)?, "board_columns.hidden")? != 0,
                wip_limit: optional_integer_value(row.get_value(6)?, "board_columns.wip_limit")?,
                created_at: integer_value(row.get_value(7)?, "board_columns.created_at")?,
                updated_at: integer_value(row.get_value(8)?, "board_columns.updated_at")?,
            });
        }
        Ok(columns)
    }

    async fn connection(&self) -> Result<Connection, StoreError> {
        let connection = self.database.connect()?;
        connection.execute("PRAGMA foreign_keys = ON", ()).await?;
        Ok(connection)
    }
}

async fn dependency_task_in_transaction(
    transaction: &Transaction<'_>,
    task_id: &str,
) -> Result<TaskRecord, StoreError> {
    let row = first_row(
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
    task_from_row(row)
}

async fn dependency_task_in_connection(
    connection: &Connection,
    task_id: &str,
) -> Result<TaskRecord, StoreError> {
    let row = first_row(
        connection
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
    task_from_row(row)
}

async fn dependency_path_exists(
    transaction: &Transaction<'_>,
    board_id: &str,
    start_task_id: &str,
    target_task_id: &str,
) -> Result<bool, StoreError> {
    // Turso 0.7.x does not implement recursive CTEs. Walk the direct edge
    // relation inside the same immediate transaction instead; the transaction
    // still gives the traversal a stable view and keeps the subsequent insert
    // atomic with the cycle check.
    let mut frontier = vec![start_task_id.to_owned()];
    let mut visited = HashSet::from([start_task_id.to_owned()]);
    while let Some(parent_task_id) = frontier.pop() {
        let mut rows = transaction
            .query(
                "SELECT child_task_id FROM task_dependencies WHERE board_id = :board_id AND parent_task_id = :parent_task_id",
                [
                    (":board_id", board_id),
                    (":parent_task_id", parent_task_id.as_str()),
                ],
            )
            .await?;
        while let Some(row) = rows.next().await? {
            let child_task_id = text_value(row.get_value(0)?, "task_dependencies.child_task_id")?;
            if child_task_id == target_task_id {
                return Ok(true);
            }
            if visited.insert(child_task_id.clone()) {
                frontier.push(child_task_id);
            }
        }
    }
    Ok(false)
}

fn dependency_parent_satisfied(parent: &TaskRecord) -> bool {
    matches!(parent.status.as_str(), "done" | "archived") || parent.archived_at.is_some()
}

async fn dependency_snapshot_in_transaction(
    transaction: &Transaction<'_>,
    board_id: &str,
    task_id: &str,
) -> Result<DependencySnapshotRecord, StoreError> {
    let task = dependency_task_in_transaction(transaction, task_id).await?;
    let mut rows = transaction
        .query(
            "SELECT parent_task_id, child_task_id FROM task_dependencies WHERE board_id = :board_id AND (parent_task_id = :task_id OR child_task_id = :task_id) ORDER BY created_at ASC, parent_task_id ASC, child_task_id ASC",
            [(":board_id", board_id), (":task_id", task_id)],
        )
        .await?;
    let mut edges = Vec::new();
    let mut parents = Vec::new();
    let mut children = Vec::new();
    while let Some(row) = rows.next().await? {
        let parent_id = text_value(row.get_value(0)?, "task_dependencies.parent_task_id")?;
        let child_id = text_value(row.get_value(1)?, "task_dependencies.child_task_id")?;
        let parent = dependency_task_in_transaction(transaction, &parent_id).await?;
        let child = dependency_task_in_transaction(transaction, &child_id).await?;
        if child_id == task_id {
            parents.push(parent.clone());
        }
        if parent_id == task_id {
            children.push(child.clone());
        }
        edges.push(DependencyEdgeRecord { parent, child });
    }
    Ok(DependencySnapshotRecord {
        task,
        parents,
        children,
        edges,
    })
}

async fn dependency_snapshot_in_connection(
    connection: &Connection,
    board_id: &str,
    task_id: &str,
) -> Result<DependencySnapshotRecord, StoreError> {
    let task = dependency_task_in_connection(connection, task_id).await?;
    let mut rows = connection
        .query(
            "SELECT parent_task_id, child_task_id FROM task_dependencies WHERE board_id = :board_id AND (parent_task_id = :task_id OR child_task_id = :task_id) ORDER BY created_at ASC, parent_task_id ASC, child_task_id ASC",
            [(":board_id", board_id), (":task_id", task_id)],
        )
        .await?;
    let mut edges = Vec::new();
    let mut parents = Vec::new();
    let mut children = Vec::new();
    while let Some(row) = rows.next().await? {
        let parent_id = text_value(row.get_value(0)?, "task_dependencies.parent_task_id")?;
        let child_id = text_value(row.get_value(1)?, "task_dependencies.child_task_id")?;
        let parent = dependency_task_in_connection(connection, &parent_id).await?;
        let child = dependency_task_in_connection(connection, &child_id).await?;
        if child_id == task_id {
            parents.push(parent.clone());
        }
        if parent_id == task_id {
            children.push(child.clone());
        }
        edges.push(DependencyEdgeRecord { parent, child });
    }
    Ok(DependencySnapshotRecord {
        task,
        parents,
        children,
        edges,
    })
}

async fn first_row(mut rows: Rows) -> Result<Row, turso::Error> {
    let row = rows
        .next()
        .await?
        .ok_or(turso::Error::QueryReturnedNoRows)?;
    while rows.next().await?.is_some() {}
    Ok(row)
}

const TASK_FROM: &str = "FROM tasks AS t JOIN boards AS b ON b.id = t.board_id";
const TASK_SELECT: &str = "SELECT t.id, t.board_id, t.seq, t.idempotency_key, t.title, t.description, t.status, t.status_reason, t.assignee, t.priority, t.position, t.scheduled_at, t.due_at, t.created_by, t.created_at, t.updated_at, t.started_at, t.completed_at, t.archived_at, t.claim_token, t.claim_owner, t.claim_expires_at, t.last_heartbeat_at, t.current_run_id, t.retry_count, t.max_retries, t.result_summary, t.result_json, t.metadata_json, t.lock_version, b.slug, EXISTS (SELECT 1 FROM task_dependencies AS d JOIN tasks AS p ON p.id = d.parent_task_id AND p.board_id = d.board_id WHERE d.board_id = t.board_id AND d.child_task_id = t.id AND p.status NOT IN ('done', 'archived')) AS dependency_blocked, (SELECT COUNT(*) FROM task_dependencies AS d JOIN tasks AS p ON p.id = d.parent_task_id AND p.board_id = d.board_id WHERE d.board_id = t.board_id AND d.child_task_id = t.id AND p.status NOT IN ('done', 'archived')) AS unfinished_parent_count, CASE WHEN EXISTS (SELECT 1 FROM task_steps AS s WHERE s.board_id = t.board_id AND s.parent_task_id = t.id) THEN 'planned' WHEN EXISTS (SELECT 1 FROM task_execution_plans AS ep WHERE ep.board_id = t.board_id AND ep.task_id = t.id AND ep.state = 'not_required') THEN 'not_required' ELSE 'unplanned' END AS execution_plan_state, (SELECT COUNT(*) FROM task_steps AS s WHERE s.board_id = t.board_id AND s.parent_task_id = t.id AND s.required = 1) AS required_step_count, (SELECT COUNT(*) FROM task_steps AS s WHERE s.board_id = t.board_id AND s.parent_task_id = t.id AND s.required = 1 AND s.status IN ('done', 'skipped')) AS completed_required_step_count, (SELECT COUNT(*) FROM task_steps AS s WHERE s.board_id = t.board_id AND s.parent_task_id = t.id AND s.required = 0) AS optional_step_count FROM tasks AS t JOIN boards AS b ON b.id = t.board_id";

fn validate_create_task_input(input: &CreateTaskInput) -> Result<(), StoreError> {
    if !input.id.starts_with("t_") || input.id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    if input.title.trim().is_empty() {
        return Err(StoreError::InvalidInput("title is required".to_owned()));
    }
    if !matches!(input.status.as_str(), "triage" | "todo" | "scheduled") {
        return Err(StoreError::InvalidInput(
            "status must be triage, todo, or scheduled".to_owned(),
        ));
    }
    if !(0..=3).contains(&input.priority) {
        return Err(StoreError::InvalidInput(
            "priority must be between 0 and 3".to_owned(),
        ));
    }
    if input.max_retries.is_some_and(|value| value < 0) {
        return Err(StoreError::InvalidInput(
            "max_retries must be non-negative".to_owned(),
        ));
    }
    if input.created_by.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "created_by is required".to_owned(),
        ));
    }
    if input
        .idempotency_key
        .as_deref()
        .is_some_and(|key| key.trim().is_empty())
    {
        return Err(StoreError::InvalidInput(
            "idempotency_key must not be empty".to_owned(),
        ));
    }
    Ok(())
}

fn validate_create_comment_input(
    task_id: &str,
    input: &CreateCommentInput,
) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    if !input.id.trim().starts_with("c_") || input.id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "comment id must start with c_".to_owned(),
        ));
    }
    if input
        .idempotency_key
        .as_deref()
        .is_some_and(|key| key.trim().is_empty())
    {
        return Err(StoreError::InvalidInput(
            "idempotency_key must not be empty".to_owned(),
        ));
    }
    if input.author.trim().is_empty() {
        return Err(StoreError::InvalidInput("author is required".to_owned()));
    }
    if !matches!(input.author_type.trim(), "user" | "agent") {
        return Err(StoreError::InvalidInput(
            "author_type must be user or agent".to_owned(),
        ));
    }
    if input.agent_type.as_deref().is_some_and(|agent_type| {
        !agent_type.trim().is_empty() && input.author_type.trim() != "agent"
    }) {
        return Err(StoreError::InvalidInput(
            "agent_type is only allowed when author_type is agent".to_owned(),
        ));
    }
    if input.body.trim().is_empty() {
        return Err(StoreError::InvalidInput("body is required".to_owned()));
    }
    if !matches!(input.kind.trim(), "note" | "decision") {
        return Err(StoreError::InvalidInput(
            "kind must be note or decision".to_owned(),
        ));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id must start with e_".to_owned(),
        ));
    }
    if input.created_at < 0 {
        return Err(StoreError::InvalidInput(
            "created_at must be non-negative".to_owned(),
        ));
    }
    Ok(())
}

fn validate_add_dependency_input(
    child_task_id: &str,
    parent_task_id: &str,
    input: &AddDependencyInput,
) -> Result<(), StoreError> {
    validate_task_id(child_task_id)?;
    validate_task_id(parent_task_id)?;
    if child_task_id.trim() == parent_task_id.trim() {
        return Err(StoreError::InvalidInput(
            "dependency cannot point to itself".to_owned(),
        ));
    }
    if input.expected_child_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_child_lock_version must be non-negative".to_owned(),
        ));
    }
    if !matches!(
        input.target_child_status.trim(),
        "triage" | "todo" | "scheduled" | "ready" | "running" | "blocked" | "review" | "done"
    ) {
        return Err(StoreError::InvalidInput(
            "target_child_status is invalid".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    for (name, value) in [
        ("event_id", input.event_id.as_str()),
        ("recompute_event_id", input.recompute_event_id.as_str()),
    ] {
        if !value.trim().starts_with("e_") || value.trim().len() <= 2 {
            return Err(StoreError::InvalidInput(format!(
                "{name} must start with e_"
            )));
        }
    }
    if input.now < 0 {
        return Err(StoreError::InvalidInput(
            "now must be non-negative".to_owned(),
        ));
    }
    Ok(())
}

fn validate_remove_dependency_input(
    child_task_id: &str,
    parent_task_id: &str,
    input: &RemoveDependencyInput,
) -> Result<(), StoreError> {
    validate_task_id(child_task_id)?;
    validate_task_id(parent_task_id)?;
    if child_task_id.trim() == parent_task_id.trim() {
        return Err(StoreError::InvalidInput(
            "dependency cannot point to itself".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id must start with e_".to_owned(),
        ));
    }
    if input.now < 0 {
        return Err(StoreError::InvalidInput(
            "now must be non-negative".to_owned(),
        ));
    }
    Ok(())
}

fn validate_task_id(task_id: &str) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    Ok(())
}

fn validate_create_step_input(task_id: &str, input: &CreateStepInput) -> Result<(), StoreError> {
    validate_task_id(task_id)?;
    if !input.id.trim().starts_with("step_") || input.id.trim().len() <= 5 {
        return Err(StoreError::InvalidInput(
            "step id must start with step_".to_owned(),
        ));
    }
    if input.title.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "step title is required".to_owned(),
        ));
    }
    if input
        .idempotency_key
        .as_deref()
        .is_some_and(|key| key.trim().is_empty())
    {
        return Err(StoreError::InvalidInput(
            "idempotency_key must not be empty".to_owned(),
        ));
    }
    if input.position.is_some_and(|position| position < 0) {
        return Err(StoreError::InvalidInput(
            "step position must be non-negative".to_owned(),
        ));
    }
    if input.created_by.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "created_by is required".to_owned(),
        ));
    }
    if input.expected_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_lock_version must be non-negative".to_owned(),
        ));
    }
    if !matches!(
        input.expected_plan_state.trim(),
        "unplanned" | "planned" | "not_required"
    ) {
        return Err(StoreError::InvalidInput(
            "expected_plan_state is invalid".to_owned(),
        ));
    }
    if !matches!(
        input.target_status.trim(),
        "triage" | "todo" | "scheduled" | "ready" | "running" | "blocked" | "review"
    ) {
        return Err(StoreError::InvalidInput(
            "target_status is invalid".to_owned(),
        ));
    }
    for (name, value) in [
        ("event_id", input.event_id.as_str()),
        ("plan_event_id", input.plan_event_id.as_str()),
        ("recompute_event_id", input.recompute_event_id.as_str()),
    ] {
        if !value.trim().starts_with("e_") || value.trim().len() <= 2 {
            return Err(StoreError::InvalidInput(format!(
                "{name} must start with e_"
            )));
        }
    }
    if input.created_at < 0 {
        return Err(StoreError::InvalidInput(
            "created_at must be non-negative".to_owned(),
        ));
    }
    Ok(())
}

fn validate_update_step_input(
    task_id: &str,
    step_id: &str,
    input: &UpdateStepInput,
) -> Result<(), StoreError> {
    validate_task_id(task_id)?;
    if !step_id.trim().starts_with("step_") || step_id.trim().len() <= 5 {
        return Err(StoreError::InvalidInput(
            "step id must start with step_".to_owned(),
        ));
    }
    if input
        .title
        .as_deref()
        .is_some_and(|title| title.trim().is_empty())
    {
        return Err(StoreError::InvalidInput(
            "step title is required when provided".to_owned(),
        ));
    }
    if input.position.is_some_and(|position| position < 0) {
        return Err(StoreError::InvalidInput(
            "step position must be non-negative".to_owned(),
        ));
    }
    if input.linked_task_id.is_some() && input.unlink_task {
        return Err(StoreError::InvalidInput(
            "linked_task_ref and unlink_task cannot be used together".to_owned(),
        ));
    }
    if input.title.is_none()
        && input.body.is_none()
        && input.linked_task_id.is_none()
        && !input.unlink_task
        && input.position.is_none()
        && input.required.is_none()
    {
        return Err(StoreError::InvalidInput(
            "step update requires at least one field".to_owned(),
        ));
    }
    if input.updated_by.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "updated_by is required".to_owned(),
        ));
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
    if input.expected_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_lock_version must be non-negative".to_owned(),
        ));
    }
    Ok(())
}

fn canonical_ready_status(
    title: &str,
    description: Option<&str>,
    scheduled_at: Option<i64>,
    dependencies_done: bool,
    now: i64,
) -> &'static str {
    if title.trim().is_empty() || description.is_none_or(|value| value.trim().is_empty()) {
        return "triage";
    }
    if scheduled_at.is_some_and(|scheduled| scheduled > now) {
        return "scheduled";
    }
    if !dependencies_done {
        return "todo";
    }
    "ready"
}

fn step_payload_matches(
    existing: &TaskStepRecord,
    title: &str,
    body: Option<&str>,
    linked_task_id: Option<&str>,
    position: i64,
    required: bool,
    created_by: &str,
) -> bool {
    existing.title == title
        && existing.body.as_deref() == body
        && existing.linked_task.as_ref().map(|task| task.id.as_str()) == linked_task_id
        && existing.position == position
        && existing.required == required
        && existing.created_by == created_by.trim()
}

fn canonical_payload_matches(
    existing: &TaskRecord,
    input: &CreateTaskInput,
    canonical_title: &str,
) -> bool {
    existing.status == input.status
        && existing.title == canonical_title
        && existing.description == input.description
        && existing.assignee == input.assignee
        && existing.priority == input.priority
        && existing.scheduled_at == input.scheduled_at
        && existing.due_at == input.due_at
        && existing.max_retries == input.max_retries
        && existing.metadata_json == input.metadata_json
        && existing.created_by == input.created_by
}

#[allow(clippy::too_many_arguments)]
fn comment_payload_matches(
    existing: &CommentRecord,
    idempotency_key: &str,
    author: &str,
    author_type: &str,
    agent_type: Option<&str>,
    body: &str,
    kind: &str,
    metadata_json: &str,
) -> bool {
    existing.idempotency_key.as_deref() == Some(idempotency_key)
        && existing.author == author
        && existing.author_type == author_type
        && existing.agent_type.as_deref() == agent_type
        && existing.body == body
        && existing.kind == kind
        && existing.metadata_json == metadata_json
}

fn validate_task_list_options(options: &TaskListOptions) -> Result<(), StoreError> {
    if options.limit > 1000 {
        return Err(StoreError::InvalidInput("limit must be <= 1000".to_owned()));
    }
    if i64::try_from(options.offset).is_err() {
        return Err(StoreError::InvalidInput("offset is too large".to_owned()));
    }
    for status in &options.statuses {
        if !matches!(
            status.as_str(),
            "triage"
                | "todo"
                | "scheduled"
                | "ready"
                | "running"
                | "blocked"
                | "review"
                | "done"
                | "archived"
        ) {
            return Err(StoreError::InvalidInput(format!(
                "unknown task status: {status}"
            )));
        }
    }
    Ok(())
}

fn task_list_where(
    board_id: &str,
    board_slug: &str,
    options: &TaskListOptions,
) -> (String, Vec<(String, Value)>) {
    let mut clauses = vec!["t.board_id = :board_id".to_owned()];
    let mut params = vec![(":board_id".to_owned(), Value::Text(board_id.to_owned()))];

    if !options.include_archived {
        clauses.push("t.status != 'archived'".to_owned());
    }
    if !options.statuses.is_empty() {
        let names = options
            .statuses
            .iter()
            .enumerate()
            .map(|(index, _)| format!(":status_{index}"))
            .collect::<Vec<_>>();
        clauses.push(format!("t.status IN ({})", names.join(", ")));
        params.extend(
            options.statuses.iter().enumerate().map(|(index, status)| {
                (format!(":status_{index}"), Value::Text(status.to_owned()))
            }),
        );
    }
    if !options.priorities.is_empty() {
        let names = options
            .priorities
            .iter()
            .enumerate()
            .map(|(index, _)| format!(":priority_{index}"))
            .collect::<Vec<_>>();
        clauses.push(format!("t.priority IN ({})", names.join(", ")));
        params.extend(
            options
                .priorities
                .iter()
                .enumerate()
                .map(|(index, priority)| (format!(":priority_{index}"), Value::Integer(*priority))),
        );
    }

    for filter in &options.plan_filters {
        let clause = match filter {
            TaskPlanFilter::PlanNeeded => {
                "t.status NOT IN ('done', 'archived') AND NOT EXISTS (SELECT 1 FROM task_steps AS s WHERE s.board_id = t.board_id AND s.parent_task_id = t.id) AND NOT EXISTS (SELECT 1 FROM task_execution_plans AS ep WHERE ep.board_id = t.board_id AND ep.task_id = t.id AND ep.state = 'not_required')"
            }
            TaskPlanFilter::HasSteps => {
                "EXISTS (SELECT 1 FROM task_steps AS s WHERE s.board_id = t.board_id AND s.parent_task_id = t.id)"
            }
            TaskPlanFilter::IncompleteRequiredSteps => {
                "EXISTS (SELECT 1 FROM task_steps AS s WHERE s.board_id = t.board_id AND s.parent_task_id = t.id AND s.required = 1 AND s.status NOT IN ('done', 'skipped'))"
            }
        };
        clauses.push(format!("({clause})"));
    }
    if let Some(assignee) = options
        .assignee
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        clauses.push("t.assignee = :assignee".to_owned());
        params.push((":assignee".to_owned(), Value::Text(assignee.to_owned())));
    }
    if let Some(q) = options
        .q
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        append_task_search_filter(&mut clauses, &mut params, board_id, board_slug, q);
    }

    (format!("WHERE {}", clauses.join(" AND ")), params)
}

fn append_task_search_filter(
    clauses: &mut Vec<String>,
    params: &mut Vec<(String, Value)>,
    board_id: &str,
    board_slug: &str,
    query: &str,
) {
    if query.starts_with("t_") {
        clauses.push("t.id = :q_task_id".to_owned());
        params.push((":q_task_id".to_owned(), Value::Text(query.to_owned())));
        return;
    }

    if query.starts_with('#') {
        let Some(seq) = parse_task_seq(query) else {
            clauses.push("0 = 1".to_owned());
            return;
        };
        clauses.push("t.seq = :q_seq".to_owned());
        params.push((":q_seq".to_owned(), Value::Integer(seq)));
        return;
    }

    if let Some((board_ref, seq_ref)) = query.split_once('#') {
        if board_ref.is_empty() || seq_ref.is_empty() {
            clauses.push("0 = 1".to_owned());
            return;
        }
        let Some(seq) = parse_task_seq(seq_ref) else {
            clauses.push("0 = 1".to_owned());
            return;
        };
        if board_ref != board_id && board_ref != board_slug {
            clauses.push("0 = 1".to_owned());
            return;
        }
        clauses.push("t.seq = :q_seq".to_owned());
        params.push((":q_seq".to_owned(), Value::Integer(seq)));
        return;
    }

    if query.chars().all(|character| character.is_ascii_digit()) {
        let Some(seq) = parse_task_seq(query) else {
            clauses.push("0 = 1".to_owned());
            return;
        };
        clauses.push("t.seq = :q_seq".to_owned());
        params.push((":q_seq".to_owned(), Value::Integer(seq)));
        return;
    }

    let needle = format!("%{}%", sqlite_like_literal(&query.to_lowercase()));
    clauses.push(
        "(lower(t.title) LIKE :q_text ESCAPE '\\' OR lower(COALESCE(t.description, '')) LIKE :q_text ESCAPE '\\')"
            .to_owned(),
    );
    params.push((":q_text".to_owned(), Value::Text(needle)));
}

fn parse_task_seq(value: &str) -> Option<i64> {
    let value = value.strip_prefix('#').unwrap_or(value);
    if value.is_empty() || !value.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn sqlite_like_literal(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '%' | '_' | '\\' => {
                escaped.push('\\');
                escaped.push(character);
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

fn task_order_by(sort: TaskListSort) -> &'static str {
    match sort {
        TaskListSort::Seq => "t.seq ASC, t.id ASC",
        TaskListSort::SeqDesc => "t.seq DESC, t.id DESC",
        TaskListSort::Title => "lower(t.title) ASC, t.seq ASC, t.id ASC",
        TaskListSort::TitleDesc => "lower(t.title) DESC, t.seq DESC, t.id DESC",
        TaskListSort::Status => {
            "CASE t.status WHEN 'triage' THEN 10 WHEN 'todo' THEN 20 WHEN 'scheduled' THEN 30 WHEN 'ready' THEN 40 WHEN 'running' THEN 50 WHEN 'blocked' THEN 60 WHEN 'review' THEN 70 WHEN 'done' THEN 80 ELSE 90 END ASC, t.position ASC, t.seq ASC, t.id ASC"
        }
        TaskListSort::StatusDesc => {
            "CASE t.status WHEN 'triage' THEN 10 WHEN 'todo' THEN 20 WHEN 'scheduled' THEN 30 WHEN 'ready' THEN 40 WHEN 'running' THEN 50 WHEN 'blocked' THEN 60 WHEN 'review' THEN 70 WHEN 'done' THEN 80 ELSE 90 END DESC, t.position DESC, t.seq DESC, t.id DESC"
        }
        TaskListSort::Position => "t.position ASC, t.created_at ASC, t.seq ASC, t.id ASC",
        TaskListSort::PositionDesc => "t.position DESC, t.created_at DESC, t.seq DESC, t.id DESC",
        TaskListSort::Priority => "t.priority ASC, t.created_at ASC, t.seq ASC, t.id ASC",
        TaskListSort::PriorityDesc => "t.priority DESC, t.created_at DESC, t.seq DESC, t.id DESC",
        TaskListSort::Assignee => {
            "COALESCE(t.assignee, t.claim_owner, '') ASC, t.seq ASC, t.id ASC"
        }
        TaskListSort::AssigneeDesc => {
            "COALESCE(t.assignee, t.claim_owner, '') DESC, t.seq DESC, t.id DESC"
        }
        TaskListSort::ScheduledAt => {
            "COALESCE(t.scheduled_at, 9223372036854775807) ASC, t.created_at ASC, t.seq ASC, t.id ASC"
        }
        TaskListSort::ScheduledAtDesc => {
            "COALESCE(t.scheduled_at, -9223372036854775808) DESC, t.created_at DESC, t.seq DESC, t.id DESC"
        }
        TaskListSort::CreatedAt => "t.created_at ASC, t.seq ASC, t.id ASC",
        TaskListSort::CreatedAtDesc => "t.created_at DESC, t.seq DESC, t.id DESC",
        TaskListSort::UpdatedAt => "t.updated_at ASC, t.seq ASC, t.id ASC",
        TaskListSort::UpdatedAtDesc => "t.updated_at DESC, t.seq DESC, t.id DESC",
        TaskListSort::DueAt => {
            "COALESCE(t.due_at, 9223372036854775807) ASC, t.created_at ASC, t.seq ASC, t.id ASC"
        }
        TaskListSort::DueAtDesc => {
            "COALESCE(t.due_at, -9223372036854775808) DESC, t.created_at DESC, t.seq DESC, t.id DESC"
        }
    }
}

fn validate_plan_not_required_input(
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

fn validate_promote_task_input(task_id: &str, input: &PromoteTaskInput) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    if input.expected_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_lock_version must be non-negative".to_owned(),
        ));
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

fn validate_claim_task_input(task_id: &str, input: &ClaimTaskInput) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    if input.expected_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_lock_version must be non-negative".to_owned(),
        ));
    }
    if input.owner.trim().is_empty() {
        return Err(StoreError::InvalidInput("owner is required".to_owned()));
    }
    if input.claim_token.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "claim_token is required".to_owned(),
        ));
    }
    if !input.run_id.trim().starts_with("r_") || input.run_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "run_id must start with r_".to_owned(),
        ));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id must start with e_".to_owned(),
        ));
    }
    if input.worker_profile.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "worker_profile is required".to_owned(),
        ));
    }
    if input
        .log_path
        .as_deref()
        .is_some_and(|log_path| log_path.trim().is_empty())
    {
        return Err(StoreError::InvalidInput(
            "log_path must not be empty".to_owned(),
        ));
    }
    if input.now < 0 {
        return Err(StoreError::InvalidInput(
            "now must be non-negative".to_owned(),
        ));
    }
    if input.claim_expires_at <= input.now {
        return Err(StoreError::InvalidInput(
            "claim_expires_at must be after now".to_owned(),
        ));
    }
    Ok(())
}

fn validate_heartbeat_task_input(
    task_id: &str,
    input: &HeartbeatTaskInput,
) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    if input.expected_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_lock_version must be non-negative".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    if input.claim_token.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "claim_token is required".to_owned(),
        ));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id must start with e_".to_owned(),
        ));
    }
    if input.now < 0 {
        return Err(StoreError::InvalidInput(
            "now must be non-negative".to_owned(),
        ));
    }
    if input.claim_expires_at <= input.now {
        return Err(StoreError::InvalidInput(
            "claim_expires_at must be after now".to_owned(),
        ));
    }
    Ok(())
}

fn validate_release_task_input(task_id: &str, input: &ReleaseTaskInput) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    if input.expected_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_lock_version must be non-negative".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    if input.claim_token.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "claim_token is required".to_owned(),
        ));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id must start with e_".to_owned(),
        ));
    }
    if input.now < 0 {
        return Err(StoreError::InvalidInput(
            "now must be non-negative".to_owned(),
        ));
    }
    Ok(())
}

fn validate_reclaim_expired_task_input(
    task_id: &str,
    input: &ReclaimExpiredTaskInput,
) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    if input.expected_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_lock_version must be non-negative".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id must start with e_".to_owned(),
        ));
    }
    if !matches!(
        input.target_status.trim(),
        "triage" | "todo" | "scheduled" | "ready" | "blocked"
    ) {
        return Err(StoreError::InvalidInput(
            "target_status must be triage, todo, scheduled, ready, or blocked".to_owned(),
        ));
    }
    if input.retry_count < 0 {
        return Err(StoreError::InvalidInput(
            "retry_count must be non-negative".to_owned(),
        ));
    }
    if input.reason.trim().is_empty() {
        return Err(StoreError::InvalidInput("reason is required".to_owned()));
    }
    if input.now < 0 {
        return Err(StoreError::InvalidInput(
            "now must be non-negative".to_owned(),
        ));
    }
    Ok(())
}

fn validate_submit_review_task_input(
    task_id: &str,
    input: &SubmitReviewTaskInput,
) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    if input.expected_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_lock_version must be non-negative".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id must start with e_".to_owned(),
        ));
    }
    if input.now < 0 {
        return Err(StoreError::InvalidInput(
            "now must be non-negative".to_owned(),
        ));
    }
    Ok(())
}

fn validate_complete_task_input(
    task_id: &str,
    input: &CompleteTaskInput,
) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    if input.expected_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_lock_version must be non-negative".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id must start with e_".to_owned(),
        ));
    }
    if input.now < 0 {
        return Err(StoreError::InvalidInput(
            "now must be non-negative".to_owned(),
        ));
    }
    Ok(())
}

fn validate_block_task_input(task_id: &str, input: &BlockTaskInput) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    if input.expected_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_lock_version must be non-negative".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    if input.reason.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "block reason is required".to_owned(),
        ));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id must start with e_".to_owned(),
        ));
    }
    if input.now < 0 {
        return Err(StoreError::InvalidInput(
            "now must be non-negative".to_owned(),
        ));
    }
    Ok(())
}

fn task_from_row(row: Row) -> Result<TaskRecord, StoreError> {
    let board_slug = text_value(row.get_value(30)?, "boards.slug")?;
    let seq = integer_value(row.get_value(2)?, "tasks.seq")?;
    Ok(TaskRecord {
        id: text_value(row.get_value(0)?, "tasks.id")?,
        board_id: text_value(row.get_value(1)?, "tasks.board_id")?,
        board_slug: board_slug.clone(),
        task_ref: format!("{board_slug}#{seq}"),
        seq,
        idempotency_key: optional_text_value(row.get_value(3)?, "tasks.idempotency_key")?,
        title: text_value(row.get_value(4)?, "tasks.title")?,
        description: optional_text_value(row.get_value(5)?, "tasks.description")?,
        status: text_value(row.get_value(6)?, "tasks.status")?,
        status_reason: optional_text_value(row.get_value(7)?, "tasks.status_reason")?,
        assignee: optional_text_value(row.get_value(8)?, "tasks.assignee")?,
        priority: integer_value(row.get_value(9)?, "tasks.priority")?,
        position: integer_value(row.get_value(10)?, "tasks.position")?,
        scheduled_at: optional_integer_value(row.get_value(11)?, "tasks.scheduled_at")?,
        due_at: optional_integer_value(row.get_value(12)?, "tasks.due_at")?,
        created_by: text_value(row.get_value(13)?, "tasks.created_by")?,
        created_at: integer_value(row.get_value(14)?, "tasks.created_at")?,
        updated_at: integer_value(row.get_value(15)?, "tasks.updated_at")?,
        started_at: optional_integer_value(row.get_value(16)?, "tasks.started_at")?,
        completed_at: optional_integer_value(row.get_value(17)?, "tasks.completed_at")?,
        archived_at: optional_integer_value(row.get_value(18)?, "tasks.archived_at")?,
        claim_token: optional_text_value(row.get_value(19)?, "tasks.claim_token")?,
        claim_owner: optional_text_value(row.get_value(20)?, "tasks.claim_owner")?,
        claim_expires_at: optional_integer_value(row.get_value(21)?, "tasks.claim_expires_at")?,
        last_heartbeat_at: optional_integer_value(row.get_value(22)?, "tasks.last_heartbeat_at")?,
        current_run_id: optional_text_value(row.get_value(23)?, "tasks.current_run_id")?,
        retry_count: integer_value(row.get_value(24)?, "tasks.retry_count")?,
        max_retries: optional_integer_value(row.get_value(25)?, "tasks.max_retries")?,
        result_summary: optional_text_value(row.get_value(26)?, "tasks.result_summary")?,
        result_json: optional_text_value(row.get_value(27)?, "tasks.result_json")?,
        metadata_json: text_value(row.get_value(28)?, "tasks.metadata_json")?,
        lock_version: integer_value(row.get_value(29)?, "tasks.lock_version")?,
        dependency_blocked: integer_value(row.get_value(31)?, "tasks.dependency_blocked")? != 0,
        unfinished_parent_count: integer_value(
            row.get_value(32)?,
            "tasks.unfinished_parent_count",
        )?,
        execution_plan_state: text_value(row.get_value(33)?, "tasks.execution_plan_state")?,
        required_step_count: integer_value(row.get_value(34)?, "tasks.required_step_count")?,
        completed_required_step_count: integer_value(
            row.get_value(35)?,
            "tasks.completed_required_step_count",
        )?,
        optional_step_count: integer_value(row.get_value(36)?, "tasks.optional_step_count")?,
        labels: Vec::new(),
    })
}

fn comment_from_row(row: Row) -> Result<CommentRecord, StoreError> {
    Ok(CommentRecord {
        id: text_value(row.get_value(0)?, "task_comments.id")?,
        board_id: text_value(row.get_value(1)?, "task_comments.board_id")?,
        task_id: text_value(row.get_value(2)?, "task_comments.task_id")?,
        idempotency_key: optional_text_value(row.get_value(3)?, "task_comments.idempotency_key")?,
        author: text_value(row.get_value(4)?, "task_comments.author")?,
        author_type: text_value(row.get_value(5)?, "task_comments.author_type")?,
        agent_type: optional_text_value(row.get_value(6)?, "task_comments.agent_type")?,
        body: text_value(row.get_value(7)?, "task_comments.body")?,
        kind: text_value(row.get_value(8)?, "task_comments.kind")?,
        metadata_json: text_value(row.get_value(9)?, "task_comments.metadata_json")?,
        created_at: integer_value(row.get_value(10)?, "task_comments.created_at")?,
    })
}

async fn step_from_row(connection: &Connection, row: Row) -> Result<TaskStepRecord, StoreError> {
    let board_id = text_value(row.get_value(1)?, "task_steps.board_id")?;
    let linked_task_id = optional_text_value(row.get_value(6)?, "task_steps.linked_task_id")?;
    let linked_task = if let Some(linked_task_id) = linked_task_id {
        let linked_row = first_row(
            connection
                .query(
                    &format!(
                        "{TASK_SELECT} WHERE t.board_id = :board_id AND t.id = :task_id LIMIT 1"
                    ),
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", linked_task_id.as_str()),
                    ],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(linked_task_id.clone()),
            other => StoreError::Turso(other),
        })?;
        Some(task_from_row(linked_row)?)
    } else {
        None
    };
    Ok(TaskStepRecord {
        id: text_value(row.get_value(0)?, "task_steps.id")?,
        board_id,
        parent_task_id: text_value(row.get_value(2)?, "task_steps.parent_task_id")?,
        position: integer_value(row.get_value(3)?, "task_steps.position")?,
        title: text_value(row.get_value(4)?, "task_steps.title")?,
        body: optional_text_value(row.get_value(5)?, "task_steps.body")?,
        linked_task,
        required: integer_value(row.get_value(7)?, "task_steps.required")? != 0,
        status: text_value(row.get_value(8)?, "task_steps.status")?,
        resolution_note: optional_text_value(row.get_value(9)?, "task_steps.resolution_note")?,
        resolved_by: optional_text_value(row.get_value(10)?, "task_steps.resolved_by")?,
        resolved_at: optional_integer_value(row.get_value(11)?, "task_steps.resolved_at")?,
        created_by: text_value(row.get_value(12)?, "task_steps.created_by")?,
        created_at: integer_value(row.get_value(13)?, "task_steps.created_at")?,
        updated_by: text_value(row.get_value(14)?, "task_steps.updated_by")?,
        updated_at: integer_value(row.get_value(15)?, "task_steps.updated_at")?,
    })
}

fn run_from_row(row: Row) -> Result<TaskRunRecord, StoreError> {
    Ok(TaskRunRecord {
        id: text_value(row.get_value(0)?, "task_runs.id")?,
        board_id: text_value(row.get_value(1)?, "task_runs.board_id")?,
        task_id: text_value(row.get_value(2)?, "task_runs.task_id")?,
        status: text_value(row.get_value(3)?, "task_runs.status")?,
        worker_profile: optional_text_value(row.get_value(4)?, "task_runs.worker_profile")?,
        worker_pid: optional_integer_value(row.get_value(5)?, "task_runs.worker_pid")?,
        claim_token: text_value(row.get_value(6)?, "task_runs.claim_token")?,
        claim_owner: text_value(row.get_value(7)?, "task_runs.claim_owner")?,
        claim_expires_at: integer_value(row.get_value(8)?, "task_runs.claim_expires_at")?,
        started_at: integer_value(row.get_value(9)?, "task_runs.started_at")?,
        last_heartbeat_at: optional_integer_value(
            row.get_value(10)?,
            "task_runs.last_heartbeat_at",
        )?,
        finished_at: optional_integer_value(row.get_value(11)?, "task_runs.finished_at")?,
        exit_code: optional_integer_value(row.get_value(12)?, "task_runs.exit_code")?,
        summary: optional_text_value(row.get_value(13)?, "task_runs.summary")?,
        error: optional_text_value(row.get_value(14)?, "task_runs.error")?,
        log_path: optional_text_value(row.get_value(15)?, "task_runs.log_path")?,
        metadata_json: text_value(row.get_value(16)?, "task_runs.metadata_json")?,
    })
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

fn text_value(value: Value, field: &'static str) -> Result<String, StoreError> {
    match value {
        Value::Text(value) => Ok(value),
        _ => Err(StoreError::InvalidStoredValue { field }),
    }
}

fn optional_text_value(value: Value, field: &'static str) -> Result<Option<String>, StoreError> {
    match value {
        Value::Text(value) => Ok(Some(value)),
        Value::Null => Ok(None),
        _ => Err(StoreError::InvalidStoredValue { field }),
    }
}

fn integer_value(value: Value, field: &'static str) -> Result<i64, StoreError> {
    match value {
        Value::Integer(value) => Ok(value),
        _ => Err(StoreError::InvalidStoredValue { field }),
    }
}

fn optional_integer_value(value: Value, field: &'static str) -> Result<Option<i64>, StoreError> {
    match value {
        Value::Integer(value) => Ok(Some(value)),
        Value::Null => Ok(None),
        _ => Err(StoreError::InvalidStoredValue { field }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    async fn store(name: &str) -> (tempfile::TempDir, TursoStore, PathBuf) {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join(format!("{name}.db"));
        let store = TursoStore::open(&path).await.expect("open Turso database");
        (directory, store, path)
    }

    fn create_input(id: &str, idempotency_key: Option<&str>, title: &str) -> CreateTaskInput {
        CreateTaskInput {
            id: id.to_owned(),
            idempotency_key: idempotency_key.map(str::to_owned),
            title: title.to_owned(),
            status: "todo".to_owned(),
            description: Some("description".to_owned()),
            assignee: Some("agent".to_owned()),
            priority: 1,
            scheduled_at: Some(100),
            due_at: Some(200),
            max_retries: Some(2),
            metadata_json: r#"{"source":"test"}"#.to_owned(),
            created_by: "tester".to_owned(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn comment_input(
        id: &str,
        idempotency_key: Option<&str>,
        author: &str,
        author_type: &str,
        agent_type: Option<&str>,
        body: &str,
        kind: &str,
        metadata_json: &str,
        event_id: &str,
        created_at: i64,
    ) -> CreateCommentInput {
        CreateCommentInput {
            id: id.to_owned(),
            idempotency_key: idempotency_key.map(str::to_owned),
            author: author.to_owned(),
            author_type: author_type.to_owned(),
            agent_type: agent_type.map(str::to_owned),
            body: body.to_owned(),
            kind: kind.to_owned(),
            metadata_json: metadata_json.to_owned(),
            event_id: event_id.to_owned(),
            created_at,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn step_input(
        id: &str,
        key: Option<&str>,
        title: &str,
        position: Option<i64>,
        actor: &str,
        expected_lock_version: i64,
        expected_plan_state: &str,
        target_status: &str,
        event_id: &str,
        plan_event_id: &str,
        recompute_event_id: &str,
        created_at: i64,
    ) -> CreateStepInput {
        CreateStepInput {
            id: id.to_owned(),
            idempotency_key: key.map(str::to_owned),
            title: title.to_owned(),
            body: Some("body".to_owned()),
            linked_task_id: None,
            position,
            required: true,
            created_by: actor.to_owned(),
            event_id: event_id.to_owned(),
            plan_event_id: plan_event_id.to_owned(),
            recompute_event_id: recompute_event_id.to_owned(),
            created_at,
            expected_lock_version,
            expected_plan_state: expected_plan_state.to_owned(),
            target_status: target_status.to_owned(),
        }
    }

    fn plan_input(
        reason: &str,
        actor: &str,
        event_id: &str,
        updated_at: i64,
    ) -> MarkExecutionPlanNotRequiredInput {
        MarkExecutionPlanNotRequiredInput {
            reason: reason.to_owned(),
            actor: actor.to_owned(),
            event_id: event_id.to_owned(),
            updated_at,
        }
    }

    fn promote_input(
        expected_lock_version: i64,
        actor: &str,
        event_id: &str,
        updated_at: i64,
    ) -> PromoteTaskInput {
        PromoteTaskInput {
            expected_lock_version,
            actor: actor.to_owned(),
            event_id: event_id.to_owned(),
            updated_at,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn claim_input(
        expected_lock_version: i64,
        owner: &str,
        claim_token: &str,
        run_id: &str,
        event_id: &str,
        metadata_json: &str,
        now: i64,
        ttl_ms: i64,
    ) -> ClaimTaskInput {
        ClaimTaskInput {
            expected_lock_version,
            owner: owner.to_owned(),
            claim_token: claim_token.to_owned(),
            run_id: run_id.to_owned(),
            event_id: event_id.to_owned(),
            worker_profile: "manual".to_owned(),
            metadata_json: metadata_json.to_owned(),
            log_path: None,
            now,
            claim_expires_at: now.saturating_add(ttl_ms),
        }
    }

    fn heartbeat_input(
        expected_lock_version: i64,
        actor: &str,
        claim_token: &str,
        event_id: &str,
        note: Option<&str>,
        now: i64,
        claim_expires_at: i64,
    ) -> HeartbeatTaskInput {
        HeartbeatTaskInput {
            expected_lock_version,
            actor: actor.to_owned(),
            claim_token: claim_token.to_owned(),
            event_id: event_id.to_owned(),
            note: note.map(str::to_owned),
            now,
            claim_expires_at,
        }
    }

    fn release_input(
        expected_lock_version: i64,
        actor: &str,
        claim_token: &str,
        event_id: &str,
        now: i64,
    ) -> ReleaseTaskInput {
        ReleaseTaskInput {
            expected_lock_version,
            actor: actor.to_owned(),
            claim_token: claim_token.to_owned(),
            event_id: event_id.to_owned(),
            now,
        }
    }

    fn reclaim_input(
        expected_lock_version: i64,
        actor: &str,
        event_id: &str,
        target_status: &str,
        retry_count: i64,
        reason: &str,
        now: i64,
    ) -> ReclaimExpiredTaskInput {
        ReclaimExpiredTaskInput {
            expected_lock_version,
            actor: actor.to_owned(),
            event_id: event_id.to_owned(),
            target_status: target_status.to_owned(),
            retry_count,
            reason: reason.to_owned(),
            now,
        }
    }

    fn submit_review_input(
        expected_lock_version: i64,
        actor: &str,
        claim_token: Option<&str>,
        force: bool,
        summary: Option<&str>,
        now: i64,
        event_id: &str,
    ) -> SubmitReviewTaskInput {
        SubmitReviewTaskInput {
            expected_lock_version,
            actor: actor.to_owned(),
            claim_token: claim_token.map(str::to_owned),
            force,
            summary: summary.map(str::to_owned),
            now,
            event_id: event_id.to_owned(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_input(
        expected_lock_version: i64,
        actor: &str,
        claim_token: Option<&str>,
        force: bool,
        summary: Option<&str>,
        result_json: Option<&str>,
        now: i64,
        event_id: &str,
    ) -> CompleteTaskInput {
        CompleteTaskInput {
            expected_lock_version,
            actor: actor.to_owned(),
            claim_token: claim_token.map(str::to_owned),
            force,
            summary: summary.map(str::to_owned),
            result_json: result_json.map(str::to_owned),
            now,
            event_id: event_id.to_owned(),
        }
    }

    fn block_input(
        expected_lock_version: i64,
        actor: &str,
        claim_token: Option<&str>,
        force: bool,
        reason: &str,
        now: i64,
        event_id: &str,
    ) -> BlockTaskInput {
        BlockTaskInput {
            expected_lock_version,
            actor: actor.to_owned(),
            claim_token: claim_token.map(str::to_owned),
            force,
            reason: reason.to_owned(),
            now,
            event_id: event_id.to_owned(),
        }
    }

    async fn ready_task_for_claim(
        store: &TursoStore,
        task_id: &str,
        idempotency_key: &str,
        title: &str,
    ) -> TaskRecord {
        store
            .create_task(
                "default",
                create_input(task_id, Some(idempotency_key), title),
            )
            .await
            .expect("create claim task");
        store
            .mark_execution_plan_not_required(
                task_id,
                plan_input(
                    "No claim plan",
                    "planner",
                    &format!("e_{task_id}_plan"),
                    100,
                ),
            )
            .await
            .expect("mark claim plan not required");
        store
            .promote_task(
                task_id,
                promote_input(0, "promoter", &format!("e_{task_id}_promote"), 200),
            )
            .await
            .expect("promote claim task")
    }

    async fn count_rows(connection: &Connection, table: &str) -> i64 {
        let mut rows = connection
            .query(&format!("SELECT COUNT(*) FROM {table}"), ())
            .await
            .expect("count rows");
        let row = rows.next().await.expect("count row").expect("count result");
        integer_value(row.get_value(0).expect("count value"), "count").expect("integer count")
    }

    #[tokio::test]
    async fn fresh_database_bootstraps_canonical_tables() {
        let (_directory, store, _path) = store("bootstrap").await;
        store.initialize().await.expect("initialize");

        let connection = store.connection().await.expect("connection");
        let mut rows = connection
            .query(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '__turso_internal_%' ORDER BY name",
                (),
            )
            .await
            .expect("table query");
        let mut names = Vec::new();
        while let Some(row) = rows.next().await.expect("next table row") {
            names.push(
                text_value(row.get_value(0).expect("table name"), "sqlite_master.name")
                    .expect("text table name"),
            );
        }
        assert_eq!(
            names,
            vec![
                "board_columns",
                "boards",
                "schema_migrations",
                "task_comments",
                "task_dependencies",
                "task_events",
                "task_execution_plans",
                "task_runs",
                "task_steps",
                "tasks",
            ]
        );
    }

    #[tokio::test]
    async fn initialize_is_idempotent_and_seeds_default_board_columns() {
        let (_directory, store, path) = store("idempotent").await;
        store.initialize().await.expect("first initialize");
        store.initialize().await.expect("second initialize");

        let boards = store.list_boards(false).await.expect("list boards");
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].slug, "default");

        let columns = store
            .list_board_columns("default")
            .await
            .expect("list columns");
        assert_eq!(columns.len(), 9);
        assert_eq!(
            columns
                .iter()
                .map(|column| (column.status.as_str(), column.position, column.hidden))
                .collect::<Vec<_>>(),
            vec![
                ("triage", 10, false),
                ("todo", 20, false),
                ("scheduled", 30, false),
                ("ready", 40, false),
                ("running", 50, false),
                ("blocked", 60, false),
                ("review", 70, false),
                ("done", 80, false),
                ("archived", 90, true),
            ]
        );

        drop(store);
        let reopened = TursoStore::open(path).await.expect("reopen database");
        reopened.initialize().await.expect("reinitialize database");
        assert_eq!(
            reopened
                .list_boards(false)
                .await
                .expect("list after reopen")
                .len(),
            1
        );
        assert_eq!(
            reopened
                .list_board_columns("b_default")
                .await
                .expect("columns by id")
                .len(),
            9
        );
    }

    #[tokio::test]
    async fn include_archived_filters_and_orders_boards() {
        let (_directory, store, _path) = store("archived").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at, archived_at) VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
                ("b_archived", "archive", "Archive", 2_i64, 3_i64),
            )
            .await
            .expect("insert archived board");

        let active = store.list_boards(false).await.expect("active boards");
        assert_eq!(
            active
                .iter()
                .map(|board| board.slug.as_str())
                .collect::<Vec<_>>(),
            vec!["default"]
        );
        let all = store.list_boards(true).await.expect("all boards");
        assert_eq!(
            all.iter()
                .map(|board| board.slug.as_str())
                .collect::<Vec<_>>(),
            vec!["default", "archive"]
        );
    }

    #[tokio::test]
    async fn create_task_writes_task_plan_and_event_atomically() {
        let (_directory, store, _path) = store("create").await;
        store.initialize().await.expect("initialize");

        let task = store
            .create_task(
                "default",
                create_input("t_create", Some("create-1"), "Create task"),
            )
            .await
            .expect("create task");
        assert_eq!(task.id, "t_create");
        assert_eq!(task.board_id, "b_default");
        assert_eq!(task.board_slug, "default");
        assert_eq!(task.task_ref, "default#1");
        assert_eq!(task.seq, 1);
        assert_eq!(task.idempotency_key.as_deref(), Some("create-1"));
        assert_eq!(task.title, "Create task");
        assert_eq!(task.status, "todo");
        assert_eq!(task.priority, 1);
        assert_eq!(task.position, 1024);
        assert_eq!(task.lock_version, 0);
        assert_eq!(task.max_retries, Some(2));

        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "tasks").await, 1);
        assert_eq!(count_rows(&connection, "task_execution_plans").await, 1);
        assert_eq!(count_rows(&connection, "task_events").await, 1);
        let plan = first_row(
            connection
                .query(
                    "SELECT state FROM task_execution_plans WHERE task_id = ?1",
                    [task.id.as_str()],
                )
                .await
                .expect("plan query"),
        )
        .await
        .expect("plan row");
        assert_eq!(
            text_value(plan.get_value(0).expect("plan state"), "plan.state")
                .expect("plan state text"),
            "unplanned"
        );
        let event = first_row(
            connection
                .query(
                    "SELECT kind, actor, payload_json FROM task_events WHERE task_id = ?1",
                    [task.id.as_str()],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.created"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "tester"
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"status":"todo"}"#
        );
    }

    #[tokio::test]
    async fn create_task_preserves_allowed_initial_statuses() {
        let (_directory, store, _path) = store("create-statuses").await;
        store.initialize().await.expect("initialize");
        let mut triage = create_input("t_triage", Some("status-triage"), "Triage");
        triage.status = "triage".to_owned();
        let mut scheduled = create_input("t_scheduled", Some("status-scheduled"), "Scheduled");
        scheduled.status = "scheduled".to_owned();

        let triage_task = store
            .create_task("default", triage)
            .await
            .expect("triage create");
        let scheduled_task = store
            .create_task("default", scheduled)
            .await
            .expect("scheduled create");
        assert_eq!(triage_task.status, "triage");
        assert_eq!(scheduled_task.status, "scheduled");

        let mut ready = create_input("t_ready", Some("status-ready"), "Ready");
        ready.status = "ready".to_owned();
        assert!(matches!(
            store.create_task("default", ready).await,
            Err(StoreError::InvalidInput(message)) if message.contains("status")
        ));

        let connection = store.connection().await.expect("connection");
        let mut rows = connection
            .query("SELECT payload_json FROM task_events ORDER BY id ASC", ())
            .await
            .expect("event payload query");
        let mut payloads = Vec::new();
        while let Some(row) = rows.next().await.expect("event payload row") {
            payloads.push(
                text_value(row.get_value(0).expect("event payload"), "event.payload")
                    .expect("event payload text"),
            );
        }
        assert_eq!(
            payloads,
            vec![r#"{"status":"triage"}"#, r#"{"status":"scheduled"}"#]
        );
    }

    #[tokio::test]
    async fn create_task_replays_same_idempotent_payload_without_extra_rows() {
        let (_directory, store, _path) = store("create-replay").await;
        store.initialize().await.expect("initialize");
        let input = create_input("t_replay", Some("replay-1"), "Replay task");
        let first = store
            .create_task("default", input.clone())
            .await
            .expect("first create");
        let mut retry_input = input;
        retry_input.id = "t_replay_retry".to_owned();
        let replay = store
            .create_task("b_default", retry_input)
            .await
            .expect("replay create");
        assert_eq!(first, replay);

        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "tasks").await, 1);
        assert_eq!(count_rows(&connection, "task_execution_plans").await, 1);
        assert_eq!(count_rows(&connection, "task_events").await, 1);
    }

    #[tokio::test]
    async fn create_task_rejects_same_key_with_different_payload() {
        let (_directory, store, _path) = store("create-conflict").await;
        store.initialize().await.expect("initialize");
        let input = create_input("t_conflict", Some("conflict-1"), "Original");
        store
            .create_task("default", input.clone())
            .await
            .expect("first create");
        let mut changed = input;
        changed.title = "Changed".to_owned();
        let error = store
            .create_task("default", changed)
            .await
            .expect_err("different payload must conflict");
        assert!(matches!(
            error,
            StoreError::IdempotencyConflict {
                board_id,
                key,
                existing_task_id
            } if board_id == "b_default" && key == "conflict-1" && existing_task_id == "t_conflict"
        ));
        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "tasks").await, 1);
        assert_eq!(count_rows(&connection, "task_execution_plans").await, 1);
        assert_eq!(count_rows(&connection, "task_events").await, 1);
    }

    #[tokio::test]
    async fn create_task_assigns_monotonic_board_local_sequences() {
        let (_directory, store, _path) = store("create-seq").await;
        store.initialize().await.expect("initialize");
        let first = store
            .create_task("default", create_input("t_seq_1", Some("seq-1"), "First"))
            .await
            .expect("first create");
        let second = store
            .create_task(
                "b_default",
                create_input("t_seq_2", Some("seq-2"), "Second"),
            )
            .await
            .expect("second create");
        assert_eq!(first.seq, 1);
        assert_eq!(second.seq, 2);
        assert_ne!(first.id, second.id);
    }

    #[tokio::test]
    async fn create_task_reports_missing_board() {
        let (_directory, store, _path) = store("create-missing-board").await;
        store.initialize().await.expect("initialize");
        let error = store
            .create_task(
                "missing",
                create_input("t_missing_board", Some("missing-board"), "Missing"),
            )
            .await
            .expect_err("missing board must fail");
        assert!(matches!(error, StoreError::BoardNotFound(selector) if selector == "missing"));
        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "tasks").await, 0);
        assert_eq!(count_rows(&connection, "task_execution_plans").await, 0);
        assert_eq!(count_rows(&connection, "task_events").await, 0);
    }

    #[tokio::test]
    async fn create_task_failure_does_not_leave_partial_rows() {
        let (_directory, store, _path) = store("create-rollback").await;
        store.initialize().await.expect("initialize");
        let mut invalid = create_input("t_invalid_json", Some("invalid-json"), "Invalid");
        invalid.metadata_json = "{not-json".to_owned();
        assert!(store.create_task("default", invalid).await.is_err());

        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "tasks").await, 0);
        assert_eq!(count_rows(&connection, "task_execution_plans").await, 0);
        assert_eq!(count_rows(&connection, "task_events").await, 0);
    }

    #[tokio::test]
    async fn list_tasks_excludes_archived_by_default_and_supports_status_priority_and_assignee_filters()
     {
        let (_directory, store, _path) = store("list-filters").await;
        store.initialize().await.expect("initialize");
        let first = store
            .create_task(
                "default",
                create_input("t_filter_1", Some("filter-1"), "First"),
            )
            .await
            .expect("first task");
        let mut second_input = create_input("t_filter_2", Some("filter-2"), "Second");
        second_input.status = "scheduled".to_owned();
        second_input.priority = 2;
        second_input.assignee = Some("other".to_owned());
        let second = store
            .create_task("default", second_input)
            .await
            .expect("second task");
        let archived = store
            .create_task(
                "default",
                create_input("t_filter_archived", Some("filter-archived"), "Archived"),
            )
            .await
            .expect("archived task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 10 WHERE id = ?1",
                [archived.id.as_str()],
            )
            .await
            .expect("archive task");

        let default_page = store
            .list_tasks("default", TaskListOptions::default())
            .await
            .expect("default task list");
        assert_eq!(default_page.total, 2);
        assert_eq!(
            default_page
                .tasks
                .iter()
                .map(|task| task.id.as_str())
                .collect::<Vec<_>>(),
            vec![first.id.as_str(), second.id.as_str()]
        );

        let filtered = store
            .list_tasks(
                "b_default",
                TaskListOptions {
                    statuses: vec!["todo".to_owned()],
                    priorities: vec![1],
                    assignee: Some("agent".to_owned()),
                    ..TaskListOptions::default()
                },
            )
            .await
            .expect("filtered task list");
        assert_eq!(filtered.total, 1);
        assert_eq!(filtered.tasks[0].id, first.id);

        let with_archived = store
            .list_tasks(
                "default",
                TaskListOptions {
                    include_archived: true,
                    ..TaskListOptions::default()
                },
            )
            .await
            .expect("archived task list");
        assert_eq!(with_archived.total, 3);
        assert_eq!(with_archived.tasks[2].id, archived.id);
    }

    #[tokio::test]
    async fn list_tasks_supports_id_seq_board_seq_and_escaped_text_search() {
        let (_directory, store, _path) = store("list-search").await;
        store.initialize().await.expect("initialize");
        let first = store
            .create_task(
                "default",
                create_input("t_search", Some("search-1"), "Literal %_\\ Marker"),
            )
            .await
            .expect("search task");
        let mut second_input = create_input("t_other", Some("search-2"), "Different");
        second_input.description = Some("Needle in description".to_owned());
        let second = store
            .create_task("default", second_input)
            .await
            .expect("second search task");

        for (query, expected) in [
            ("t_search", vec![first.id.as_str()]),
            ("default#1", vec![first.id.as_str()]),
            ("#1", vec![first.id.as_str()]),
            ("1", vec![first.id.as_str()]),
            ("%_\\", vec![first.id.as_str()]),
            ("needle", vec![second.id.as_str()]),
        ] {
            let page = store
                .list_tasks(
                    "default",
                    TaskListOptions {
                        q: Some(query.to_owned()),
                        ..TaskListOptions::default()
                    },
                )
                .await
                .expect("search task list");
            assert_eq!(
                page.tasks
                    .iter()
                    .map(|task| task.id.as_str())
                    .collect::<Vec<_>>(),
                expected,
                "query {query}"
            );
        }

        let mismatch = store
            .list_tasks(
                "default",
                TaskListOptions {
                    q: Some("other#1".to_owned()),
                    ..TaskListOptions::default()
                },
            )
            .await
            .expect("mismatched board search");
        assert!(mismatch.tasks.is_empty());
        assert_eq!(mismatch.total, 0);
    }

    #[tokio::test]
    async fn list_tasks_plan_filters_and_derived_fields_are_consistent() {
        let (_directory, store, _path) = store("list-plan").await;
        store.initialize().await.expect("initialize");
        let plain = store
            .create_task(
                "default",
                create_input("t_plan_plain", Some("plan-plain"), "Plain"),
            )
            .await
            .expect("plain task");
        let with_steps = store
            .create_task(
                "default",
                create_input("t_plan_steps", Some("plan-steps"), "With steps"),
            )
            .await
            .expect("steps task");
        let not_required = store
            .create_task(
                "default",
                create_input(
                    "t_plan_not_required",
                    Some("plan-not-required"),
                    "Not required",
                ),
            )
            .await
            .expect("not required task");
        let done = store
            .create_task(
                "default",
                create_input("t_plan_done", Some("plan-done"), "Done"),
            )
            .await
            .expect("done task");
        let parent = store
            .create_task(
                "default",
                create_input("t_plan_parent", Some("plan-parent"), "Parent"),
            )
            .await
            .expect("parent task");
        let child = store
            .create_task(
                "default",
                create_input("t_plan_child", Some("plan-child"), "Child"),
            )
            .await
            .expect("child task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_steps(id, board_id, parent_task_id, position, title, required, status, created_by, created_at, updated_by, updated_at) VALUES (?1, 'b_default', ?2, 1, 'required', 1, 'todo', 'tester', 1, 'tester', 1)",
                ("step_incomplete", with_steps.id.as_str()),
            )
            .await
            .expect("insert incomplete step");
        connection
            .execute(
                "INSERT INTO task_steps(id, board_id, parent_task_id, position, title, required, status, created_by, created_at, updated_by, updated_at) VALUES (?1, 'b_default', ?2, 2, 'optional', 0, 'done', 'tester', 1, 'tester', 1)",
                ("step_optional", with_steps.id.as_str()),
            )
            .await
            .expect("insert optional step");
        connection
            .execute(
                "UPDATE task_execution_plans SET state = 'not_required' WHERE task_id = ?1",
                [not_required.id.as_str()],
            )
            .await
            .expect("set plan not required");
        connection
            .execute(
                "UPDATE tasks SET status = 'done' WHERE id = ?1",
                [done.id.as_str()],
            )
            .await
            .expect("finish task");
        connection
            .execute(
                "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES ('b_default', ?1, ?2, 1)",
                (parent.id.as_str(), child.id.as_str()),
            )
            .await
            .expect("insert dependency");
        connection
            .execute(
                "UPDATE tasks SET status = 'blocked' WHERE id = ?1",
                [parent.id.as_str()],
            )
            .await
            .expect("block parent");

        let all = store
            .list_tasks(
                "default",
                TaskListOptions {
                    include_archived: true,
                    ..TaskListOptions::default()
                },
            )
            .await
            .expect("all tasks");
        let child_record = all.tasks.iter().find(|task| task.id == child.id).unwrap();
        assert!(child_record.dependency_blocked);
        assert_eq!(child_record.unfinished_parent_count, 1);
        let steps_record = all
            .tasks
            .iter()
            .find(|task| task.id == with_steps.id)
            .unwrap();
        assert_eq!(steps_record.execution_plan_state, "planned");
        assert_eq!(steps_record.required_step_count, 1);
        assert_eq!(steps_record.completed_required_step_count, 0);
        assert_eq!(steps_record.optional_step_count, 1);
        assert!(all.tasks.iter().all(|task| task.labels.is_empty()));

        for (filter, expected) in [
            (
                TaskPlanFilter::PlanNeeded,
                vec![plain.id.as_str(), parent.id.as_str(), child.id.as_str()],
            ),
            (TaskPlanFilter::HasSteps, vec![with_steps.id.as_str()]),
            (
                TaskPlanFilter::IncompleteRequiredSteps,
                vec![with_steps.id.as_str()],
            ),
        ] {
            let page = store
                .list_tasks(
                    "default",
                    TaskListOptions {
                        include_archived: true,
                        plan_filters: vec![filter],
                        ..TaskListOptions::default()
                    },
                )
                .await
                .expect("plan filter list");
            assert_eq!(
                page.tasks
                    .iter()
                    .map(|task| task.id.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[tokio::test]
    async fn list_tasks_paginates_with_total_and_all_sort_mappings_are_deterministic() {
        let (_directory, store, _path) = store("list-pagination-sort").await;
        store.initialize().await.expect("initialize");
        for (id, title) in [
            ("t_sort_1", "Zeta"),
            ("t_sort_2", "Alpha"),
            ("t_sort_3", "Beta"),
        ] {
            store
                .create_task("default", create_input(id, Some(id), title))
                .await
                .expect("sort task");
        }
        let page = store
            .list_tasks(
                "default",
                TaskListOptions {
                    limit: 1,
                    offset: 1,
                    ..TaskListOptions::default()
                },
            )
            .await
            .expect("paged task list");
        assert_eq!(page.total, 3);
        assert_eq!(page.tasks.len(), 1);
        assert_eq!(page.tasks[0].seq, 2);
        let empty_page = store
            .list_tasks(
                "default",
                TaskListOptions {
                    limit: 0,
                    ..TaskListOptions::default()
                },
            )
            .await
            .expect("zero limit task list");
        assert_eq!(empty_page.total, 3);
        assert!(empty_page.tasks.is_empty());

        let sorts = [
            TaskListSort::Seq,
            TaskListSort::SeqDesc,
            TaskListSort::Title,
            TaskListSort::TitleDesc,
            TaskListSort::Status,
            TaskListSort::StatusDesc,
            TaskListSort::Position,
            TaskListSort::PositionDesc,
            TaskListSort::Priority,
            TaskListSort::PriorityDesc,
            TaskListSort::Assignee,
            TaskListSort::AssigneeDesc,
            TaskListSort::ScheduledAt,
            TaskListSort::ScheduledAtDesc,
            TaskListSort::CreatedAt,
            TaskListSort::CreatedAtDesc,
            TaskListSort::UpdatedAt,
            TaskListSort::UpdatedAtDesc,
            TaskListSort::DueAt,
            TaskListSort::DueAtDesc,
        ];
        for sort in sorts {
            let options = TaskListOptions {
                sort,
                ..TaskListOptions::default()
            };
            let first = store
                .list_tasks("default", options.clone())
                .await
                .expect("sort task list");
            let second = store
                .list_tasks("default", options)
                .await
                .expect("repeat sort task list");
            assert_eq!(
                first
                    .tasks
                    .iter()
                    .map(|task| task.id.as_str())
                    .collect::<Vec<_>>(),
                second
                    .tasks
                    .iter()
                    .map(|task| task.id.as_str())
                    .collect::<Vec<_>>()
            );
        }

        let error = store
            .list_tasks(
                "default",
                TaskListOptions {
                    limit: 1001,
                    ..TaskListOptions::default()
                },
            )
            .await
            .expect_err("limit above maximum must fail");
        assert!(matches!(error, StoreError::InvalidInput(message) if message.contains("limit")));
    }

    #[tokio::test]
    async fn list_tasks_reports_missing_board() {
        let (_directory, store, _path) = store("list-missing-board").await;
        store.initialize().await.expect("initialize");
        let error = store
            .list_tasks("missing", TaskListOptions::default())
            .await
            .expect_err("missing board must fail");
        assert!(matches!(error, StoreError::BoardNotFound(selector) if selector == "missing"));
    }

    #[tokio::test]
    async fn get_task_global_returns_complete_task_record() {
        let (_directory, store, _path) = store("show-global").await;
        store.initialize().await.expect("initialize");
        let created = store
            .create_task(
                "default",
                create_input("t_show", Some("show-1"), "Show task"),
            )
            .await
            .expect("create task");

        let shown = store
            .get_task_global("t_show")
            .await
            .expect("get global task");
        assert_eq!(shown, created);
        assert_eq!(shown.board_id, "b_default");
        assert_eq!(shown.board_slug, "default");
        assert_eq!(shown.task_ref, "default#1");
        assert_eq!(shown.execution_plan_state, "unplanned");
        assert_eq!(shown.unfinished_parent_count, 0);
        assert!(shown.labels.is_empty());
    }

    #[tokio::test]
    async fn get_task_global_rejects_invalid_and_unknown_ids() {
        let (_directory, store, _path) = store("show-errors").await;
        store.initialize().await.expect("initialize");

        let invalid = store
            .get_task_global("default#1")
            .await
            .expect_err("board-local ref must be rejected");
        assert!(
            matches!(invalid, StoreError::InvalidInput(message) if message.contains("task id"))
        );

        let unknown = store
            .get_task_global("t_unknown")
            .await
            .expect_err("unknown global id must be not found");
        assert!(matches!(unknown, StoreError::TaskNotFound(task_id) if task_id == "t_unknown"));
    }

    #[tokio::test]
    async fn get_task_global_resolves_the_correct_board_without_board_local_lookup() {
        let (_directory, store, _path) = store("show-multi-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_other', 'other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("insert second board");
        let default_task = store
            .create_task(
                "default",
                create_input("t_default_show", Some("show-default"), "Default task"),
            )
            .await
            .expect("create default task");
        let other_task = store
            .create_task(
                "b_other",
                create_input("t_other_show", Some("show-other"), "Other task"),
            )
            .await
            .expect("create other task");

        let shown_default = store
            .get_task_global(&default_task.id)
            .await
            .expect("get default global task");
        let shown_other = store
            .get_task_global(&other_task.id)
            .await
            .expect("get other global task");
        assert_eq!(shown_default.board_slug, "default");
        assert_eq!(shown_default.seq, 1);
        assert_eq!(shown_other.board_id, "b_other");
        assert_eq!(shown_other.board_slug, "other");
        assert_eq!(shown_other.seq, 1);
    }

    #[tokio::test]
    async fn promote_task_todo_writes_ready_and_event() {
        let (_directory, store, _path) = store("promote-todo").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input("t_promote_todo", Some("promote-todo"), "Promote todo"),
            )
            .await
            .expect("create task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input("No execution plan", "planner", "e_promote_plan", 100),
            )
            .await
            .expect("mark plan not required");

        let promoted = store
            .promote_task(&task.id, promote_input(0, "promoter", "e_promoted", 200))
            .await
            .expect("promote task");
        assert_eq!(promoted.id, task.id);
        assert_eq!(promoted.board_id, "b_default");
        assert_eq!(promoted.board_slug, "default");
        assert_eq!(promoted.task_ref, "default#1");
        assert_eq!(promoted.status, "ready");
        assert_eq!(promoted.status_reason, None);
        assert_eq!(promoted.lock_version, 1);
        assert_eq!(promoted.updated_at, 200);
        assert_eq!(promoted.execution_plan_state, "not_required");
        assert!(!promoted.dependency_blocked);
        assert_eq!(promoted.unfinished_parent_count, 0);
        assert!(promoted.labels.is_empty());

        let connection = store.connection().await.expect("connection");
        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_promoted"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_default"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.promoted"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "promoter"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"to_status":"ready"}"#
        );
        assert_eq!(
            integer_value(
                event.get_value(5).expect("event created_at"),
                "event.created_at"
            )
            .expect("event created_at integer"),
            200
        );
    }

    #[tokio::test]
    async fn promote_task_scheduled_when_due_writes_ready() {
        let (_directory, store, _path) = store("promote-scheduled-due").await;
        store.initialize().await.expect("initialize");
        let mut input = create_input(
            "t_promote_scheduled",
            Some("promote-scheduled"),
            "Promote scheduled",
        );
        input.status = "scheduled".to_owned();
        input.scheduled_at = Some(100);
        let task = store
            .create_task("default", input)
            .await
            .expect("create scheduled task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "No scheduled execution",
                    "planner",
                    "e_promote_scheduled_plan",
                    100,
                ),
            )
            .await
            .expect("mark scheduled plan not required");

        let promoted = store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_promoted_scheduled", 100),
            )
            .await
            .expect("promote due scheduled task");
        assert_eq!(promoted.status, "ready");
        assert_eq!(promoted.scheduled_at, Some(100));
        assert_eq!(promoted.lock_version, 1);
        assert_eq!(promoted.updated_at, 100);

        let connection = store.connection().await.expect("connection");
        let event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.promoted' AND payload_json = '{\"to_status\":\"ready\"}'",
                    [task.id.as_str()],
                )
                .await
                .expect("promoted event count query"),
        )
        .await
        .expect("promoted event count row");
        assert_eq!(
            integer_value(
                event_count.get_value(0).expect("event count"),
                "event.count"
            )
            .expect("event count integer"),
            1
        );
    }

    #[tokio::test]
    async fn promote_task_rejects_source_and_readiness_guards_without_partial_write() {
        let (_directory, store, _path) = store("promote-guards").await;
        store.initialize().await.expect("initialize");
        let unplanned = store
            .create_task(
                "default",
                create_input(
                    "t_promote_unplanned",
                    Some("promote-unplanned"),
                    "Unplanned",
                ),
            )
            .await
            .expect("create unplanned task");

        let mut triage_input =
            create_input("t_promote_source", Some("promote-source"), "Invalid source");
        triage_input.status = "triage".to_owned();
        let source = store
            .create_task("default", triage_input)
            .await
            .expect("create source task");

        let mut incomplete_input = create_input(
            "t_promote_incomplete",
            Some("promote-incomplete"),
            "Incomplete",
        );
        incomplete_input.description = None;
        let incomplete = store
            .create_task("default", incomplete_input)
            .await
            .expect("create incomplete task");
        store
            .mark_execution_plan_not_required(
                &incomplete.id,
                plan_input(
                    "No execution plan",
                    "planner",
                    "e_promote_incomplete_plan",
                    100,
                ),
            )
            .await
            .expect("mark incomplete plan not required");

        let mut future_input = create_input(
            "t_promote_future",
            Some("promote-future"),
            "Future scheduled",
        );
        future_input.status = "scheduled".to_owned();
        future_input.scheduled_at = Some(500);
        let future = store
            .create_task("default", future_input)
            .await
            .expect("create future task");
        store
            .mark_execution_plan_not_required(
                &future.id,
                plan_input(
                    "No future execution",
                    "planner",
                    "e_promote_future_plan",
                    100,
                ),
            )
            .await
            .expect("mark future plan not required");

        let parent = store
            .create_task(
                "default",
                create_input("t_promote_parent", Some("promote-parent"), "Parent"),
            )
            .await
            .expect("create dependency parent");
        let child = store
            .create_task(
                "default",
                create_input("t_promote_child", Some("promote-child"), "Child"),
            )
            .await
            .expect("create dependency child");
        store
            .mark_execution_plan_not_required(
                &child.id,
                plan_input("No child execution", "planner", "e_promote_child_plan", 100),
            )
            .await
            .expect("mark child plan not required");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES ('b_default', ?1, ?2, 1)",
                (parent.id.as_str(), child.id.as_str()),
            )
            .await
            .expect("insert unfinished dependency");

        let cases = [
            (unplanned.id.as_str(), "execution plan"),
            (source.id.as_str(), "cannot promote from triage"),
            (incomplete.id.as_str(), "task spec"),
            (future.id.as_str(), "future"),
            (child.id.as_str(), "dependency"),
        ];
        for (index, (task_id, message)) in cases.into_iter().enumerate() {
            let error = store
                .promote_task(
                    task_id,
                    promote_input(0, "promoter", &format!("e_promote_guard_{index}"), 100),
                )
                .await
                .expect_err("readiness guard must fail");
            assert!(matches!(
                error,
                StoreError::InvalidTransition(error_message)
                    if error_message.contains(message)
            ));
        }

        for (task_id, expected_status, expected_plan) in [
            (unplanned.id.as_str(), "todo", "unplanned"),
            (source.id.as_str(), "triage", "unplanned"),
            (incomplete.id.as_str(), "todo", "not_required"),
            (future.id.as_str(), "scheduled", "not_required"),
            (child.id.as_str(), "todo", "not_required"),
        ] {
            let unchanged = store
                .get_task_global(task_id)
                .await
                .expect("get unchanged task");
            assert_eq!(unchanged.status, expected_status, "task {task_id}");
            assert_eq!(unchanged.lock_version, 0, "task {task_id}");
            assert_eq!(
                unchanged.execution_plan_state, expected_plan,
                "task {task_id}"
            );
        }
        let promoted_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.promoted'",
                    (),
                )
                .await
                .expect("promoted event count query"),
        )
        .await
        .expect("promoted event count row");
        assert_eq!(
            integer_value(
                promoted_event_count
                    .get_value(0)
                    .expect("promoted event count"),
                "event.count",
            )
            .expect("promoted event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn promote_task_rejects_archived_task_board_and_stale_version_without_partial_write() {
        let (_directory, store, _path) = store("promote-archive-stale").await;
        store.initialize().await.expect("initialize");
        let archived_task = store
            .create_task(
                "default",
                create_input(
                    "t_promote_archived_task",
                    Some("promote-archived-task"),
                    "Archived task",
                ),
            )
            .await
            .expect("create archived task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 300 WHERE id = ?1",
                [archived_task.id.as_str()],
            )
            .await
            .expect("archive task");

        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at, archived_at) VALUES ('b_promote_archived', 'promote-archived', 'Archived promote board', 1, 1, 350)",
                (),
            )
            .await
            .expect("insert archived board");
        let archived_board_task = store
            .create_task(
                "promote-archived",
                create_input(
                    "t_promote_archived_board",
                    Some("promote-archived-board"),
                    "Archived board task",
                ),
            )
            .await
            .expect("create task on archived board");

        let stale = store
            .create_task(
                "default",
                create_input("t_promote_stale", Some("promote-stale"), "Stale task"),
            )
            .await
            .expect("create stale task");
        store
            .mark_execution_plan_not_required(
                &stale.id,
                plan_input("No stale execution", "planner", "e_promote_stale_plan", 100),
            )
            .await
            .expect("mark stale plan not required");

        for (task_id, expected_lock_version, message) in [
            (archived_task.id.as_str(), 0_i64, "archived task or board"),
            (
                archived_board_task.id.as_str(),
                0_i64,
                "archived task or board",
            ),
            (stale.id.as_str(), 1_i64, "lock_version mismatch"),
        ] {
            let error = store
                .promote_task(
                    task_id,
                    promote_input(
                        expected_lock_version,
                        "promoter",
                        &format!("e_promote_archive_stale_{}", task_id),
                        100,
                    ),
                )
                .await
                .expect_err("archive/stale guard must fail");
            assert!(matches!(
                error,
                StoreError::InvalidTransition(error_message)
                    if error_message.contains(message)
            ));
        }

        let archived_task_after = store
            .get_task_global(&archived_task.id)
            .await
            .expect("get archived task");
        assert_eq!(archived_task_after.status, "archived");
        assert_eq!(archived_task_after.lock_version, 0);
        let archived_board_task_after = store
            .get_task_global(&archived_board_task.id)
            .await
            .expect("get archived board task");
        assert_eq!(archived_board_task_after.status, "todo");
        assert_eq!(archived_board_task_after.lock_version, 0);
        let stale_after = store
            .get_task_global(&stale.id)
            .await
            .expect("get stale task");
        assert_eq!(stale_after.status, "todo");
        assert_eq!(stale_after.lock_version, 0);
        assert_eq!(stale_after.execution_plan_state, "not_required");

        let promoted_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.promoted'",
                    (),
                )
                .await
                .expect("promoted event count query"),
        )
        .await
        .expect("promoted event count row");
        assert_eq!(
            integer_value(
                promoted_event_count
                    .get_value(0)
                    .expect("promoted event count"),
                "event.count",
            )
            .expect("promoted event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn promote_task_event_conflict_rolls_back_status_update() {
        let (_directory, store, _path) = store("promote-event-conflict").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input(
                    "t_promote_event_conflict",
                    Some("promote-event-conflict"),
                    "Promote event conflict",
                ),
            )
            .await
            .expect("create task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "No conflicting execution",
                    "planner",
                    "e_promote_conflict_plan",
                    100,
                ),
            )
            .await
            .expect("mark plan not required");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', ?2, NULL, 'other.event', 'tester', '{}', 1)",
                ("e_promote_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");

        let error = store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_promote_conflict", 200),
            )
            .await
            .expect_err("event conflict must fail");
        assert!(matches!(error, StoreError::Turso(_)));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get rolled back task");
        assert_eq!(unchanged.status, "todo");
        assert_eq!(unchanged.lock_version, 0);
        assert_eq!(unchanged.updated_at, task.updated_at);
        assert_eq!(unchanged.execution_plan_state, "not_required");

        let promoted_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.promoted'",
                    [task.id.as_str()],
                )
                .await
                .expect("promoted event count query"),
        )
        .await
        .expect("promoted event count row");
        assert_eq!(
            integer_value(
                promoted_event_count
                    .get_value(0)
                    .expect("promoted event count"),
                "event.count",
            )
            .expect("promoted event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn promote_task_uses_global_task_board() {
        let (_directory, store, _path) = store("promote-multi-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_other', 'other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let task = store
            .create_task(
                "other",
                create_input("t_promote_other", Some("promote-other"), "Other task"),
            )
            .await
            .expect("create other-board task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "No other-board execution",
                    "planner",
                    "e_promote_other_plan",
                    100,
                ),
            )
            .await
            .expect("mark other-board plan not required");

        let promoted = store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_promote_other", 200),
            )
            .await
            .expect("promote other-board task");
        assert_eq!(promoted.board_id, "b_other");
        assert_eq!(promoted.board_slug, "other");
        assert_eq!(promoted.task_ref, "other#1");
        assert_eq!(promoted.status, "ready");
        assert_eq!(promoted.lock_version, 1);

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_promote_other"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_other"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"to_status":"ready"}"#
        );
    }

    #[tokio::test]
    async fn promote_task_validates_global_input() {
        let (_directory, store, _path) = store("promote-input").await;
        store.initialize().await.expect("initialize");

        let invalid_id = store
            .promote_task(
                "default#1",
                promote_input(0, "promoter", "e_promote_input", 100),
            )
            .await
            .expect_err("board-local id must fail");
        assert!(matches!(
            invalid_id,
            StoreError::InvalidInput(message) if message.contains("task id")
        ));

        let invalid_version = store
            .promote_task(
                "t_promote_input",
                promote_input(-1, "promoter", "e_promote_input_version", 100),
            )
            .await
            .expect_err("negative version must fail");
        assert!(matches!(
            invalid_version,
            StoreError::InvalidInput(message) if message.contains("expected_lock_version")
        ));

        let invalid_actor = store
            .promote_task(
                "t_promote_input",
                promote_input(0, " ", "e_promote_input_actor", 100),
            )
            .await
            .expect_err("empty actor must fail");
        assert!(matches!(
            invalid_actor,
            StoreError::InvalidInput(message) if message.contains("actor")
        ));

        let invalid_event = store
            .promote_task(
                "t_promote_input",
                promote_input(0, "promoter", "promote_input_event", 100),
            )
            .await
            .expect_err("invalid event id must fail");
        assert!(matches!(
            invalid_event,
            StoreError::InvalidInput(message) if message.contains("event_id")
        ));

        let invalid_time = store
            .promote_task(
                "t_promote_input",
                promote_input(0, "promoter", "e_promote_input_time", -1),
            )
            .await
            .expect_err("negative time must fail");
        assert!(matches!(
            invalid_time,
            StoreError::InvalidInput(message) if message.contains("updated_at")
        ));
    }

    #[tokio::test]
    async fn claim_task_writes_running_task_run_and_event_atomically() {
        let (_directory, store, _path) = store("claim-success").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_claim_success", "claim-success", "Claim success").await;

        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_success",
                    "r_claim_success",
                    "e_claim_success",
                    r#"{"lane":"test"}"#,
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        assert_eq!(claimed.claim_token, "claim_success");
        assert_eq!(claimed.claim_expires_at, 1_300);
        assert_eq!(claimed.task.id, task.id);
        assert_eq!(claimed.task.status, "running");
        assert_eq!(claimed.task.lock_version, 2);
        assert_eq!(claimed.task.claim_token.as_deref(), Some("claim_success"));
        assert_eq!(claimed.task.claim_owner.as_deref(), Some("worker"));
        assert_eq!(claimed.task.claim_expires_at, Some(1_300));
        assert_eq!(claimed.task.last_heartbeat_at, Some(300));
        assert_eq!(claimed.task.started_at, Some(300));
        assert_eq!(
            claimed.task.current_run_id.as_deref(),
            Some("r_claim_success")
        );
        assert_eq!(claimed.run.id, "r_claim_success");
        assert_eq!(claimed.run.board_id, "b_default");
        assert_eq!(claimed.run.task_id, task.id);
        assert_eq!(claimed.run.status, "running");
        assert_eq!(claimed.run.worker_profile.as_deref(), Some("manual"));
        assert_eq!(claimed.run.claim_token, "claim_success");
        assert_eq!(claimed.run.claim_owner, "worker");
        assert_eq!(claimed.run.claim_expires_at, 1_300);
        assert_eq!(claimed.run.started_at, 300);
        assert_eq!(claimed.run.last_heartbeat_at, Some(300));
        assert_eq!(claimed.run.metadata_json, r#"{"lane":"test"}"#);

        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "task_runs").await, 1);
        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_claim_success"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_default"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_claim_success"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.claimed"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "worker"
        );
        assert_eq!(
            text_value(event.get_value(5).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"claim_owner":"worker","metadata":{"lane":"test"}}"#
        );
        assert_eq!(
            integer_value(
                event.get_value(6).expect("event created_at"),
                "event.created_at"
            )
            .expect("event created_at integer"),
            300
        );
    }

    #[tokio::test]
    async fn claim_task_persists_optional_log_path_and_rejects_blank_without_writes() {
        let (_directory, store, _path) = store("claim-log-path").await;
        store.initialize().await.expect("initialize");

        let with_path = ready_task_for_claim(
            &store,
            "t_claim_log_path",
            "claim-log-path",
            "With log path",
        )
        .await;
        let mut with_path_input = claim_input(
            1,
            "worker",
            "claim_log_path",
            "r_claim_log_path",
            "e_claim_log_path",
            "{}",
            300,
            1_000,
        );
        with_path_input.log_path = Some(" /tmp/claim.log ".to_owned());
        let claimed_with_path = store
            .claim_task(&with_path.id, with_path_input)
            .await
            .expect("claim with log path");
        assert_eq!(
            claimed_with_path.run.log_path.as_deref(),
            Some("/tmp/claim.log")
        );

        let none_path =
            ready_task_for_claim(&store, "t_claim_no_log_path", "claim-no-log", "No log path")
                .await;
        let claimed_without_path = store
            .claim_task(
                &none_path.id,
                claim_input(
                    1,
                    "worker",
                    "claim_no_log_path",
                    "r_claim_no_log_path",
                    "e_claim_no_log_path",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim without log path");
        assert_eq!(claimed_without_path.run.log_path, None);

        let invalid_task = ready_task_for_claim(
            &store,
            "t_claim_blank_log_path",
            "claim-blank-log",
            "Blank log path",
        )
        .await;
        let mut invalid_input = claim_input(
            1,
            "worker",
            "claim_blank_log_path",
            "r_claim_blank_log_path",
            "e_claim_blank_log_path",
            "{}",
            300,
            1_000,
        );
        invalid_input.log_path = Some(" \t ".to_owned());
        let error = store
            .claim_task(&invalid_task.id, invalid_input)
            .await
            .expect_err("blank log path must fail");
        assert!(matches!(
            error,
            StoreError::InvalidInput(message) if message.contains("log_path")
        ));
        let unchanged = store
            .get_task_global(&invalid_task.id)
            .await
            .expect("get unchanged task");
        assert_eq!(unchanged.status, "ready");
        assert_eq!(unchanged.lock_version, invalid_task.lock_version);
        let connection = store.connection().await.expect("connection");
        let run_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_runs WHERE task_id = ?1",
                    [invalid_task.id.as_str()],
                )
                .await
                .expect("run count query"),
        )
        .await
        .expect("run count row");
        assert_eq!(
            integer_value(run_count.get_value(0).expect("run count"), "run.count")
                .expect("run count integer"),
            0
        );
        let event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.claimed'",
                    [invalid_task.id.as_str()],
                )
                .await
                .expect("event count query"),
        )
        .await
        .expect("event count row");
        assert_eq!(
            integer_value(
                event_count.get_value(0).expect("event count"),
                "event.count"
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn claim_task_concurrent_callers_have_exactly_one_winner() {
        let (_directory, store, _path) = store("claim-race").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(&store, "t_claim_race", "claim-race", "Claim race").await;
        let first_store = store.clone();
        let second_store = store.clone();
        let (first, second) = tokio::join!(
            first_store.claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker-a",
                    "claim_race_a",
                    "r_claim_race_a",
                    "e_claim_race_a",
                    "{}",
                    300,
                    1_000,
                ),
            ),
            second_store.claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker-b",
                    "claim_race_b",
                    "r_claim_race_b",
                    "e_claim_race_b",
                    "{}",
                    300,
                    1_000,
                ),
            )
        );
        let winners = usize::from(first.is_ok()) + usize::from(second.is_ok());
        assert_eq!(winners, 1);
        for result in [first, second] {
            if let Err(error) = result {
                assert!(matches!(
                    error,
                    StoreError::ClaimConflict(_) | StoreError::InvalidTransition(_)
                ));
            }
        }

        let connection = store.connection().await.expect("connection");
        let active_runs = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_runs WHERE task_id = ?1 AND status = 'running'",
                    [task.id.as_str()],
                )
                .await
                .expect("active run count query"),
        )
        .await
        .expect("active run count row");
        assert_eq!(
            integer_value(
                active_runs.get_value(0).expect("active run count"),
                "run.count"
            )
            .expect("active run count integer"),
            1
        );
        let claimed_events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.claimed'",
                    [task.id.as_str()],
                )
                .await
                .expect("claimed event count query"),
        )
        .await
        .expect("claimed event count row");
        assert_eq!(
            integer_value(
                claimed_events.get_value(0).expect("claimed event count"),
                "event.count",
            )
            .expect("claimed event count integer"),
            1
        );
        let claimed = store
            .get_task_global(&task.id)
            .await
            .expect("get claimed task");
        assert_eq!(claimed.status, "running");
        assert_eq!(claimed.lock_version, 2);
    }

    #[tokio::test]
    async fn claim_task_validates_token_run_event_metadata_and_ttl_input() {
        let (_directory, store, _path) = store("claim-input").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_claim_input", "claim-input", "Claim input").await;

        let mut cases = vec![
            (
                "claim_token",
                claim_input(
                    1,
                    "worker",
                    "",
                    "r_claim_input_token",
                    "e_claim_input_token",
                    "{}",
                    300,
                    1_000,
                ),
            ),
            (
                "run_id",
                claim_input(
                    1,
                    "worker",
                    "claim_input_run",
                    "bad-run",
                    "e_claim_input_run",
                    "{}",
                    300,
                    1_000,
                ),
            ),
            (
                "event_id",
                claim_input(
                    1,
                    "worker",
                    "claim_input_event",
                    "r_claim_input_event",
                    "bad-event",
                    "{}",
                    300,
                    1_000,
                ),
            ),
            (
                "owner",
                claim_input(
                    1,
                    " ",
                    "claim_input_owner",
                    "r_claim_input_owner",
                    "e_claim_input_owner",
                    "{}",
                    300,
                    1_000,
                ),
            ),
            (
                "claim_expires_at",
                claim_input(
                    1,
                    "worker",
                    "claim_input_ttl",
                    "r_claim_input_ttl",
                    "e_claim_input_ttl",
                    "{}",
                    300,
                    0,
                ),
            ),
        ];
        let mut invalid_profile = claim_input(
            1,
            "worker",
            "claim_input_profile",
            "r_claim_input_profile",
            "e_claim_input_profile",
            "{}",
            300,
            1_000,
        );
        invalid_profile.worker_profile = " ".to_owned();
        cases.push(("worker_profile", invalid_profile));
        let mut invalid_metadata = claim_input(
            1,
            "worker",
            "claim_input_metadata",
            "r_claim_input_metadata",
            "e_claim_input_metadata",
            "{bad",
            300,
            1_000,
        );
        invalid_metadata.metadata_json = "{bad".to_owned();
        cases.push(("metadata_json", invalid_metadata));

        for (field, input) in cases {
            let error = store
                .claim_task(&task.id, input)
                .await
                .expect_err("invalid claim input must fail");
            assert!(matches!(error, StoreError::InvalidInput(message) if message.contains(field)));
        }
        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged input task");
        assert_eq!(unchanged.status, "ready");
        assert_eq!(unchanged.lock_version, 1);
        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "task_runs").await, 0);
        let event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.claimed'",
                    [task.id.as_str()],
                )
                .await
                .expect("claimed event count query"),
        )
        .await
        .expect("claimed event count row");
        assert_eq!(
            integer_value(
                event_count.get_value(0).expect("event count"),
                "event.count"
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn claim_task_rejects_source_plan_spec_schedule_dependency_and_archive_guards() {
        let (_directory, store, _path) = store("claim-guards").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");

        let unplanned = store
            .create_task(
                "default",
                create_input(
                    "t_claim_unplanned",
                    Some("claim-unplanned"),
                    "Claim unplanned",
                ),
            )
            .await
            .expect("create unplanned task");
        connection
            .execute(
                "UPDATE tasks SET status = 'ready' WHERE id = ?1",
                [unplanned.id.as_str()],
            )
            .await
            .expect("make unplanned task ready");

        let source = store
            .create_task(
                "default",
                create_input("t_claim_source", Some("claim-source"), "Claim source"),
            )
            .await
            .expect("create source task");
        store
            .mark_execution_plan_not_required(
                &source.id,
                plan_input("No source execution", "planner", "e_claim_source_plan", 100),
            )
            .await
            .expect("mark source plan not required");

        let mut incomplete_input = create_input(
            "t_claim_incomplete",
            Some("claim-incomplete"),
            "Claim incomplete",
        );
        incomplete_input.description = None;
        let incomplete = store
            .create_task("default", incomplete_input)
            .await
            .expect("create incomplete task");
        store
            .mark_execution_plan_not_required(
                &incomplete.id,
                plan_input(
                    "No incomplete execution",
                    "planner",
                    "e_claim_incomplete_plan",
                    100,
                ),
            )
            .await
            .expect("mark incomplete plan not required");
        connection
            .execute(
                "UPDATE tasks SET status = 'ready' WHERE id = ?1",
                [incomplete.id.as_str()],
            )
            .await
            .expect("make incomplete task ready");

        let mut future_input = create_input("t_claim_future", Some("claim-future"), "Claim future");
        future_input.scheduled_at = Some(500);
        let future = store
            .create_task("default", future_input)
            .await
            .expect("create future task");
        store
            .mark_execution_plan_not_required(
                &future.id,
                plan_input("No future execution", "planner", "e_claim_future_plan", 100),
            )
            .await
            .expect("mark future plan not required");
        connection
            .execute(
                "UPDATE tasks SET status = 'ready' WHERE id = ?1",
                [future.id.as_str()],
            )
            .await
            .expect("make future task ready");

        let parent = store
            .create_task(
                "default",
                create_input("t_claim_parent", Some("claim-parent"), "Claim parent"),
            )
            .await
            .expect("create dependency parent");
        let dependency = store
            .create_task(
                "default",
                create_input(
                    "t_claim_dependency",
                    Some("claim-dependency"),
                    "Claim dependency",
                ),
            )
            .await
            .expect("create dependency child");
        store
            .mark_execution_plan_not_required(
                &dependency.id,
                plan_input(
                    "No dependency execution",
                    "planner",
                    "e_claim_dependency_plan",
                    100,
                ),
            )
            .await
            .expect("mark dependency plan not required");
        connection
            .execute(
                "UPDATE tasks SET status = 'ready' WHERE id = ?1",
                [dependency.id.as_str()],
            )
            .await
            .expect("make dependency task ready");
        connection
            .execute(
                "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES ('b_default', ?1, ?2, 1)",
                (parent.id.as_str(), dependency.id.as_str()),
            )
            .await
            .expect("insert unfinished dependency");

        let archived = store
            .create_task(
                "default",
                create_input("t_claim_archived", Some("claim-archived"), "Claim archived"),
            )
            .await
            .expect("create archived task");
        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 400 WHERE id = ?1",
                [archived.id.as_str()],
            )
            .await
            .expect("archive claim task");

        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at, archived_at) VALUES ('b_claim_archived', 'claim-archived-board', 'Claim archived board', 1, 1, 450)",
                (),
            )
            .await
            .expect("insert archived claim board");
        let archived_board = store
            .create_task(
                "claim-archived-board",
                create_input(
                    "t_claim_archived_board",
                    Some("claim-archived-board"),
                    "Claim archived board",
                ),
            )
            .await
            .expect("create task on archived claim board");

        let cases = [
            (unplanned.id.as_str(), "execution plan"),
            (source.id.as_str(), "not ready"),
            (incomplete.id.as_str(), "task spec"),
            (future.id.as_str(), "future"),
            (dependency.id.as_str(), "dependency"),
            (archived.id.as_str(), "archived"),
            (archived_board.id.as_str(), "archived"),
        ];
        for (index, (task_id, message)) in cases.into_iter().enumerate() {
            let error = store
                .claim_task(
                    task_id,
                    claim_input(
                        0,
                        "worker",
                        &format!("claim_guard_{index}"),
                        &format!("r_claim_guard_{index}"),
                        &format!("e_claim_guard_{index}"),
                        "{}",
                        100,
                        1_000,
                    ),
                )
                .await
                .expect_err("claim guard must fail");
            assert!(matches!(
                error,
                StoreError::InvalidTransition(error_message)
                    if error_message.contains(message)
            ));
        }

        for (task_id, expected_status, expected_plan) in [
            (unplanned.id.as_str(), "ready", "unplanned"),
            (source.id.as_str(), "todo", "not_required"),
            (incomplete.id.as_str(), "ready", "not_required"),
            (future.id.as_str(), "ready", "not_required"),
            (dependency.id.as_str(), "ready", "not_required"),
            (archived.id.as_str(), "archived", "unplanned"),
            (archived_board.id.as_str(), "todo", "unplanned"),
        ] {
            let unchanged = store
                .get_task_global(task_id)
                .await
                .expect("get unchanged guard task");
            assert_eq!(unchanged.status, expected_status, "task {task_id}");
            assert_eq!(unchanged.lock_version, 0, "task {task_id}");
            assert_eq!(
                unchanged.execution_plan_state, expected_plan,
                "task {task_id}"
            );
        }
        assert_eq!(count_rows(&connection, "task_runs").await, 0);
        let claimed_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.claimed'",
                    (),
                )
                .await
                .expect("claimed event count query"),
        )
        .await
        .expect("claimed event count row");
        assert_eq!(
            integer_value(
                claimed_event_count
                    .get_value(0)
                    .expect("claimed event count"),
                "event.count",
            )
            .expect("claimed event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn claim_task_run_id_conflict_rolls_back_task_update() {
        let (_directory, store, _path) = store("claim-run-conflict").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_claim_run_conflict",
            "claim-run-conflict",
            "Claim run conflict",
        )
        .await;
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_runs(id, board_id, task_id, status, worker_profile, worker_pid, claim_token, claim_owner, claim_expires_at, started_at, metadata_json) VALUES (?1, 'b_default', ?2, 'succeeded', 'previous', NULL, 'claim_previous', 'previous-worker', 500, 100, '{}')",
                ("r_claim_run_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting run");

        let error = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_run_conflict",
                    "r_claim_run_conflict",
                    "e_claim_run_conflict",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect_err("run id conflict must fail");
        assert!(matches!(error, StoreError::Turso(_)));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get rolled back run-conflict task");
        assert_eq!(unchanged.status, "ready");
        assert_eq!(unchanged.lock_version, 1);
        assert_eq!(unchanged.claim_token, None);
        assert_eq!(unchanged.current_run_id, None);
        assert_eq!(count_rows(&connection, "task_runs").await, 1);
        let claimed_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.claimed'",
                    [task.id.as_str()],
                )
                .await
                .expect("claimed event count query"),
        )
        .await
        .expect("claimed event count row");
        assert_eq!(
            integer_value(
                claimed_event_count
                    .get_value(0)
                    .expect("claimed event count"),
                "event.count",
            )
            .expect("claimed event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn claim_task_event_conflict_rolls_back_task_and_run_update() {
        let (_directory, store, _path) = store("claim-event-conflict").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_claim_event_conflict",
            "claim-event-conflict",
            "Claim event conflict",
        )
        .await;
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', ?2, NULL, 'other.event', 'tester', '{}', 1)",
                ("e_claim_event_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");

        let error = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_event_conflict",
                    "r_claim_event_conflict",
                    "e_claim_event_conflict",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect_err("event conflict must fail");
        assert!(matches!(error, StoreError::Turso(_)));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get rolled back event-conflict task");
        assert_eq!(unchanged.status, "ready");
        assert_eq!(unchanged.lock_version, 1);
        assert_eq!(unchanged.claim_token, None);
        assert_eq!(unchanged.current_run_id, None);
        assert_eq!(count_rows(&connection, "task_runs").await, 0);
        let claimed_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.claimed'",
                    [task.id.as_str()],
                )
                .await
                .expect("claimed event count query"),
        )
        .await
        .expect("claimed event count row");
        assert_eq!(
            integer_value(
                claimed_event_count
                    .get_value(0)
                    .expect("claimed event count"),
                "event.count",
            )
            .expect("claimed event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn claim_task_uses_global_task_board_for_run_and_event() {
        let (_directory, store, _path) = store("claim-multi-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_claim_other', 'claim-other', 'Claim other', 1, 1)",
                (),
            )
            .await
            .expect("insert other claim board");
        let task = store
            .create_task(
                "claim-other",
                create_input("t_claim_other", Some("claim-other"), "Claim other task"),
            )
            .await
            .expect("create other-board claim task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "No other-board claim plan",
                    "planner",
                    "e_claim_other_plan",
                    100,
                ),
            )
            .await
            .expect("mark other-board plan not required");
        store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_claim_other_promote", 200),
            )
            .await
            .expect("promote other-board claim task");

        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "other-worker",
                    "claim_other",
                    "r_claim_other",
                    "e_claim_other",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim other-board task");
        assert_eq!(claimed.task.board_id, "b_claim_other");
        assert_eq!(claimed.task.board_slug, "claim-other");
        assert_eq!(claimed.run.board_id, "b_claim_other");
        assert_eq!(claimed.run.task_id, task.id);

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id FROM task_events WHERE event_id = ?1",
                    ["e_claim_other"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_claim_other"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_claim_other"
        );
    }

    #[tokio::test]
    async fn heartbeat_task_extends_task_and_run_and_writes_note_event_atomically() {
        let (_directory, store, _path) = store("heartbeat-success").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_heartbeat_success",
            "heartbeat-success",
            "Heartbeat success",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_heartbeat_success",
                    "r_heartbeat_success",
                    "e_heartbeat_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");

        let heartbeated = store
            .heartbeat_task(
                &task.id,
                heartbeat_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_heartbeat_success",
                    "e_heartbeat_success",
                    Some("still alive"),
                    500,
                    1_500,
                ),
            )
            .await
            .expect("heartbeat task");
        assert_eq!(heartbeated.status, "running");
        assert_eq!(heartbeated.claim_expires_at, Some(1_500));
        assert_eq!(heartbeated.last_heartbeat_at, Some(500));
        assert_eq!(heartbeated.updated_at, 500);
        assert_eq!(heartbeated.lock_version, claimed.task.lock_version + 1);

        let connection = store.connection().await.expect("connection");
        let run = first_row(
            connection
                .query(
                    "SELECT claim_expires_at, last_heartbeat_at FROM task_runs WHERE id = ?1",
                    ["r_heartbeat_success"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            integer_value(run.get_value(0).expect("run expiry"), "run.expiry")
                .expect("run expiry integer"),
            1_500
        );
        assert_eq!(
            integer_value(run.get_value(1).expect("run heartbeat"), "run.heartbeat")
                .expect("run heartbeat integer"),
            500
        );

        let event = first_row(
            connection
                .query(
                    "SELECT kind, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_heartbeat_success"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.heartbeat"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"note":"still alive"}"#
        );
    }

    #[tokio::test]
    async fn heartbeat_task_rejects_credentials_and_damaged_state_without_writes() {
        let (_directory, store, _path) = store("heartbeat-guards").await;
        store.initialize().await.expect("initialize");

        let task = ready_task_for_claim(
            &store,
            "t_heartbeat_guards_running",
            "heartbeat-guards-running",
            "Heartbeat guards running",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_heartbeat_guards",
                    "r_heartbeat_guards",
                    "e_heartbeat_guards_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim running task");

        let wrong_token = store
            .heartbeat_task(
                &task.id,
                heartbeat_input(
                    claimed.task.lock_version,
                    "worker",
                    "secret-token-that-must-not-leak",
                    "e_heartbeat_wrong_token",
                    None,
                    500,
                    1_500,
                ),
            )
            .await
            .expect_err("wrong token must fail");
        assert!(matches!(wrong_token, StoreError::ClaimTokenMismatch));
        assert!(
            !wrong_token
                .to_string()
                .contains("secret-token-that-must-not-leak")
        );

        let padded_token = store
            .heartbeat_task(
                &task.id,
                heartbeat_input(
                    claimed.task.lock_version,
                    "worker",
                    " claim_heartbeat_guards ",
                    "e_heartbeat_padded_token",
                    None,
                    500,
                    1_500,
                ),
            )
            .await
            .expect_err("padded token must not be normalized");
        assert!(matches!(padded_token, StoreError::ClaimTokenMismatch));

        let wrong_owner = store
            .heartbeat_task(
                &task.id,
                heartbeat_input(
                    claimed.task.lock_version,
                    "other-worker",
                    "claim_heartbeat_guards",
                    "e_heartbeat_wrong_owner",
                    None,
                    500,
                    1_500,
                ),
            )
            .await
            .expect_err("wrong owner must fail");
        assert!(matches!(
            wrong_owner,
            StoreError::InvalidTransition(message) if message.contains("owner")
        ));

        let after_credentials = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged running task");
        assert_eq!(after_credentials.lock_version, claimed.task.lock_version);
        assert_eq!(
            after_credentials.claim_expires_at,
            claimed.task.claim_expires_at
        );
        assert_eq!(
            after_credentials.last_heartbeat_at,
            claimed.task.last_heartbeat_at
        );

        let ready = ready_task_for_claim(
            &store,
            "t_heartbeat_guards_ready",
            "heartbeat-guards-ready",
            "Heartbeat guards ready",
        )
        .await;
        let non_running = store
            .heartbeat_task(
                &ready.id,
                heartbeat_input(
                    ready.lock_version,
                    "worker",
                    "claim_never_created",
                    "e_heartbeat_non_running",
                    None,
                    500,
                    1_500,
                ),
            )
            .await
            .expect_err("non-running task must fail");
        assert!(matches!(
            non_running,
            StoreError::InvalidTransition(message) if message.contains("running")
        ));

        let missing_run = ready_task_for_claim(
            &store,
            "t_heartbeat_guards_missing_run",
            "heartbeat-guards-missing-run",
            "Heartbeat guards missing run",
        )
        .await;
        let missing_claim = store
            .claim_task(
                &missing_run.id,
                claim_input(
                    1,
                    "worker",
                    "claim_heartbeat_missing_run",
                    "r_heartbeat_missing_run",
                    "e_heartbeat_missing_run_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim missing-run task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE tasks SET current_run_id = NULL WHERE id = ?1",
                [missing_run.id.as_str()],
            )
            .await
            .expect("remove current run id");
        let missing_run_error = store
            .heartbeat_task(
                &missing_run.id,
                heartbeat_input(
                    missing_claim.task.lock_version,
                    "worker",
                    "claim_heartbeat_missing_run",
                    "e_heartbeat_missing_run",
                    None,
                    500,
                    1_500,
                ),
            )
            .await
            .expect_err("missing run must fail");
        assert!(matches!(
            missing_run_error,
            StoreError::InvalidTransition(message) if message.contains("current running run")
        ));
        let unchanged_missing_run = store
            .get_task_global(&missing_run.id)
            .await
            .expect("get missing-run task");
        assert_eq!(
            unchanged_missing_run.lock_version,
            missing_claim.task.lock_version
        );
        assert_eq!(
            unchanged_missing_run.claim_expires_at,
            missing_claim.task.claim_expires_at
        );
        assert_eq!(
            unchanged_missing_run.last_heartbeat_at,
            missing_claim.task.last_heartbeat_at
        );

        connection
            .execute(
                "UPDATE task_runs SET status = 'succeeded' WHERE id = ?1",
                ["r_heartbeat_guards"],
            )
            .await
            .expect("damage active run status");
        let damaged_run_error = store
            .heartbeat_task(
                &task.id,
                heartbeat_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_heartbeat_guards",
                    "e_heartbeat_damaged_run",
                    None,
                    500,
                    1_500,
                ),
            )
            .await
            .expect_err("damaged run must fail");
        assert!(matches!(
            damaged_run_error,
            StoreError::InvalidTransition(_)
        ));

        let event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.heartbeat'",
                    (),
                )
                .await
                .expect("heartbeat event count query"),
        )
        .await
        .expect("heartbeat event count row");
        assert_eq!(
            integer_value(
                event_count.get_value(0).expect("event count"),
                "event.count"
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn heartbeat_task_validates_input_without_opening_a_mutation_path() {
        let (_directory, store, _path) = store("heartbeat-input").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_heartbeat_input",
            "heartbeat-input",
            "Heartbeat input",
        )
        .await;
        let cases = [
            (
                "task id",
                "default#1".to_owned(),
                heartbeat_input(1, "worker", "claim_input", "e_input", None, 300, 1_500),
            ),
            (
                "expected_lock_version",
                task.id.clone(),
                heartbeat_input(
                    -1,
                    "worker",
                    "claim_input",
                    "e_input_version",
                    None,
                    300,
                    1_500,
                ),
            ),
            (
                "actor",
                task.id.clone(),
                heartbeat_input(1, " ", "claim_input", "e_input_actor", None, 300, 1_500),
            ),
            (
                "claim_token",
                task.id.clone(),
                heartbeat_input(1, "worker", " ", "e_input_token", None, 300, 1_500),
            ),
            (
                "event_id",
                task.id.clone(),
                heartbeat_input(1, "worker", "claim_input", "input_event", None, 300, 1_500),
            ),
            (
                "now",
                task.id.clone(),
                heartbeat_input(1, "worker", "claim_input", "e_input_now", None, -1, 1_500),
            ),
            (
                "claim_expires_at",
                task.id.clone(),
                heartbeat_input(1, "worker", "claim_input", "e_input_expiry", None, 300, 300),
            ),
        ];
        for (field, task_id, input) in cases {
            let error = store
                .heartbeat_task(&task_id, input)
                .await
                .expect_err("invalid heartbeat input must fail");
            assert!(matches!(error, StoreError::InvalidInput(message) if message.contains(field)));
        }
        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged input task");
        assert_eq!(unchanged.status, "ready");
        assert_eq!(unchanged.lock_version, task.lock_version);
        let connection = store.connection().await.expect("connection");
        assert_eq!(count_rows(&connection, "task_runs").await, 0);
        let heartbeat_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.heartbeat'",
                    (),
                )
                .await
                .expect("heartbeat event count query"),
        )
        .await
        .expect("heartbeat event count row");
        assert_eq!(
            integer_value(
                heartbeat_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn heartbeat_task_event_conflict_rolls_back_task_and_run_updates() {
        let (_directory, store, _path) = store("heartbeat-event-conflict").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_heartbeat_event_conflict",
            "heartbeat-event-conflict",
            "Heartbeat event conflict",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_heartbeat_event_conflict",
                    "r_heartbeat_event_conflict",
                    "e_heartbeat_event_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', ?2, NULL, 'other.event', 'tester', '{}', 1)",
                ("e_heartbeat_event_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");

        let error = store
            .heartbeat_task(
                &task.id,
                heartbeat_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_heartbeat_event_conflict",
                    "e_heartbeat_event_conflict",
                    Some("must roll back"),
                    500,
                    1_500,
                ),
            )
            .await
            .expect_err("event conflict must fail");
        assert!(matches!(error, StoreError::Turso(_)));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get rolled back task");
        assert_eq!(unchanged.lock_version, claimed.task.lock_version);
        assert_eq!(unchanged.claim_expires_at, claimed.task.claim_expires_at);
        assert_eq!(unchanged.last_heartbeat_at, claimed.task.last_heartbeat_at);
        let run = first_row(
            connection
                .query(
                    "SELECT claim_expires_at, last_heartbeat_at FROM task_runs WHERE id = ?1",
                    ["r_heartbeat_event_conflict"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            integer_value(run.get_value(0).expect("run expiry"), "run.expiry")
                .expect("run expiry integer"),
            claimed.run.claim_expires_at
        );
        assert_eq!(
            integer_value(run.get_value(1).expect("run heartbeat"), "run.heartbeat")
                .expect("run heartbeat integer"),
            claimed.run.last_heartbeat_at.expect("claimed heartbeat")
        );
        let heartbeat_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.heartbeat'",
                    [task.id.as_str()],
                )
                .await
                .expect("heartbeat event count query"),
        )
        .await
        .expect("heartbeat event count row");
        assert_eq!(
            integer_value(
                heartbeat_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn heartbeat_task_uses_global_task_board_for_run_and_event() {
        let (_directory, store, _path) = store("heartbeat-multi-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_heartbeat_other', 'heartbeat-other', 'Heartbeat other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let task = store
            .create_task(
                "heartbeat-other",
                create_input(
                    "t_heartbeat_other",
                    Some("heartbeat-other"),
                    "Heartbeat other task",
                ),
            )
            .await
            .expect("create other-board task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "No other-board heartbeat plan",
                    "planner",
                    "e_heartbeat_other_plan",
                    100,
                ),
            )
            .await
            .expect("mark plan not required");
        store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_heartbeat_other_promote", 200),
            )
            .await
            .expect("promote other-board task");
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "other-worker",
                    "claim_heartbeat_other",
                    "r_heartbeat_other",
                    "e_heartbeat_other_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim other-board task");
        let heartbeated = store
            .heartbeat_task(
                &task.id,
                heartbeat_input(
                    claimed.task.lock_version,
                    "other-worker",
                    "claim_heartbeat_other",
                    "e_heartbeat_other",
                    None,
                    500,
                    1_500,
                ),
            )
            .await
            .expect("heartbeat other-board task");
        assert_eq!(heartbeated.board_id, "b_heartbeat_other");
        assert_eq!(heartbeated.board_slug, "heartbeat-other");

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_heartbeat_other"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_heartbeat_other"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_heartbeat_other"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"note":null}"#
        );
    }

    #[tokio::test]
    async fn release_task_returns_ready_and_cancels_run_atomically() {
        let (_directory, store, _path) = store("release-success").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_release_success",
            "release-success",
            "Release success",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_release_success",
                    "r_release_success",
                    "e_release_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");

        let released = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_release_success",
                    "e_release_success",
                    500,
                ),
            )
            .await
            .expect("release task");
        assert_eq!(released.id, task.id);
        assert_eq!(released.status, "ready");
        assert_eq!(released.status_reason, None);
        assert_eq!(released.claim_token, None);
        assert_eq!(released.claim_owner, None);
        assert_eq!(released.claim_expires_at, None);
        assert_eq!(released.last_heartbeat_at, None);
        assert_eq!(released.current_run_id, None);
        assert_eq!(released.started_at, claimed.task.started_at);
        assert_eq!(released.retry_count, claimed.task.retry_count);
        assert_eq!(released.updated_at, 500);
        assert_eq!(released.lock_version, claimed.task.lock_version + 1);

        let connection = store.connection().await.expect("connection");
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, error FROM task_runs WHERE id = ?1",
                    ["r_release_success"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "canceled"
        );
        assert_eq!(
            integer_value(
                run.get_value(1).expect("run finished_at"),
                "run.finished_at"
            )
            .expect("run finished_at integer"),
            500
        );
        assert_eq!(
            optional_text_value(run.get_value(2).expect("run error"), "run.error")
                .expect("run error text")
                .as_deref(),
            None
        );

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_release_success"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_default"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_release_success"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.released"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "worker"
        );
        assert_eq!(
            text_value(event.get_value(5).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"to_status":"ready"}"#
        );
        assert_eq!(
            integer_value(
                event.get_value(6).expect("event created_at"),
                "event.created_at"
            )
            .expect("event created_at integer"),
            500
        );
    }

    #[tokio::test]
    async fn release_task_rejects_credentials_and_guards_without_writes() {
        let (_directory, store, _path) = store("release-guards").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_release_guards_running",
            "release-guards-running",
            "Release guards running",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_release_guards",
                    "r_release_guards",
                    "e_release_guards_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim running task");
        let connection = store.connection().await.expect("connection");

        let wrong_token = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "worker",
                    "secret-release-token",
                    "e_release_wrong_token",
                    500,
                ),
            )
            .await
            .expect_err("wrong token must fail");
        assert!(matches!(wrong_token, StoreError::ClaimTokenMismatch));
        assert!(!wrong_token.to_string().contains("secret-release-token"));

        let padded_token = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "worker",
                    " claim_release_guards ",
                    "e_release_padded_token",
                    500,
                ),
            )
            .await
            .expect_err("padded token must not be normalized");
        assert!(matches!(padded_token, StoreError::ClaimTokenMismatch));

        let wrong_owner = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "other-worker",
                    "claim_release_guards",
                    "e_release_wrong_owner",
                    500,
                ),
            )
            .await
            .expect_err("wrong owner must fail");
        assert!(matches!(
            wrong_owner,
            StoreError::InvalidTransition(message) if message.contains("owner")
        ));

        let after_credentials = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged running task");
        assert_eq!(after_credentials.status, "running");
        assert_eq!(after_credentials.lock_version, claimed.task.lock_version);
        assert_eq!(after_credentials.claim_token, claimed.task.claim_token);
        assert_eq!(
            after_credentials.current_run_id,
            claimed.task.current_run_id
        );

        connection
            .execute(
                "UPDATE tasks SET status = 'ready' WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("make task non-running");
        let non_running = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_release_guards",
                    "e_release_non_running",
                    500,
                ),
            )
            .await
            .expect_err("non-running task must fail");
        assert!(matches!(
            non_running,
            StoreError::InvalidTransition(message) if message.contains("running")
        ));
        connection
            .execute(
                "UPDATE tasks SET status = 'running' WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("restore running task");

        connection
            .execute(
                "UPDATE tasks SET current_run_id = NULL WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("remove current run id");
        let missing_run_error = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_release_guards",
                    "e_release_missing_run",
                    500,
                ),
            )
            .await
            .expect_err("missing run must fail");
        assert!(matches!(
            missing_run_error,
            StoreError::InvalidTransition(message) if message.contains("current running run")
        ));
        connection
            .execute(
                "UPDATE tasks SET current_run_id = ?1 WHERE id = ?2",
                ("r_release_guards", task.id.as_str()),
            )
            .await
            .expect("restore current run id");

        let unplanned_claim = store
            .get_task_global(&task.id)
            .await
            .expect("get task before plan guard");
        connection
            .execute(
                "UPDATE task_execution_plans SET state = 'unplanned' WHERE task_id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("make plan unplanned");
        let unplanned_error = store
            .release_task(
                &task.id,
                release_input(
                    unplanned_claim.lock_version,
                    "worker",
                    "claim_release_guards",
                    "e_release_unplanned",
                    500,
                ),
            )
            .await
            .expect_err("unplanned task must not release to ready");
        assert!(matches!(
            unplanned_error,
            StoreError::InvalidTransition(message) if message.contains("execution plan")
        ));
        connection
            .execute(
                "UPDATE task_execution_plans SET state = 'not_required' WHERE task_id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("restore execution plan");

        let dependency_parent = store
            .create_task(
                "default",
                create_input(
                    "t_release_dependency_parent",
                    Some("release-dependency-parent"),
                    "Release dependency parent",
                ),
            )
            .await
            .expect("create dependency parent");
        connection
            .execute(
                "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES ('b_default', ?1, ?2, 1)",
                (dependency_parent.id.as_str(), task.id.as_str()),
            )
            .await
            .expect("insert dependency");
        let dependency_error = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_release_guards",
                    "e_release_dependency",
                    500,
                ),
            )
            .await
            .expect_err("dependency-blocked task must not release to ready");
        assert!(matches!(
            dependency_error,
            StoreError::InvalidTransition(message) if message.contains("dependency")
        ));
        connection
            .execute(
                "DELETE FROM task_dependencies WHERE parent_task_id = ?1 AND child_task_id = ?2",
                (dependency_parent.id.as_str(), task.id.as_str()),
            )
            .await
            .expect("remove dependency");

        connection
            .execute(
                "UPDATE tasks SET scheduled_at = 1_000 WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("set future schedule");
        let future_error = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_release_guards",
                    "e_release_future",
                    500,
                ),
            )
            .await
            .expect_err("future schedule must fail");
        assert!(matches!(
            future_error,
            StoreError::InvalidTransition(message) if message.contains("future")
        ));

        let release_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.released'",
                    (),
                )
                .await
                .expect("release event count query"),
        )
        .await
        .expect("release event count row");
        assert_eq!(
            integer_value(
                release_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn release_task_event_conflict_rolls_back_task_and_run_updates() {
        let (_directory, store, _path) = store("release-event-conflict").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_release_event_conflict",
            "release-event-conflict",
            "Release event conflict",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_release_event_conflict",
                    "r_release_event_conflict",
                    "e_release_event_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', ?2, NULL, 'other.event', 'tester', '{}', 1)",
                ("e_release_event_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");

        let error = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_release_event_conflict",
                    "e_release_event_conflict",
                    500,
                ),
            )
            .await
            .expect_err("event conflict must fail");
        assert!(matches!(error, StoreError::Turso(_)));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get rolled back task");
        assert_eq!(unchanged.status, "running");
        assert_eq!(unchanged.lock_version, claimed.task.lock_version);
        assert_eq!(unchanged.claim_expires_at, claimed.task.claim_expires_at);
        assert_eq!(unchanged.current_run_id, claimed.task.current_run_id);
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, error FROM task_runs WHERE id = ?1",
                    ["r_release_event_conflict"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "running"
        );
        assert!(matches!(
            run.get_value(1).expect("run finished_at"),
            Value::Null
        ));
        assert!(matches!(run.get_value(2).expect("run error"), Value::Null));
        let release_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.released'",
                    [task.id.as_str()],
                )
                .await
                .expect("release event count query"),
        )
        .await
        .expect("release event count row");
        assert_eq!(
            integer_value(
                release_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn release_task_validates_input_without_writes() {
        let (_directory, store, _path) = store("release-input").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_release_input", "release-input", "Release input").await;
        let cases = [
            (
                "task id",
                "default#1".to_owned(),
                release_input(1, "worker", "claim_input", "e_input", 500),
            ),
            (
                "expected_lock_version",
                task.id.clone(),
                release_input(-1, "worker", "claim_input", "e_input_version", 500),
            ),
            (
                "actor",
                task.id.clone(),
                release_input(1, " ", "claim_input", "e_input_actor", 500),
            ),
            (
                "claim_token",
                task.id.clone(),
                release_input(1, "worker", " ", "e_input_token", 500),
            ),
            (
                "event_id",
                task.id.clone(),
                release_input(1, "worker", "claim_input", "input_event", 500),
            ),
            (
                "now",
                task.id.clone(),
                release_input(1, "worker", "claim_input", "e_input_now", -1),
            ),
        ];
        for (field, task_id, input) in cases {
            let error = store
                .release_task(&task_id, input)
                .await
                .expect_err("invalid release input must fail");
            assert!(matches!(error, StoreError::InvalidInput(message) if message.contains(field)));
        }
        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged input task");
        assert_eq!(unchanged.status, "ready");
        assert_eq!(unchanged.lock_version, task.lock_version);
        let connection = store.connection().await.expect("connection");
        let release_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.released'",
                    (),
                )
                .await
                .expect("release event count query"),
        )
        .await
        .expect("release event count row");
        assert_eq!(
            integer_value(
                release_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn release_task_uses_global_task_board_for_run_and_event() {
        let (_directory, store, _path) = store("release-multi-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_release_other', 'release-other', 'Release other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let task = store
            .create_task(
                "release-other",
                create_input(
                    "t_release_other",
                    Some("release-other"),
                    "Release other task",
                ),
            )
            .await
            .expect("create other-board task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "No other-board release plan",
                    "planner",
                    "e_release_other_plan",
                    100,
                ),
            )
            .await
            .expect("mark plan not required");
        store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_release_other_promote", 200),
            )
            .await
            .expect("promote other-board task");
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "other-worker",
                    "claim_release_other",
                    "r_release_other",
                    "e_release_other_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim other-board task");
        let released = store
            .release_task(
                &task.id,
                release_input(
                    claimed.task.lock_version,
                    "other-worker",
                    "claim_release_other",
                    "e_release_other",
                    500,
                ),
            )
            .await
            .expect("release other-board task");
        assert_eq!(released.board_id, "b_release_other");
        assert_eq!(released.board_slug, "release-other");

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_release_other"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_release_other"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_release_other"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"to_status":"ready"}"#
        );
    }

    #[tokio::test]
    async fn submit_review_task_moves_running_task_and_run_atomically() {
        let (_directory, store, _path) = store("review-success").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_review_success",
            "review-success",
            "Review success",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_review_success",
                    "r_review_success",
                    "e_review_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE task_runs SET error = ?1 WHERE id = ?2",
                ("preexisting error", "r_review_success"),
            )
            .await
            .expect("set preexisting run error");

        let reviewed = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_review_success"),
                    false,
                    Some("ready for review"),
                    500,
                    "e_review_success",
                ),
            )
            .await
            .expect("submit review");
        assert_eq!(reviewed.id, task.id);
        assert_eq!(reviewed.status, "review");
        assert_eq!(reviewed.status_reason, None);
        assert_eq!(reviewed.claim_token, None);
        assert_eq!(reviewed.claim_owner, None);
        assert_eq!(reviewed.claim_expires_at, None);
        assert_eq!(reviewed.last_heartbeat_at, None);
        assert_eq!(reviewed.current_run_id.as_deref(), Some("r_review_success"));
        assert_eq!(reviewed.result_summary.as_deref(), Some("ready for review"));
        assert_eq!(reviewed.completed_at, None);
        assert_eq!(reviewed.updated_at, 500);
        assert_eq!(reviewed.lock_version, claimed.task.lock_version + 1);

        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, exit_code, summary, error FROM task_runs WHERE id = ?1",
                    ["r_review_success"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "succeeded"
        );
        assert_eq!(
            integer_value(
                run.get_value(1).expect("run finished_at"),
                "run.finished_at"
            )
            .expect("run finished_at integer"),
            500
        );
        assert_eq!(
            integer_value(run.get_value(2).expect("run exit_code"), "run.exit_code")
                .expect("run exit code integer"),
            0
        );
        assert_eq!(
            optional_text_value(run.get_value(3).expect("run summary"), "run.summary")
                .expect("run summary text")
                .as_deref(),
            Some("ready for review")
        );
        assert_eq!(
            optional_text_value(run.get_value(4).expect("run error"), "run.error")
                .expect("run error text")
                .as_deref(),
            Some("preexisting error")
        );

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_review_success"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_default"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_review_success"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.submitted_for_review"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "worker"
        );
        assert_eq!(
            text_value(event.get_value(5).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"result":null}"#
        );
        assert_eq!(
            integer_value(
                event.get_value(6).expect("event created_at"),
                "event.created_at"
            )
            .expect("event created_at integer"),
            500
        );
    }

    #[tokio::test]
    async fn submit_review_task_rejects_credentials_and_damaged_state_without_writes() {
        let (_directory, store, _path) = store("review-guards").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_review_guards", "review-guards", "Review guards").await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_review_guards",
                    "r_review_guards",
                    "e_review_guards_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let connection = store.connection().await.expect("connection");

        for (token, event_id) in [
            (Some("wrong-review-token"), "e_review_wrong_token"),
            (Some(" claim_review_guards "), "e_review_padded_token"),
            (None, "e_review_missing_token"),
        ] {
            let error = store
                .submit_review_task(
                    &task.id,
                    submit_review_input(
                        claimed.task.lock_version,
                        "worker",
                        token,
                        false,
                        None,
                        500,
                        event_id,
                    ),
                )
                .await
                .expect_err("token mismatch must fail");
            assert!(matches!(error, StoreError::ClaimTokenMismatch));
            assert!(!error.to_string().contains("wrong-review-token"));
        }

        let owner_error = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "other-worker",
                    Some("claim_review_guards"),
                    false,
                    None,
                    500,
                    "e_review_wrong_owner",
                ),
            )
            .await
            .expect_err("owner mismatch must fail");
        assert!(matches!(
            owner_error,
            StoreError::InvalidTransition(message) if message.contains("owner")
        ));

        let stale_error = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version - 1,
                    "worker",
                    Some("claim_review_guards"),
                    false,
                    None,
                    500,
                    "e_review_stale",
                ),
            )
            .await
            .expect_err("stale lock must fail");
        assert!(matches!(stale_error, StoreError::ClaimConflict(_)));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged task");
        assert_eq!(unchanged.status, "running");
        assert_eq!(unchanged.lock_version, claimed.task.lock_version);
        assert_eq!(unchanged.claim_token, claimed.task.claim_token);
        assert_eq!(unchanged.current_run_id, claimed.task.current_run_id);

        connection
            .execute(
                "UPDATE tasks SET status = 'ready' WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("make task non-running");
        let non_running = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_review_guards"),
                    false,
                    None,
                    500,
                    "e_review_non_running",
                ),
            )
            .await
            .expect_err("non-running task must fail");
        assert!(matches!(
            non_running,
            StoreError::InvalidTransition(message) if message.contains("running")
        ));
        connection
            .execute(
                "UPDATE tasks SET status = 'running' WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("restore running task");

        connection
            .execute(
                "UPDATE tasks SET current_run_id = NULL WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("remove current run");
        let missing_run = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_review_guards"),
                    false,
                    None,
                    500,
                    "e_review_missing_run",
                ),
            )
            .await
            .expect_err("missing run must fail");
        assert!(matches!(
            missing_run,
            StoreError::InvalidTransition(message) if message.contains("current running run")
        ));
        connection
            .execute(
                "UPDATE tasks SET current_run_id = ?1 WHERE id = ?2",
                ("r_review_guards", task.id.as_str()),
            )
            .await
            .expect("restore current run");

        connection
            .execute(
                "UPDATE task_runs SET claim_owner = 'tampered' WHERE id = ?1",
                ["r_review_guards"],
            )
            .await
            .expect("tamper run owner");
        let inconsistent_run = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_review_guards"),
                    false,
                    None,
                    500,
                    "e_review_inconsistent_run",
                ),
            )
            .await
            .expect_err("inconsistent run must fail");
        assert!(matches!(
            inconsistent_run,
            StoreError::InvalidTransition(message) if message.contains("inconsistent")
        ));
        connection
            .execute(
                "UPDATE task_runs SET claim_owner = 'worker' WHERE id = ?1",
                ["r_review_guards"],
            )
            .await
            .expect("restore run owner");

        connection
            .execute(
                "UPDATE task_runs SET status = 'succeeded' WHERE id = ?1",
                ["r_review_guards"],
            )
            .await
            .expect("remove active run");
        let no_active_run = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_review_guards"),
                    false,
                    None,
                    500,
                    "e_review_no_active_run",
                ),
            )
            .await
            .expect_err("missing active run must fail");
        assert!(matches!(no_active_run, StoreError::InvalidTransition(_)));
        connection
            .execute(
                "UPDATE task_runs SET status = 'running' WHERE id = ?1",
                ["r_review_guards"],
            )
            .await
            .expect("restore active run");

        let release_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.submitted_for_review'",
                    [task.id.as_str()],
                )
                .await
                .expect("review event count query"),
        )
        .await
        .expect("review event count row");
        assert_eq!(
            integer_value(
                release_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn submit_review_task_force_bypasses_input_credentials_but_keeps_run_consistency() {
        let (_directory, store, _path) = store("review-force").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_review_force", "review-force", "Review force").await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_review_force",
                    "r_review_force",
                    "e_review_force_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE tasks SET result_summary = ?1 WHERE id = ?2",
                ("existing task summary", task.id.as_str()),
            )
            .await
            .expect("set existing task summary");
        connection
            .execute(
                "UPDATE task_runs SET summary = ?1 WHERE id = ?2",
                ("existing run summary", "r_review_force"),
            )
            .await
            .expect("set existing run summary");

        let reviewed = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "force-reviewer",
                    Some("wrong force token"),
                    true,
                    None,
                    500,
                    "e_review_force",
                ),
            )
            .await
            .expect("force submit review");
        assert_eq!(reviewed.status, "review");
        assert_eq!(
            reviewed.result_summary.as_deref(),
            Some("existing task summary")
        );
        assert_eq!(reviewed.current_run_id.as_deref(), Some("r_review_force"));

        let run = first_row(
            connection
                .query(
                    "SELECT summary FROM task_runs WHERE id = ?1",
                    ["r_review_force"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            optional_text_value(run.get_value(0).expect("run summary"), "run.summary")
                .expect("run summary text")
                .as_deref(),
            Some("existing run summary")
        );
        let event = first_row(
            connection
                .query(
                    "SELECT actor, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_review_force"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "force-reviewer"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"result":null}"#
        );
    }

    #[tokio::test]
    async fn submit_review_task_event_conflict_rolls_back_task_and_run_updates() {
        let (_directory, store, _path) = store("review-event-conflict").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_review_event_conflict",
            "review-event-conflict",
            "Review event conflict",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_review_event_conflict",
                    "r_review_event_conflict",
                    "e_review_event_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', ?2, NULL, 'other.event', 'tester', '{}', 1)",
                ("e_review_event_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");

        let error = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_review_event_conflict"),
                    false,
                    Some("must roll back"),
                    500,
                    "e_review_event_conflict",
                ),
            )
            .await
            .expect_err("event conflict must fail");
        assert!(matches!(error, StoreError::Turso(_)));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get rolled back task");
        assert_eq!(unchanged.status, "running");
        assert_eq!(unchanged.lock_version, claimed.task.lock_version);
        assert_eq!(unchanged.claim_token, claimed.task.claim_token);
        assert_eq!(unchanged.current_run_id, claimed.task.current_run_id);
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, exit_code, summary FROM task_runs WHERE id = ?1",
                    ["r_review_event_conflict"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "running"
        );
        assert!(matches!(
            run.get_value(1).expect("run finished_at"),
            Value::Null
        ));
        assert!(matches!(
            run.get_value(2).expect("run exit_code"),
            Value::Null
        ));
        assert!(matches!(
            run.get_value(3).expect("run summary"),
            Value::Null
        ));
        let review_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.submitted_for_review'",
                    [task.id.as_str()],
                )
                .await
                .expect("review event count query"),
        )
        .await
        .expect("review event count row");
        assert_eq!(
            integer_value(
                review_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn submit_review_task_validates_input_without_writes() {
        let (_directory, store, _path) = store("review-input").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_review_input", "review-input", "Review input").await;
        let cases = [
            (
                "task id",
                "default#1".to_owned(),
                submit_review_input(1, "worker", Some("claim"), false, None, 500, "e_input"),
            ),
            (
                "expected_lock_version",
                task.id.clone(),
                submit_review_input(
                    -1,
                    "worker",
                    Some("claim"),
                    false,
                    None,
                    500,
                    "e_input_version",
                ),
            ),
            (
                "actor",
                task.id.clone(),
                submit_review_input(1, " ", Some("claim"), false, None, 500, "e_input_actor"),
            ),
            (
                "event_id",
                task.id.clone(),
                submit_review_input(1, "worker", Some("claim"), false, None, 500, "input_event"),
            ),
            (
                "now",
                task.id.clone(),
                submit_review_input(1, "worker", Some("claim"), false, None, -1, "e_input_now"),
            ),
        ];
        for (field, task_id, input) in cases {
            let error = store
                .submit_review_task(&task_id, input)
                .await
                .expect_err("invalid review input must fail");
            assert!(matches!(error, StoreError::InvalidInput(message) if message.contains(field)));
        }
        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged input task");
        assert_eq!(unchanged.status, "ready");
        assert_eq!(unchanged.lock_version, task.lock_version);
        let connection = store.connection().await.expect("connection");
        let review_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.submitted_for_review'",
                    (),
                )
                .await
                .expect("review event count query"),
        )
        .await
        .expect("review event count row");
        assert_eq!(
            integer_value(
                review_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn submit_review_task_uses_global_task_board_for_run_and_event() {
        let (_directory, store, _path) = store("review-multi-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_review_other', 'review-other', 'Review other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let task = store
            .create_task(
                "review-other",
                create_input("t_review_other", Some("review-other"), "Review other task"),
            )
            .await
            .expect("create other-board task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "No other-board review plan",
                    "planner",
                    "e_review_other_plan",
                    100,
                ),
            )
            .await
            .expect("mark plan not required");
        store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_review_other_promote", 200),
            )
            .await
            .expect("promote other-board task");
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "other-worker",
                    "claim_review_other",
                    "r_review_other",
                    "e_review_other_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim other-board task");
        let reviewed = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "other-worker",
                    Some("claim_review_other"),
                    false,
                    None,
                    500,
                    "e_review_other",
                ),
            )
            .await
            .expect("review other-board task");
        assert_eq!(reviewed.board_id, "b_review_other");
        assert_eq!(reviewed.board_slug, "review-other");
        assert_eq!(reviewed.current_run_id.as_deref(), Some("r_review_other"));

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_review_other"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_review_other"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_review_other"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"result":null}"#
        );
    }

    #[tokio::test]
    async fn mark_execution_plan_not_required_writes_plan_and_event() {
        let (_directory, store, _path) = store("plan-not-required-success").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input(
                    "t_plan_not_required_success",
                    Some("plan-success"),
                    "Plan success",
                ),
            )
            .await
            .expect("create task");

        let plan = store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input("No execution needed", "planner", "e_plan_success", 100),
            )
            .await
            .expect("mark plan not required");
        assert_eq!(
            plan,
            TaskExecutionPlanRecord {
                board_id: "b_default".to_owned(),
                task_id: task.id.clone(),
                state: "not_required".to_owned(),
                reason: Some("No execution needed".to_owned()),
                updated_by: "planner".to_owned(),
                updated_at: 100,
            }
        );

        let connection = store.connection().await.expect("connection");
        let persisted = first_row(
            connection
                .query(
                    "SELECT state, reason, updated_by, updated_at FROM task_execution_plans WHERE board_id = ?1 AND task_id = ?2",
                    ("b_default", task.id.as_str()),
                )
                .await
                .expect("plan query"),
        )
        .await
        .expect("plan row");
        assert_eq!(
            text_value(persisted.get_value(0).expect("plan state"), "plan.state")
                .expect("plan state text"),
            "not_required"
        );
        assert_eq!(
            text_value(persisted.get_value(1).expect("plan reason"), "plan.reason")
                .expect("plan reason text"),
            "No execution needed"
        );
        assert_eq!(
            text_value(
                persisted.get_value(2).expect("plan actor"),
                "plan.updated_by"
            )
            .expect("plan actor text"),
            "planner"
        );
        assert_eq!(
            integer_value(
                persisted.get_value(3).expect("plan updated_at"),
                "plan.updated_at"
            )
            .expect("plan updated_at integer"),
            100
        );
        let event = first_row(
            connection
                .query(
                    "SELECT event_id, board_id, task_id, kind, actor, payload_json, created_at FROM task_events WHERE kind = 'task.execution_plan.not_required'",
                    (),
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event id"), "event.event_id")
                .expect("event id text"),
            "e_plan_success"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_default"
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.execution_plan.not_required"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "planner"
        );
        assert_eq!(
            text_value(event.get_value(5).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"state":"not_required"}"#
        );
        assert_eq!(
            integer_value(
                event.get_value(6).expect("event created_at"),
                "event.created_at"
            )
            .expect("event created_at integer"),
            100
        );
    }

    #[tokio::test]
    async fn mark_execution_plan_not_required_retries_without_extra_event_and_updates_reason() {
        let (_directory, store, _path) = store("plan-not-required-retry").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input(
                    "t_plan_not_required_retry",
                    Some("plan-retry"),
                    "Plan retry",
                ),
            )
            .await
            .expect("create task");

        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input("First reason", "planner", "e_plan_retry_first", 100),
            )
            .await
            .expect("first mark");
        let retry = store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input("Updated reason", "reviewer", "e_plan_retry_second", 200),
            )
            .await
            .expect("retry mark");
        assert_eq!(retry.state, "not_required");
        assert_eq!(retry.reason.as_deref(), Some("Updated reason"));
        assert_eq!(retry.updated_by, "reviewer");
        assert_eq!(retry.updated_at, 200);

        let connection = store.connection().await.expect("connection");
        let event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.execution_plan.not_required'",
                    [task.id.as_str()],
                )
                .await
                .expect("event count query"),
        )
        .await
        .expect("event count row");
        assert_eq!(
            integer_value(
                event_count.get_value(0).expect("event count"),
                "event.count"
            )
            .expect("event count integer"),
            1
        );
        let event = first_row(
            connection
                .query(
                    "SELECT event_id, actor, created_at FROM task_events WHERE task_id = ?1 AND kind = 'task.execution_plan.not_required'",
                    [task.id.as_str()],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event id"), "event.event_id")
                .expect("event id text"),
            "e_plan_retry_first"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "planner"
        );
        assert_eq!(
            integer_value(
                event.get_value(2).expect("event created_at"),
                "event.created_at"
            )
            .expect("event created_at integer"),
            100
        );
    }

    #[tokio::test]
    async fn mark_execution_plan_not_required_rejects_steps_archived_and_unknown_without_partial_write()
     {
        let (_directory, store, _path) = store("plan-not-required-reject").await;
        store.initialize().await.expect("initialize");

        let unknown = store
            .mark_execution_plan_not_required(
                "t_plan_not_required_unknown",
                plan_input("Unknown", "planner", "e_plan_unknown", 100),
            )
            .await
            .expect_err("unknown task must fail");
        assert!(matches!(
            unknown,
            StoreError::TaskNotFound(task_id) if task_id == "t_plan_not_required_unknown"
        ));

        let with_step = store
            .create_task(
                "default",
                create_input("t_plan_not_required_step", Some("plan-step"), "Plan step"),
            )
            .await
            .expect("create task with step");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_steps(id, board_id, parent_task_id, position, title, required, status, created_by, created_at, updated_by, updated_at) VALUES (?1, 'b_default', ?2, 1, 'Existing step', 1, 'todo', 'tester', 1, 'tester', 1)",
                ("step_plan_not_required", with_step.id.as_str()),
            )
            .await
            .expect("insert existing step");
        let step_error = store
            .mark_execution_plan_not_required(
                &with_step.id,
                plan_input("Has steps", "planner", "e_plan_step", 200),
            )
            .await
            .expect_err("task with steps must fail");
        assert!(matches!(
            step_error,
            StoreError::InvalidInput(message) if message.contains("steps")
        ));

        let archived = store
            .create_task(
                "default",
                create_input(
                    "t_plan_not_required_archived",
                    Some("plan-archived"),
                    "Plan archived",
                ),
            )
            .await
            .expect("create archived task");
        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 300 WHERE id = ?1",
                [archived.id.as_str()],
            )
            .await
            .expect("archive task");
        let archived_error = store
            .mark_execution_plan_not_required(
                &archived.id,
                plan_input("Archived", "planner", "e_plan_archived", 300),
            )
            .await
            .expect_err("archived task must fail");
        assert!(matches!(
            archived_error,
            StoreError::InvalidInput(message) if message.contains("archived")
        ));

        let mut rows = connection
            .query(
                "SELECT task_id, state FROM task_execution_plans WHERE task_id IN (?1, ?2) ORDER BY task_id",
                (with_step.id.as_str(), archived.id.as_str()),
            )
            .await
            .expect("plan query");
        let mut states = Vec::new();
        while let Some(row) = rows.next().await.expect("plan row") {
            states.push((
                text_value(row.get_value(0).expect("plan task"), "plan.task_id")
                    .expect("plan task text"),
                text_value(row.get_value(1).expect("plan state"), "plan.state")
                    .expect("plan state text"),
            ));
        }
        assert_eq!(
            states,
            vec![
                (archived.id.clone(), "unplanned".to_owned()),
                (with_step.id.clone(), "unplanned".to_owned()),
            ]
        );
        let event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE kind = 'task.execution_plan.not_required'",
                    (),
                )
                .await
                .expect("event count query"),
        )
        .await
        .expect("event count row");
        assert_eq!(
            integer_value(
                event_count.get_value(0).expect("event count"),
                "event.count"
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn mark_execution_plan_not_required_rejects_archived_board_without_partial_write() {
        let (_directory, store, _path) = store("plan-not-required-archived-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at, archived_at) VALUES ('b_archived', 'archived-board', 'Archived board', 1, 1, 350)",
                (),
            )
            .await
            .expect("insert archived board");
        let task = store
            .create_task(
                "archived-board",
                create_input(
                    "t_plan_not_required_archived_board",
                    Some("plan-archived-board"),
                    "Archived board task",
                ),
            )
            .await
            .expect("create task on archived board");

        let error = store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input("Archived board", "planner", "e_plan_archived_board", 400),
            )
            .await
            .expect_err("archived board must fail");
        assert!(matches!(
            error,
            StoreError::InvalidInput(message) if message.contains("archived")
        ));

        let plan = first_row(
            connection
                .query(
                    "SELECT state, reason, updated_by, updated_at FROM task_execution_plans WHERE board_id = ?1 AND task_id = ?2",
                    ("b_archived", task.id.as_str()),
                )
                .await
                .expect("plan query"),
        )
        .await
        .expect("plan row");
        assert_eq!(
            text_value(plan.get_value(0).expect("plan state"), "plan.state")
                .expect("plan state text"),
            "unplanned"
        );
        assert!(matches!(
            plan.get_value(1).expect("plan reason"),
            Value::Null
        ));
        assert_eq!(
            text_value(plan.get_value(2).expect("plan actor"), "plan.updated_by")
                .expect("plan actor text"),
            "tester"
        );
        let generated_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE board_id = ?1 AND task_id = ?2 AND kind = 'task.execution_plan.not_required'",
                    ("b_archived", task.id.as_str()),
                )
                .await
                .expect("generated event count query"),
        )
        .await
        .expect("generated event count row");
        assert_eq!(
            integer_value(
                generated_event_count
                    .get_value(0)
                    .expect("generated event count"),
                "event.count",
            )
            .expect("generated event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn mark_execution_plan_not_required_event_conflict_rolls_back_plan_update() {
        let (_directory, store, _path) = store("plan-not-required-conflict").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input(
                    "t_plan_not_required_conflict",
                    Some("plan-conflict"),
                    "Plan conflict",
                ),
            )
            .await
            .expect("create task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', ?2, NULL, 'other.event', 'tester', '{}', 1)",
                ("e_plan_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");
        let baseline_plan = first_row(
            connection
                .query(
                    "SELECT updated_at FROM task_execution_plans WHERE task_id = ?1",
                    [task.id.as_str()],
                )
                .await
                .expect("baseline plan query"),
        )
        .await
        .expect("baseline plan row");
        let baseline_updated_at = integer_value(
            baseline_plan
                .get_value(0)
                .expect("baseline plan updated_at"),
            "plan.updated_at",
        )
        .expect("baseline plan updated_at integer");

        let error = store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input("Should roll back", "planner", "e_plan_conflict", 400),
            )
            .await
            .expect_err("event conflict must fail");
        assert!(matches!(error, StoreError::Turso(_)));

        let plan = first_row(
            connection
                .query(
                    "SELECT state, reason, updated_by, updated_at FROM task_execution_plans WHERE task_id = ?1",
                    [task.id.as_str()],
                )
                .await
                .expect("plan query"),
        )
        .await
        .expect("plan row");
        assert_eq!(
            text_value(plan.get_value(0).expect("plan state"), "plan.state")
                .expect("plan state text"),
            "unplanned"
        );
        assert!(matches!(
            plan.get_value(1).expect("plan reason"),
            Value::Null
        ));
        assert_eq!(
            text_value(plan.get_value(2).expect("plan actor"), "plan.updated_by")
                .expect("plan actor text"),
            "tester"
        );
        assert_eq!(
            integer_value(
                plan.get_value(3).expect("plan updated_at"),
                "plan.updated_at"
            )
            .expect("plan updated_at integer"),
            baseline_updated_at
        );
        let generated_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.execution_plan.not_required'",
                    [task.id.as_str()],
                )
                .await
                .expect("generated event count query"),
        )
        .await
        .expect("generated event count row");
        assert_eq!(
            integer_value(
                generated_event_count
                    .get_value(0)
                    .expect("generated event count"),
                "event.count",
            )
            .expect("generated event count integer"),
            0
        );
        assert_eq!(count_rows(&connection, "task_events").await, 2);
    }

    #[tokio::test]
    async fn mark_execution_plan_not_required_uses_task_board() {
        let (_directory, store, _path) = store("plan-not-required-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_other', 'other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("insert second board");
        let task = store
            .create_task(
                "other",
                create_input(
                    "t_plan_not_required_other",
                    Some("plan-other"),
                    "Other plan",
                ),
            )
            .await
            .expect("create other-board task");

        let plan = store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "Other board does not need execution",
                    "planner",
                    "e_plan_other",
                    500,
                ),
            )
            .await
            .expect("mark other-board plan");
        assert_eq!(plan.board_id, "b_other");
        assert_eq!(plan.task_id, task.id);

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id FROM task_events WHERE event_id = ?1",
                    ["e_plan_other"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_other"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
    }

    #[tokio::test]
    async fn complete_task_writes_done_run_and_result_event() {
        let (_directory, store, _path) = store("complete-success").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_complete_success",
            "complete-success",
            "Complete success",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_complete_success",
                    "r_complete_success",
                    "e_complete_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let setup_connection = store.connection().await.expect("connection");
        setup_connection
            .execute(
                "UPDATE task_runs SET error = ?1 WHERE id = ?2",
                ("preexisting error", "r_complete_success"),
            )
            .await
            .expect("set preexisting run error");

        let completed = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_success"),
                    false,
                    Some("finished"),
                    Some(r#"{"ok":true}"#),
                    500,
                    "e_complete_success",
                ),
            )
            .await
            .expect("complete task");
        assert_eq!(completed.status, "done");
        assert_eq!(completed.status_reason, None);
        assert_eq!(completed.completed_at, Some(500));
        assert_eq!(completed.claim_token, None);
        assert_eq!(completed.claim_owner, None);
        assert_eq!(completed.claim_expires_at, None);
        assert_eq!(completed.last_heartbeat_at, None);
        assert_eq!(
            completed.current_run_id.as_deref(),
            Some("r_complete_success")
        );
        assert_eq!(completed.result_summary.as_deref(), Some("finished"));
        assert_eq!(completed.result_json.as_deref(), Some(r#"{"ok":true}"#));
        assert_eq!(completed.lock_version, claimed.task.lock_version + 1);

        let connection = store.connection().await.expect("connection");
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, exit_code, summary, error FROM task_runs WHERE id = ?1",
                    ["r_complete_success"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "succeeded"
        );
        assert_eq!(
            integer_value(run.get_value(1).expect("run finished"), "run.finished_at")
                .expect("run finished integer"),
            500
        );
        assert_eq!(
            integer_value(run.get_value(2).expect("run exit"), "run.exit_code")
                .expect("run exit integer"),
            0
        );
        assert_eq!(
            text_value(run.get_value(3).expect("run summary"), "run.summary")
                .expect("run summary text"),
            "finished"
        );
        assert!(matches!(run.get_value(4).expect("run error"), Value::Null));

        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_complete_success"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_default"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_complete_success"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.completed"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "worker"
        );
        assert_eq!(
            text_value(event.get_value(5).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"result":{"ok":true}}"#
        );
        assert_eq!(
            integer_value(
                event.get_value(6).expect("event created"),
                "event.created_at"
            )
            .expect("event created integer"),
            500
        );
    }

    #[tokio::test]
    async fn complete_task_review_does_not_require_token_or_finish_succeeded_run() {
        let (_directory, store, _path) = store("complete-review").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_complete_review",
            "complete-review",
            "Complete review",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_complete_review",
                    "r_complete_review",
                    "e_complete_review_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let reviewed = store
            .submit_review_task(
                &task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_review"),
                    false,
                    Some("reviewed"),
                    400,
                    "e_complete_review_submit",
                ),
            )
            .await
            .expect("submit review");
        assert_eq!(reviewed.status, "review");
        assert_eq!(reviewed.result_summary.as_deref(), Some("reviewed"));

        let completed = store
            .complete_task(
                &task.id,
                complete_input(
                    reviewed.lock_version,
                    "reviewer",
                    None,
                    false,
                    None,
                    None,
                    500,
                    "e_complete_review_done",
                ),
            )
            .await
            .expect("complete reviewed task");
        assert_eq!(completed.status, "done");
        assert_eq!(completed.result_summary.as_deref(), Some("reviewed"));
        assert_eq!(
            completed.current_run_id.as_deref(),
            Some("r_complete_review")
        );
        assert_eq!(completed.completed_at, Some(500));

        let connection = store.connection().await.expect("connection");
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, exit_code FROM task_runs WHERE id = ?1",
                    ["r_complete_review"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "succeeded"
        );
        assert_eq!(
            integer_value(run.get_value(1).expect("run finished"), "run.finished_at")
                .expect("run finished integer"),
            400
        );
        assert_eq!(
            integer_value(run.get_value(2).expect("run exit"), "run.exit_code")
                .expect("run exit integer"),
            0
        );
        let event = first_row(
            connection
                .query(
                    "SELECT run_id, actor, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_complete_review_done"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_complete_review"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "reviewer"
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"result":null}"#
        );
    }

    #[tokio::test]
    async fn complete_task_rejects_credentials_steps_and_damaged_state_without_writes() {
        let (_directory, store, _path) = store("complete-guards").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_complete_guards",
            "complete-guards",
            "Complete guards",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_complete_guards",
                    "r_complete_guards",
                    "e_complete_guards_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let connection = store.connection().await.expect("connection");

        for (token, event_id) in [
            (Some("wrong-complete-token"), "e_complete_wrong_token"),
            (Some(" claim_complete_guards "), "e_complete_padded_token"),
            (None, "e_complete_missing_token"),
        ] {
            let error = store
                .complete_task(
                    &task.id,
                    complete_input(
                        claimed.task.lock_version,
                        "worker",
                        token,
                        false,
                        None,
                        None,
                        500,
                        event_id,
                    ),
                )
                .await
                .expect_err("token mismatch must fail");
            assert!(matches!(error, StoreError::ClaimTokenMismatch));
            assert!(!error.to_string().contains("wrong-complete-token"));
        }

        let owner_error = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "other-worker",
                    Some("claim_complete_guards"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_wrong_owner",
                ),
            )
            .await
            .expect_err("owner mismatch must fail");
        assert!(matches!(
            owner_error,
            StoreError::InvalidTransition(message) if message.contains("owner")
        ));

        let stale_error = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version - 1,
                    "worker",
                    Some("claim_complete_guards"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_stale",
                ),
            )
            .await
            .expect_err("stale lock must fail");
        assert!(matches!(stale_error, StoreError::ClaimConflict(_)));

        connection
            .execute(
                "UPDATE tasks SET status = 'ready' WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("make task non-running");
        let non_running = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_guards"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_non_running",
                ),
            )
            .await
            .expect_err("non-running task must fail");
        assert!(matches!(
            non_running,
            StoreError::InvalidTransition(message) if message.contains("running or review")
        ));
        connection
            .execute(
                "UPDATE tasks SET status = 'running' WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("restore running task");

        connection
            .execute(
                "UPDATE tasks SET current_run_id = NULL WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("remove current run");
        let missing_run = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_guards"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_missing_run",
                ),
            )
            .await
            .expect_err("missing run must fail");
        assert!(matches!(
            missing_run,
            StoreError::InvalidTransition(message) if message.contains("current running run")
        ));
        connection
            .execute(
                "UPDATE tasks SET current_run_id = ?1 WHERE id = ?2",
                ("r_complete_guards", task.id.as_str()),
            )
            .await
            .expect("restore current run");

        connection
            .execute(
                "UPDATE task_runs SET claim_owner = 'tampered' WHERE id = ?1",
                ["r_complete_guards"],
            )
            .await
            .expect("tamper run owner");
        let inconsistent_run = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_guards"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_inconsistent_run",
                ),
            )
            .await
            .expect_err("inconsistent run must fail");
        assert!(matches!(
            inconsistent_run,
            StoreError::InvalidTransition(message) if message.contains("inconsistent")
        ));
        connection
            .execute(
                "UPDATE task_runs SET claim_owner = 'worker' WHERE id = ?1",
                ["r_complete_guards"],
            )
            .await
            .expect("restore run owner");

        connection
            .execute(
                "UPDATE task_runs SET status = 'succeeded' WHERE id = ?1",
                ["r_complete_guards"],
            )
            .await
            .expect("remove active run");
        let no_active_run = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_guards"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_no_active_run",
                ),
            )
            .await
            .expect_err("missing active run must fail");
        assert!(matches!(no_active_run, StoreError::InvalidTransition(_)));
        connection
            .execute(
                "UPDATE task_runs SET status = 'running' WHERE id = ?1",
                ["r_complete_guards"],
            )
            .await
            .expect("restore active run");

        connection
            .execute("PRAGMA ignore_check_constraints = ON", ())
            .await
            .expect("disable checks for damaged state");
        connection
            .execute(
                "UPDATE tasks SET claim_expires_at = NULL WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("remove task claim expiry");
        connection
            .execute("PRAGMA ignore_check_constraints = OFF", ())
            .await
            .expect("restore checks after damaged state");
        let missing_claim_expiry = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "admin",
                    None,
                    true,
                    None,
                    None,
                    500,
                    "e_complete_missing_claim_expiry",
                ),
            )
            .await
            .expect_err("missing claim expiry must fail");
        assert!(matches!(
            missing_claim_expiry,
            StoreError::InvalidTransition(message) if message.contains("active claim")
        ));
        connection
            .execute("PRAGMA ignore_check_constraints = ON", ())
            .await
            .expect("disable checks for restore");
        connection
            .execute(
                "UPDATE tasks SET claim_expires_at = ?1 WHERE id = ?2",
                (1_300_i64, task.id.as_str()),
            )
            .await
            .expect("restore task claim expiry");
        connection
            .execute("PRAGMA ignore_check_constraints = OFF", ())
            .await
            .expect("restore checks");

        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 600 WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("archive task");
        let archived = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_guards"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_archived",
                ),
            )
            .await
            .expect_err("archived task must fail");
        assert!(matches!(
            archived,
            StoreError::InvalidTransition(message) if message.contains("archived")
        ));
        connection
            .execute(
                "UPDATE tasks SET status = 'running', archived_at = NULL WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("restore task archive state");

        connection
            .execute(
                "INSERT INTO task_steps(id, board_id, parent_task_id, position, title, required, status, created_by, created_at, updated_by, updated_at) VALUES (?1, 'b_default', ?2, 1, 'Required', 1, 'todo', 'tester', 1, 'tester', 1)",
                ("step_complete_required", task.id.as_str()),
            )
            .await
            .expect("insert incomplete required step");
        let incomplete = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "unrelated",
                    Some("wrong-token"),
                    true,
                    None,
                    None,
                    500,
                    "e_complete_steps_incomplete",
                ),
            )
            .await
            .expect_err("incomplete required step must fail even force");
        assert!(matches!(incomplete, StoreError::StepsIncomplete(_)));
        connection
            .execute(
                "DELETE FROM task_steps WHERE id = ?1",
                ["step_complete_required"],
            )
            .await
            .expect("remove required step");

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged task");
        assert_eq!(unchanged.status, "running");
        assert_eq!(unchanged.lock_version, claimed.task.lock_version);
        assert_eq!(unchanged.claim_token, claimed.task.claim_token);
        assert_eq!(unchanged.current_run_id, claimed.task.current_run_id);
        let completed_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.completed'",
                    [task.id.as_str()],
                )
                .await
                .expect("completed event count query"),
        )
        .await
        .expect("completed event count row");
        assert_eq!(
            integer_value(
                completed_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn complete_task_force_bypasses_caller_credentials_and_preserves_results() {
        let (_directory, store, _path) = store("complete-force").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_complete_force",
            "complete-force",
            "Complete force",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_complete_force",
                    "r_complete_force",
                    "e_complete_force_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE tasks SET result_summary = ?1, result_json = ?2 WHERE id = ?3",
                ("previous result", r#"{"previous":true}"#, task.id.as_str()),
            )
            .await
            .expect("set previous task result");
        connection
            .execute(
                "UPDATE task_runs SET summary = ?1 WHERE id = ?2",
                ("previous run summary", "r_complete_force"),
            )
            .await
            .expect("set previous run summary");

        let completed = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "admin",
                    Some("wrong-token"),
                    true,
                    None,
                    None,
                    500,
                    "e_complete_force_done",
                ),
            )
            .await
            .expect("force complete task");
        assert_eq!(completed.status, "done");
        assert_eq!(completed.result_summary.as_deref(), Some("previous result"));
        assert_eq!(
            completed.result_json.as_deref(),
            Some(r#"{"previous":true}"#)
        );

        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, exit_code, summary FROM task_runs WHERE id = ?1",
                    ["r_complete_force"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "succeeded"
        );
        assert_eq!(
            integer_value(run.get_value(1).expect("run finished"), "run.finished_at")
                .expect("run finished integer"),
            500
        );
        assert_eq!(
            integer_value(run.get_value(2).expect("run exit"), "run.exit_code")
                .expect("run exit integer"),
            0
        );
        assert_eq!(
            text_value(run.get_value(3).expect("run summary"), "run.summary")
                .expect("run summary text"),
            "previous run summary"
        );
        let event = first_row(
            connection
                .query(
                    "SELECT actor, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_complete_force_done"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "admin"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"result":null}"#
        );
    }

    #[tokio::test]
    async fn complete_task_event_conflict_rolls_back_task_and_run() {
        let (_directory, store, _path) = store("complete-event-conflict").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_complete_event_conflict",
            "complete-event-conflict",
            "Complete event conflict",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_complete_event_conflict",
                    "r_complete_event_conflict",
                    "e_complete_event_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', ?2, NULL, 'other.event', 'tester', '{}', 1)",
                ("e_complete_event_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");
        let error = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_event_conflict"),
                    false,
                    Some("should rollback"),
                    Some(r#"{"ok":true}"#),
                    500,
                    "e_complete_event_conflict",
                ),
            )
            .await
            .expect_err("event conflict must fail");
        assert!(matches!(error, StoreError::Turso(_)));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get rolled back task");
        assert_eq!(unchanged.status, "running");
        assert_eq!(unchanged.lock_version, claimed.task.lock_version);
        assert_eq!(unchanged.claim_token, claimed.task.claim_token);
        assert_eq!(unchanged.current_run_id, claimed.task.current_run_id);
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, exit_code, summary FROM task_runs WHERE id = ?1",
                    ["r_complete_event_conflict"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "running"
        );
        assert!(matches!(
            run.get_value(1).expect("run finished"),
            Value::Null
        ));
        assert!(matches!(run.get_value(2).expect("run exit"), Value::Null));
        assert!(matches!(
            run.get_value(3).expect("run summary"),
            Value::Null
        ));
        let completed_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.completed'",
                    [task.id.as_str()],
                )
                .await
                .expect("completed event count query"),
        )
        .await
        .expect("completed event count row");
        assert_eq!(
            integer_value(
                completed_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn complete_task_validates_input_and_result_json_without_writes() {
        let (_directory, store, _path) = store("complete-input").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_complete_input",
            "complete-input",
            "Complete input",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_complete_input",
                    "r_complete_input",
                    "e_complete_input_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let cases = [
            (
                "task id",
                "default#1".to_owned(),
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_input"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_input_task",
                ),
            ),
            (
                "expected_lock_version",
                task.id.clone(),
                complete_input(
                    -1,
                    "worker",
                    Some("claim_complete_input"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_input_version",
                ),
            ),
            (
                "actor",
                task.id.clone(),
                complete_input(
                    claimed.task.lock_version,
                    " ",
                    Some("claim_complete_input"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_input_actor",
                ),
            ),
            (
                "event_id",
                task.id.clone(),
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_input"),
                    false,
                    None,
                    None,
                    500,
                    "invalid_event",
                ),
            ),
            (
                "now",
                task.id.clone(),
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_input"),
                    false,
                    None,
                    None,
                    -1,
                    "e_complete_input_now",
                ),
            ),
        ];
        for (field, task_id, input) in cases {
            let error = store
                .complete_task(&task_id, input)
                .await
                .expect_err("invalid complete input must fail");
            assert!(matches!(error, StoreError::InvalidInput(message) if message.contains(field)));
        }

        let invalid_json = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_input"),
                    false,
                    None,
                    Some("{not-json"),
                    500,
                    "e_complete_invalid_json",
                ),
            )
            .await
            .expect_err("invalid result json must fail");
        assert!(matches!(
            invalid_json,
            StoreError::InvalidInput(message) if message.contains("result_json")
        ));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged input task");
        assert_eq!(unchanged.status, "running");
        assert_eq!(unchanged.lock_version, claimed.task.lock_version);
        assert_eq!(unchanged.claim_token, claimed.task.claim_token);
        assert_eq!(unchanged.current_run_id, claimed.task.current_run_id);
        let connection = store.connection().await.expect("connection");
        let completed_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.completed'",
                    [task.id.as_str()],
                )
                .await
                .expect("completed event count query"),
        )
        .await
        .expect("completed event count row");
        assert_eq!(
            integer_value(
                completed_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn complete_task_uses_global_task_board_for_run_and_event() {
        let (_directory, store, _path) = store("complete-multi-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_complete_other', 'complete-other', 'Complete other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let task = store
            .create_task(
                "complete-other",
                create_input(
                    "t_complete_other",
                    Some("complete-other"),
                    "Complete other task",
                ),
            )
            .await
            .expect("create other-board task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input(
                    "No other-board completion plan",
                    "planner",
                    "e_complete_other_plan",
                    100,
                ),
            )
            .await
            .expect("mark plan not required");
        store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_complete_other_promote", 200),
            )
            .await
            .expect("promote other-board task");
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_complete_other",
                    "r_complete_other",
                    "e_complete_other_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim other-board task");
        let completed = store
            .complete_task(
                &task.id,
                complete_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_complete_other"),
                    false,
                    None,
                    None,
                    500,
                    "e_complete_other_done",
                ),
            )
            .await
            .expect("complete other-board task");
        assert_eq!(completed.board_id, "b_complete_other");
        assert_eq!(completed.board_slug, "complete-other");
        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, kind, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_complete_other_done"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_complete_other"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_complete_other"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.completed"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"result":null}"#
        );
    }

    #[tokio::test]
    async fn block_task_ready_writes_blocked_task_and_reason_event() {
        let (_directory, store, _path) = store("block-ready").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_block_ready", "block-ready", "Block ready").await;
        let reason = "waiting on API";
        let blocked = store
            .block_task(
                &task.id,
                block_input(1, " blocker ", None, false, reason, 500, "e_block_ready"),
            )
            .await
            .expect("block ready task");
        assert_eq!(blocked.status, "blocked");
        assert_eq!(blocked.status_reason.as_deref(), Some(reason));
        assert_eq!(blocked.lock_version, 2);
        assert_eq!(blocked.claim_token, None);
        assert_eq!(blocked.claim_owner, None);
        assert_eq!(blocked.claim_expires_at, None);
        assert_eq!(blocked.last_heartbeat_at, None);
        assert_eq!(blocked.current_run_id, None);

        let connection = store.connection().await.expect("connection");
        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_block_ready"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_default"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert!(matches!(
            event.get_value(2).expect("event run"),
            Value::Null
        ));
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.blocked"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "blocker"
        );
        assert_eq!(
            text_value(event.get_value(5).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"reason":"waiting on API"}"#
        );
        assert_eq!(
            integer_value(
                event.get_value(6).expect("event created"),
                "event.created_at"
            )
            .expect("event created integer"),
            500
        );
    }

    #[tokio::test]
    async fn block_task_running_fails_run_and_clears_claim_atomically() {
        let (_directory, store, _path) = store("block-running").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_block_running", "block-running", "Block running").await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_block_running",
                    "r_block_running",
                    "e_block_running_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let reason = "worker failed: waiting for API";
        let blocked = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_block_running"),
                    false,
                    reason,
                    500,
                    "e_block_running",
                ),
            )
            .await
            .expect("block running task");
        assert_eq!(blocked.status, "blocked");
        assert_eq!(blocked.status_reason.as_deref(), Some(reason));
        assert_eq!(blocked.completed_at, None);
        assert_eq!(blocked.current_run_id.as_deref(), Some("r_block_running"));
        assert_eq!(blocked.claim_token, None);
        assert_eq!(blocked.claim_owner, None);
        assert_eq!(blocked.claim_expires_at, None);
        assert_eq!(blocked.last_heartbeat_at, None);
        assert_eq!(blocked.lock_version, claimed.task.lock_version + 1);

        let connection = store.connection().await.expect("connection");
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, exit_code, error FROM task_runs WHERE id = ?1",
                    ["r_block_running"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "failed"
        );
        assert_eq!(
            integer_value(run.get_value(1).expect("run finished"), "run.finished_at")
                .expect("run finished integer"),
            500
        );
        assert_eq!(
            integer_value(run.get_value(2).expect("run exit"), "run.exit_code")
                .expect("run exit integer"),
            1
        );
        assert_eq!(
            text_value(run.get_value(3).expect("run error"), "run.error").expect("run error text"),
            reason
        );
        let event = first_row(
            connection
                .query(
                    "SELECT run_id, actor, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_block_running"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_block_running"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "worker"
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"reason":"worker failed: waiting for API"}"#
        );
    }

    #[tokio::test]
    async fn block_task_accepts_all_public_non_running_source_states_and_review() {
        let (_directory, store, _path) = store("block-sources").await;
        store.initialize().await.expect("initialize");
        for (index, status, event_id) in [
            (1_i64, "triage", "e_block_triage"),
            (2_i64, "todo", "e_block_todo"),
            (3_i64, "scheduled", "e_block_scheduled"),
        ] {
            let task_id = format!("t_block_source_{index}");
            let mut task_input = create_input(
                &task_id,
                Some(&format!("block-source-{index}")),
                "Block source",
            );
            task_input.status = status.to_owned();
            let task = store
                .create_task("default", task_input)
                .await
                .expect("create source task");
            let blocked = store
                .block_task(
                    &task.id,
                    block_input(0, "worker", None, false, "waiting", 500, event_id),
                )
                .await
                .expect("block source task");
            assert_eq!(blocked.status, "blocked");
            assert_eq!(blocked.status_reason.as_deref(), Some("waiting"));
            assert_eq!(blocked.lock_version, 1);
        }

        let ready_task = ready_task_for_claim(
            &store,
            "t_block_ready_source",
            "block-ready-source",
            "Block ready source",
        )
        .await;
        let blocked_ready = store
            .block_task(
                &ready_task.id,
                block_input(
                    1,
                    "worker",
                    None,
                    false,
                    "waiting",
                    500,
                    "e_block_ready_source",
                ),
            )
            .await
            .expect("block ready source");
        assert_eq!(blocked_ready.status, "blocked");
        assert_eq!(blocked_ready.lock_version, 2);

        let review_task =
            ready_task_for_claim(&store, "t_block_review", "block-review", "Block review").await;
        let claimed = store
            .claim_task(
                &review_task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_block_review",
                    "r_block_review",
                    "e_block_review_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim review source");
        let reviewed = store
            .submit_review_task(
                &review_task.id,
                submit_review_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_block_review"),
                    false,
                    None,
                    400,
                    "e_block_review_submit",
                ),
            )
            .await
            .expect("submit review source");
        let blocked = store
            .block_task(
                &review_task.id,
                block_input(
                    reviewed.lock_version,
                    "reviewer",
                    None,
                    false,
                    "review rejected",
                    500,
                    "e_block_review",
                ),
            )
            .await
            .expect("block review source");
        assert_eq!(blocked.status, "blocked");
        assert_eq!(blocked.status_reason.as_deref(), Some("review rejected"));
        assert_eq!(blocked.current_run_id.as_deref(), Some("r_block_review"));
        assert_eq!(blocked.claim_token, None);
        assert_eq!(blocked.claim_owner, None);
    }

    #[tokio::test]
    async fn block_task_rejects_credentials_and_damaged_state_without_writes() {
        let (_directory, store, _path) = store("block-guards").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_block_guards", "block-guards", "Block guards").await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_block_guards",
                    "r_block_guards",
                    "e_block_guards_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let connection = store.connection().await.expect("connection");

        for (token, event_id) in [
            (Some("wrong-block-token"), "e_block_wrong_token"),
            (Some(" claim_block_guards "), "e_block_padded_token"),
            (None, "e_block_missing_token"),
        ] {
            let error = store
                .block_task(
                    &task.id,
                    block_input(
                        claimed.task.lock_version,
                        "worker",
                        token,
                        false,
                        "waiting",
                        500,
                        event_id,
                    ),
                )
                .await
                .expect_err("token mismatch must fail");
            assert!(matches!(error, StoreError::ClaimTokenMismatch));
            assert!(!error.to_string().contains("wrong-block-token"));
        }

        let owner_error = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "other-worker",
                    Some("claim_block_guards"),
                    false,
                    "waiting",
                    500,
                    "e_block_wrong_owner",
                ),
            )
            .await
            .expect_err("owner mismatch must fail");
        assert!(matches!(
            owner_error,
            StoreError::InvalidTransition(message) if message.contains("owner")
        ));

        let stale_error = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version - 1,
                    "worker",
                    Some("claim_block_guards"),
                    false,
                    "waiting",
                    500,
                    "e_block_stale",
                ),
            )
            .await
            .expect_err("stale lock must fail");
        assert!(matches!(stale_error, StoreError::ClaimConflict(_)));

        connection
            .execute(
                "UPDATE tasks SET status = 'done' WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("make task non-blockable");
        let non_source = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_block_guards"),
                    false,
                    "waiting",
                    500,
                    "e_block_non_source",
                ),
            )
            .await
            .expect_err("done task must fail");
        assert!(matches!(
            non_source,
            StoreError::InvalidTransition(message) if message.contains("cannot block")
        ));
        connection
            .execute(
                "UPDATE tasks SET status = 'running' WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("restore running status");

        connection
            .execute(
                "UPDATE tasks SET current_run_id = NULL WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("remove current run");
        let missing_run = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_block_guards"),
                    false,
                    "waiting",
                    500,
                    "e_block_missing_run",
                ),
            )
            .await
            .expect_err("missing run must fail");
        assert!(matches!(
            missing_run,
            StoreError::InvalidTransition(message) if message.contains("current running run")
        ));
        connection
            .execute(
                "UPDATE tasks SET current_run_id = ?1 WHERE id = ?2",
                ("r_block_guards", task.id.as_str()),
            )
            .await
            .expect("restore current run");

        connection
            .execute(
                "UPDATE task_runs SET claim_owner = 'tampered' WHERE id = ?1",
                ["r_block_guards"],
            )
            .await
            .expect("tamper run claim owner");
        let inconsistent_run = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_block_guards"),
                    false,
                    "waiting",
                    500,
                    "e_block_inconsistent_run",
                ),
            )
            .await
            .expect_err("inconsistent run must fail");
        assert!(matches!(
            inconsistent_run,
            StoreError::InvalidTransition(message) if message.contains("inconsistent")
        ));
        connection
            .execute(
                "UPDATE task_runs SET claim_owner = 'worker' WHERE id = ?1",
                ["r_block_guards"],
            )
            .await
            .expect("restore run claim owner");

        connection
            .execute(
                "UPDATE task_runs SET status = 'succeeded' WHERE id = ?1",
                ["r_block_guards"],
            )
            .await
            .expect("remove active run");
        let no_active_run = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_block_guards"),
                    false,
                    "waiting",
                    500,
                    "e_block_no_active_run",
                ),
            )
            .await
            .expect_err("missing active run must fail");
        assert!(matches!(no_active_run, StoreError::InvalidTransition(_)));
        connection
            .execute(
                "UPDATE task_runs SET status = 'running' WHERE id = ?1",
                ["r_block_guards"],
            )
            .await
            .expect("restore active run");

        connection
            .execute("PRAGMA ignore_check_constraints = ON", ())
            .await
            .expect("disable checks for damaged claim");
        connection
            .execute(
                "UPDATE tasks SET claim_expires_at = NULL WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("remove claim expiry");
        connection
            .execute("PRAGMA ignore_check_constraints = OFF", ())
            .await
            .expect("restore checks after damaged claim");
        let missing_claim = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "admin",
                    None,
                    true,
                    "waiting",
                    500,
                    "e_block_missing_claim",
                ),
            )
            .await
            .expect_err("missing claim expiry must fail");
        assert!(matches!(
            missing_claim,
            StoreError::InvalidTransition(message) if message.contains("active claim")
        ));
        connection
            .execute("PRAGMA ignore_check_constraints = ON", ())
            .await
            .expect("disable checks to restore claim");
        connection
            .execute(
                "UPDATE tasks SET claim_expires_at = ?1 WHERE id = ?2",
                (1_300_i64, task.id.as_str()),
            )
            .await
            .expect("restore claim expiry");
        connection
            .execute("PRAGMA ignore_check_constraints = OFF", ())
            .await
            .expect("restore checks");

        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 600 WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("archive task");
        let archived = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_block_guards"),
                    false,
                    "waiting",
                    500,
                    "e_block_archived",
                ),
            )
            .await
            .expect_err("archived task must fail");
        assert!(matches!(
            archived,
            StoreError::InvalidTransition(message) if message.contains("archived")
        ));
        connection
            .execute(
                "UPDATE tasks SET status = 'running', archived_at = NULL WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("restore task archive state");

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged task");
        assert_eq!(unchanged.status, "running");
        assert_eq!(unchanged.lock_version, claimed.task.lock_version);
        assert_eq!(unchanged.claim_token, claimed.task.claim_token);
        assert_eq!(unchanged.claim_owner, claimed.task.claim_owner);
        assert_eq!(unchanged.current_run_id, claimed.task.current_run_id);
        let completed_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.blocked'",
                    [task.id.as_str()],
                )
                .await
                .expect("blocked event count query"),
        )
        .await
        .expect("blocked event count row");
        assert_eq!(
            integer_value(
                completed_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn block_task_force_bypasses_caller_credentials_but_keeps_claim_consistency() {
        let (_directory, store, _path) = store("block-force").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_block_force", "block-force", "Block force").await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_block_force",
                    "r_block_force",
                    "e_block_force_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let blocked = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "admin",
                    Some("wrong-token"),
                    true,
                    "manual intervention",
                    500,
                    "e_block_force",
                ),
            )
            .await
            .expect("force block task");
        assert_eq!(blocked.status, "blocked");
        assert_eq!(
            blocked.status_reason.as_deref(),
            Some("manual intervention")
        );
        assert_eq!(blocked.claim_token, None);
        assert_eq!(blocked.claim_owner, None);
        assert_eq!(blocked.current_run_id.as_deref(), Some("r_block_force"));

        let connection = store.connection().await.expect("connection");
        let run = first_row(
            connection
                .query(
                    "SELECT status, error FROM task_runs WHERE id = ?1",
                    ["r_block_force"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "failed"
        );
        assert_eq!(
            text_value(run.get_value(1).expect("run error"), "run.error").expect("run error text"),
            "manual intervention"
        );
        let event = first_row(
            connection
                .query(
                    "SELECT actor, run_id, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_block_force"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "admin"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_block_force"
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"reason":"manual intervention"}"#
        );
    }

    #[tokio::test]
    async fn block_task_event_conflict_rolls_back_task_and_run() {
        let (_directory, store, _path) = store("block-event-conflict").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_block_event_conflict",
            "block-event-conflict",
            "Block event conflict",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_block_event_conflict",
                    "r_block_event_conflict",
                    "e_block_event_claim",
                    "{}",
                    300,
                    1_000,
                ),
            )
            .await
            .expect("claim task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', ?2, NULL, 'other.event', 'tester', '{}', 1)",
                ("e_block_event_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");
        let error = store
            .block_task(
                &task.id,
                block_input(
                    claimed.task.lock_version,
                    "worker",
                    Some("claim_block_event_conflict"),
                    false,
                    "should rollback",
                    500,
                    "e_block_event_conflict",
                ),
            )
            .await
            .expect_err("event conflict must fail");
        assert!(matches!(error, StoreError::Turso(_)));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get rolled back task");
        assert_eq!(unchanged.status, "running");
        assert_eq!(unchanged.lock_version, claimed.task.lock_version);
        assert_eq!(unchanged.claim_token, claimed.task.claim_token);
        assert_eq!(unchanged.claim_owner, claimed.task.claim_owner);
        assert_eq!(unchanged.current_run_id, claimed.task.current_run_id);
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, exit_code, error FROM task_runs WHERE id = ?1",
                    ["r_block_event_conflict"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "running"
        );
        assert!(matches!(
            run.get_value(1).expect("run finished"),
            Value::Null
        ));
        assert!(matches!(run.get_value(2).expect("run exit"), Value::Null));
        assert!(matches!(run.get_value(3).expect("run error"), Value::Null));
        let blocked_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.blocked'",
                    [task.id.as_str()],
                )
                .await
                .expect("blocked event count query"),
        )
        .await
        .expect("blocked event count row");
        assert_eq!(
            integer_value(
                blocked_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn block_task_validates_input_without_writes() {
        let (_directory, store, _path) = store("block-input").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_block_input", "block-input", "Block input").await;
        let cases = [
            (
                "task id",
                "default#1".to_owned(),
                block_input(
                    1,
                    "worker",
                    None,
                    false,
                    "waiting",
                    500,
                    "e_block_input_task",
                ),
            ),
            (
                "expected_lock_version",
                task.id.clone(),
                block_input(
                    -1,
                    "worker",
                    None,
                    false,
                    "waiting",
                    500,
                    "e_block_input_version",
                ),
            ),
            (
                "actor",
                task.id.clone(),
                block_input(1, " ", None, false, "waiting", 500, "e_block_input_actor"),
            ),
            (
                "reason",
                task.id.clone(),
                block_input(1, "worker", None, false, "  ", 500, "e_block_input_reason"),
            ),
            (
                "event_id",
                task.id.clone(),
                block_input(1, "worker", None, false, "waiting", 500, "invalid_event"),
            ),
            (
                "now",
                task.id.clone(),
                block_input(1, "worker", None, false, "waiting", -1, "e_block_input_now"),
            ),
        ];
        for (field, task_id, input) in cases {
            let error = store
                .block_task(&task_id, input)
                .await
                .expect_err("invalid block input must fail");
            assert!(matches!(error, StoreError::InvalidInput(message) if message.contains(field)));
        }
        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged input task");
        assert_eq!(unchanged.status, "ready");
        assert_eq!(unchanged.lock_version, task.lock_version);
        assert_eq!(unchanged.claim_token, None);
        assert_eq!(unchanged.current_run_id, None);
        let connection = store.connection().await.expect("connection");
        let blocked_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.blocked'",
                    [task.id.as_str()],
                )
                .await
                .expect("blocked event count query"),
        )
        .await
        .expect("blocked event count row");
        assert_eq!(
            integer_value(
                blocked_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn block_task_uses_global_task_board_for_event_and_update() {
        let (_directory, store, _path) = store("block-multi-board").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_block_other', 'block-other', 'Block other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let task = store
            .create_task(
                "block-other",
                create_input("t_block_other", Some("block-other"), "Block other task"),
            )
            .await
            .expect("create other-board task");
        store
            .mark_execution_plan_not_required(
                &task.id,
                plan_input("No block plan", "planner", "e_block_other_plan", 100),
            )
            .await
            .expect("mark plan not required");
        store
            .promote_task(
                &task.id,
                promote_input(0, "promoter", "e_block_other_promote", 200),
            )
            .await
            .expect("promote other-board task");
        let blocked = store
            .block_task(
                &task.id,
                block_input(
                    1,
                    "worker",
                    None,
                    false,
                    "other board waiting",
                    500,
                    "e_block_other",
                ),
            )
            .await
            .expect("block other-board task");
        assert_eq!(blocked.board_id, "b_block_other");
        assert_eq!(blocked.board_slug, "block-other");
        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, kind, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_block_other"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_block_other"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert!(matches!(
            event.get_value(2).expect("event run"),
            Value::Null
        ));
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.blocked"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"reason":"other board waiting"}"#
        );
    }

    #[tokio::test]
    async fn block_task_rejects_non_running_task_with_residual_active_run_without_writes() {
        let (_directory, store, _path) = store("block-residual-run").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input(
                    "t_block_residual_run",
                    Some("block-residual-run"),
                    "Residual run",
                ),
            )
            .await
            .expect("create todo task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_runs(id, board_id, task_id, status, worker_profile, worker_pid, claim_token, claim_owner, claim_expires_at, started_at, last_heartbeat_at, metadata_json) VALUES (?1, 'b_default', ?2, 'running', 'manual', NULL, ?3, ?4, ?5, ?6, ?6, '{}')",
                (
                    "r_block_residual_run",
                    task.id.as_str(),
                    "residual-token",
                    "residual-owner",
                    1_000_i64,
                    300_i64,
                ),
            )
            .await
            .expect("insert residual active run");

        let error = store
            .block_task(
                &task.id,
                block_input(
                    0,
                    "operator",
                    None,
                    false,
                    "waiting",
                    500,
                    "e_block_residual_run",
                ),
            )
            .await
            .expect_err("residual active run must reject block");
        assert!(matches!(
            error,
            StoreError::InvalidTransition(message) if message.contains("active running run")
        ));

        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged task");
        assert_eq!(unchanged.status, "todo");
        assert_eq!(unchanged.status_reason, None);
        assert_eq!(unchanged.lock_version, task.lock_version);
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, exit_code, error FROM task_runs WHERE id = ?1",
                    ["r_block_residual_run"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "running"
        );
        assert!(matches!(
            run.get_value(1).expect("run finished"),
            Value::Null
        ));
        assert!(matches!(run.get_value(2).expect("run exit"), Value::Null));
        assert!(matches!(run.get_value(3).expect("run error"), Value::Null));
        let blocked_event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.blocked'",
                    [task.id.as_str()],
                )
                .await
                .expect("blocked event count query"),
        )
        .await
        .expect("blocked event count row");
        assert_eq!(
            integer_value(
                blocked_event_count.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn reclaim_expired_task_returns_ready_and_closes_run_atomically() {
        let (_directory, store, _path) = store("reclaim-expired-success").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_reclaim_expired_success",
            "reclaim-expired-success",
            "Reclaim expired",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_success",
                    "r_reclaim_success",
                    "e_reclaim_success_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim task");

        let expired = store
            .list_expired_claims(" default ", 500)
            .await
            .expect("list expired claims");
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].id, task.id);

        let reclaimed = store
            .reclaim_expired_task(
                &task.id,
                reclaim_input(
                    claimed.task.lock_version,
                    "dispatcher",
                    "e_reclaim_success",
                    "ready",
                    1,
                    "claim expired",
                    500,
                ),
            )
            .await
            .expect("reclaim expired task")
            .expect("expired task must be reclaimed");
        assert_eq!(reclaimed.status, "ready");
        assert_eq!(reclaimed.retry_count, 1);
        assert_eq!(reclaimed.lock_version, claimed.task.lock_version + 1);
        assert_eq!(reclaimed.claim_token, None);
        assert_eq!(reclaimed.claim_owner, None);
        assert_eq!(reclaimed.claim_expires_at, None);
        assert_eq!(reclaimed.last_heartbeat_at, None);
        assert_eq!(reclaimed.current_run_id, None);

        let connection = store.connection().await.expect("connection");
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, error FROM task_runs WHERE id = ?1",
                    ["r_reclaim_success"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "expired"
        );
        assert_eq!(
            integer_value(run.get_value(1).expect("run finished"), "run.finished_at")
                .expect("run finished integer"),
            500
        );
        assert_eq!(
            text_value(run.get_value(2).expect("run error"), "run.error").expect("run error text"),
            "claim expired"
        );
        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_reclaim_success"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_default"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_reclaim_success"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.reclaimed"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "dispatcher"
        );
        assert_eq!(
            text_value(event.get_value(5).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"retry_count":1,"max_retries":2,"to_status":"ready","reason":"claim expired"}"#
        );
        assert_eq!(
            integer_value(
                event.get_value(6).expect("event created"),
                "event.created_at"
            )
            .expect("event created integer"),
            500
        );
    }

    #[tokio::test]
    async fn reclaim_expired_task_recomputes_retry_target_from_canonical_facts() {
        let (_directory, store, _path) = store("reclaim-expired-targets").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");

        let maxed =
            ready_task_for_claim(&store, "t_reclaim_maxed", "reclaim-maxed", "Maxed retry").await;
        connection
            .execute(
                "UPDATE tasks SET max_retries = 1 WHERE id = ?1",
                [maxed.id.as_str()],
            )
            .await
            .expect("set max retries");
        let maxed_claim = store
            .claim_task(
                &maxed.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_maxed",
                    "r_reclaim_maxed",
                    "e_reclaim_maxed_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim maxed task");
        let maxed_reclaimed = store
            .reclaim_expired_task(
                &maxed.id,
                reclaim_input(
                    maxed_claim.task.lock_version,
                    "dispatcher",
                    "e_reclaim_maxed",
                    "blocked",
                    1,
                    "max retries reached",
                    500,
                ),
            )
            .await
            .expect("reclaim maxed task")
            .expect("maxed task reclaimed");
        assert_eq!(maxed_reclaimed.status, "blocked");
        assert_eq!(
            maxed_reclaimed.status_reason.as_deref(),
            Some("max retries reached")
        );
        assert_eq!(maxed_reclaimed.retry_count, 1);

        let dependency = ready_task_for_claim(
            &store,
            "t_reclaim_dependency",
            "reclaim-dependency",
            "Dependency retry",
        )
        .await;
        let parent = store
            .create_task(
                "default",
                create_input(
                    "t_reclaim_dependency_parent",
                    Some("reclaim-dependency-parent"),
                    "Unfinished parent",
                ),
            )
            .await
            .expect("create dependency parent");
        let dependency_claim = store
            .claim_task(
                &dependency.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_dependency",
                    "r_reclaim_dependency",
                    "e_reclaim_dependency_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim dependency task");
        connection
            .execute(
                "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES ('b_default', ?1, ?2, 400)",
                (parent.id.as_str(), dependency.id.as_str()),
            )
            .await
            .expect("insert dependency");
        let dependency_reclaimed = store
            .reclaim_expired_task(
                &dependency.id,
                reclaim_input(
                    dependency_claim.task.lock_version,
                    "dispatcher",
                    "e_reclaim_dependency",
                    "todo",
                    1,
                    "claim expired",
                    500,
                ),
            )
            .await
            .expect("reclaim dependency task")
            .expect("dependency task reclaimed");
        assert_eq!(dependency_reclaimed.status, "todo");

        let unplanned = ready_task_for_claim(
            &store,
            "t_reclaim_unplanned",
            "reclaim-unplanned",
            "Unplanned retry",
        )
        .await;
        let unplanned_claim = store
            .claim_task(
                &unplanned.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_unplanned",
                    "r_reclaim_unplanned",
                    "e_reclaim_unplanned_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim unplanned task");
        connection
            .execute(
                "UPDATE task_execution_plans SET state = 'planned' WHERE task_id = ?1",
                [unplanned.id.as_str()],
            )
            .await
            .expect("make plan non-ready");
        let unplanned_reclaimed = store
            .reclaim_expired_task(
                &unplanned.id,
                reclaim_input(
                    unplanned_claim.task.lock_version,
                    "dispatcher",
                    "e_reclaim_unplanned",
                    "todo",
                    1,
                    "claim expired",
                    500,
                ),
            )
            .await
            .expect("reclaim unplanned task")
            .expect("unplanned task reclaimed");
        assert_eq!(unplanned_reclaimed.status, "todo");

        let scheduled = ready_task_for_claim(
            &store,
            "t_reclaim_scheduled",
            "reclaim-scheduled",
            "Scheduled retry",
        )
        .await;
        let scheduled_claim = store
            .claim_task(
                &scheduled.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_scheduled",
                    "r_reclaim_scheduled",
                    "e_reclaim_scheduled_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim scheduled task");
        connection
            .execute(
                "UPDATE tasks SET scheduled_at = 1_000 WHERE id = ?1",
                [scheduled.id.as_str()],
            )
            .await
            .expect("schedule task");
        let scheduled_reclaimed = store
            .reclaim_expired_task(
                &scheduled.id,
                reclaim_input(
                    scheduled_claim.task.lock_version,
                    "dispatcher",
                    "e_reclaim_scheduled",
                    "scheduled",
                    1,
                    "claim expired",
                    500,
                ),
            )
            .await
            .expect("reclaim scheduled task")
            .expect("scheduled task reclaimed");
        assert_eq!(scheduled_reclaimed.status, "scheduled");

        let triage =
            ready_task_for_claim(&store, "t_reclaim_triage", "reclaim-triage", "Triage retry")
                .await;
        let triage_claim = store
            .claim_task(
                &triage.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_triage",
                    "r_reclaim_triage",
                    "e_reclaim_triage_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim triage task");
        connection
            .execute(
                "UPDATE tasks SET description = NULL WHERE id = ?1",
                [triage.id.as_str()],
            )
            .await
            .expect("remove task description");
        let triage_reclaimed = store
            .reclaim_expired_task(
                &triage.id,
                reclaim_input(
                    triage_claim.task.lock_version,
                    "dispatcher",
                    "e_reclaim_triage",
                    "triage",
                    1,
                    "claim expired",
                    500,
                ),
            )
            .await
            .expect("reclaim triage task")
            .expect("triage task reclaimed");
        assert_eq!(triage_reclaimed.status, "triage");
    }

    #[tokio::test]
    async fn reclaim_expired_task_skips_fresh_heartbeat_and_lock_races_without_writes() {
        let (_directory, store, _path) = store("reclaim-expired-races").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_reclaim_races", "reclaim-races", "Reclaim races").await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_races",
                    "r_reclaim_races",
                    "e_reclaim_races_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim task");

        assert!(
            store
                .list_expired_claims("default", 350)
                .await
                .expect("list fresh claims")
                .is_empty()
        );
        let fresh = store
            .reclaim_expired_task(
                &task.id,
                reclaim_input(
                    claimed.task.lock_version,
                    "dispatcher",
                    "e_reclaim_fresh",
                    "ready",
                    1,
                    "claim expired",
                    350,
                ),
            )
            .await
            .expect("fresh claim must be skipped");
        assert_eq!(fresh, None);

        let heartbeated = store
            .heartbeat_task(
                &task.id,
                heartbeat_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_reclaim_races",
                    "e_reclaim_races_heartbeat",
                    None,
                    400,
                    2_000,
                ),
            )
            .await
            .expect("heartbeat task");
        let heartbeat_race = store
            .reclaim_expired_task(
                &task.id,
                reclaim_input(
                    heartbeated.lock_version,
                    "dispatcher",
                    "e_reclaim_heartbeat_race",
                    "ready",
                    1,
                    "claim expired",
                    1_000,
                ),
            )
            .await
            .expect("heartbeated claim must be skipped");
        assert_eq!(heartbeat_race, None);

        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE tasks SET lock_version = lock_version + 1 WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("advance task lock");
        let lock_race = store
            .reclaim_expired_task(
                &task.id,
                reclaim_input(
                    heartbeated.lock_version,
                    "dispatcher",
                    "e_reclaim_lock_race",
                    "ready",
                    1,
                    "claim expired",
                    2_500,
                ),
            )
            .await
            .expect("lock race must be skipped");
        assert_eq!(lock_race, None);
        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged race task");
        assert_eq!(unchanged.status, "running");
        assert_eq!(unchanged.retry_count, 0);
        assert_eq!(unchanged.current_run_id.as_deref(), Some("r_reclaim_races"));
        let reclaimed_events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.reclaimed'",
                    [task.id.as_str()],
                )
                .await
                .expect("reclaimed event count"),
        )
        .await
        .expect("reclaimed event count row");
        assert_eq!(
            integer_value(
                reclaimed_events.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn reclaim_expired_task_rejects_inconsistent_run_and_rolls_back_event_conflict() {
        let (_directory, store, _path) = store("reclaim-expired-rollback").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_reclaim_rollback",
            "reclaim-rollback",
            "Reclaim rollback",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_rollback",
                    "r_reclaim_rollback",
                    "e_reclaim_rollback_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE task_runs SET claim_owner = 'different-worker' WHERE id = ?1",
                ["r_reclaim_rollback"],
            )
            .await
            .expect("corrupt run owner");
        let inconsistent = store
            .reclaim_expired_task(
                &task.id,
                reclaim_input(
                    claimed.task.lock_version,
                    "dispatcher",
                    "e_reclaim_inconsistent",
                    "ready",
                    1,
                    "claim expired",
                    500,
                ),
            )
            .await
            .expect_err("inconsistent run must fail");
        assert!(matches!(
            inconsistent,
            StoreError::InvalidTransition(message) if message.contains("inconsistent")
        ));
        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get inconsistent task");
        assert_eq!(unchanged.status, "running");
        assert_eq!(unchanged.lock_version, claimed.task.lock_version);

        connection
            .execute(
                "UPDATE task_runs SET claim_owner = ?1 WHERE id = ?2",
                ("worker", "r_reclaim_rollback"),
            )
            .await
            .expect("restore run owner");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', ?2, NULL, 'other.event', 'tester', '{}', 1)",
                ("e_reclaim_event_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");
        let event_error = store
            .reclaim_expired_task(
                &task.id,
                reclaim_input(
                    claimed.task.lock_version,
                    "dispatcher",
                    "e_reclaim_event_conflict",
                    "ready",
                    1,
                    "claim expired",
                    500,
                ),
            )
            .await
            .expect_err("event conflict must fail");
        assert!(matches!(event_error, StoreError::Turso(_)));
        let rolled_back = store
            .get_task_global(&task.id)
            .await
            .expect("get rolled back task");
        assert_eq!(rolled_back.status, "running");
        assert_eq!(rolled_back.lock_version, claimed.task.lock_version);
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, error FROM task_runs WHERE id = ?1",
                    ["r_reclaim_rollback"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "running"
        );
        assert!(matches!(
            run.get_value(1).expect("run finished"),
            Value::Null
        ));
        assert!(matches!(run.get_value(2).expect("run error"), Value::Null));
        let reclaimed_events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.reclaimed'",
                    [task.id.as_str()],
                )
                .await
                .expect("reclaimed event count"),
        )
        .await
        .expect("reclaimed event count row");
        assert_eq!(
            integer_value(
                reclaimed_events.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn list_expired_claims_is_board_isolated_and_excludes_archived_records() {
        let (_directory, store, _path) = store("reclaim-expired-isolation").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_reclaim_other', 'reclaim-other', 'Reclaim other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let other = store
            .create_task(
                "reclaim-other",
                create_input(
                    "t_reclaim_other",
                    Some("reclaim-other"),
                    "Other board reclaim",
                ),
            )
            .await
            .expect("create other-board task");
        store
            .mark_execution_plan_not_required(
                &other.id,
                plan_input("No plan", "planner", "e_reclaim_other_plan", 100),
            )
            .await
            .expect("mark other plan");
        store
            .promote_task(
                &other.id,
                promote_input(0, "promoter", "e_reclaim_other_promote", 200),
            )
            .await
            .expect("promote other task");
        let other_claim = store
            .claim_task(
                &other.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_other",
                    "r_reclaim_other",
                    "e_reclaim_other_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim other task");
        assert!(
            store
                .list_expired_claims("default", 500)
                .await
                .expect("list default expired claims")
                .is_empty()
        );
        let other_expired = store
            .list_expired_claims("reclaim-other", 500)
            .await
            .expect("list other expired claims");
        assert_eq!(other_expired.len(), 1);
        assert_eq!(other_expired[0].id, other.id);
        assert_eq!(other_expired[0].board_id, "b_reclaim_other");
        assert_eq!(other_expired[0].lock_version, other_claim.task.lock_version);

        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 600 WHERE id = ?1",
                [other.id.as_str()],
            )
            .await
            .expect("archive other task");
        assert!(
            store
                .list_expired_claims("reclaim-other", 700)
                .await
                .expect("list archived task claims")
                .is_empty()
        );
        let archived = store
            .reclaim_expired_task(
                &other.id,
                reclaim_input(
                    other_claim.task.lock_version,
                    "dispatcher",
                    "e_reclaim_archived",
                    "ready",
                    1,
                    "claim expired",
                    700,
                ),
            )
            .await
            .expect("archived task must be skipped");
        assert_eq!(archived, None);
    }

    #[tokio::test]
    async fn create_comment_writes_comment_and_event_atomically() {
        let (_directory, store, _path) = store("comment-create-success").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input("t_comment_success", None, "Comment task"),
            )
            .await
            .expect("create task");

        let comment = store
            .create_comment(
                &task.id,
                comment_input(
                    "c_comment_success",
                    Some("comment-key"),
                    " operator ",
                    "user",
                    None,
                    " handoff note ",
                    "note",
                    " {} ",
                    "e_comment_success",
                    500,
                ),
            )
            .await
            .expect("create comment");
        assert_eq!(comment.id, "c_comment_success");
        assert_eq!(comment.board_id, "b_default");
        assert_eq!(comment.task_id, task.id);
        assert_eq!(comment.idempotency_key.as_deref(), Some("comment-key"));
        assert_eq!(comment.author, "operator");
        assert_eq!(comment.author_type, "user");
        assert_eq!(comment.agent_type, None);
        assert_eq!(comment.body, "handoff note");
        assert_eq!(comment.kind, "note");
        assert_eq!(comment.metadata_json, "{}");
        assert_eq!(comment.created_at, 500);

        let connection = store.connection().await.expect("connection");
        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_comment_success"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_default"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.comment.created"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "operator"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"comment_id":"c_comment_success","kind":"note","author_type":"user","agent_type":null}"#
        );
        assert_eq!(
            integer_value(
                event.get_value(5).expect("event created"),
                "event.created_at"
            )
            .expect("event created integer"),
            500
        );
        let comment_events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.comment.created'",
                    [task.id.as_str()],
                )
                .await
                .expect("comment event count"),
        )
        .await
        .expect("comment event count row");
        assert_eq!(
            integer_value(
                comment_events.get_value(0).expect("comment event count"),
                "event.count",
            )
            .expect("comment event count integer"),
            1
        );
    }

    #[tokio::test]
    async fn create_comment_replays_same_payload_and_conflicts_on_changed_payload() {
        let (_directory, store, _path) = store("comment-create-idempotency").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input("t_comment_idempotency", None, "Comment idempotency"),
            )
            .await
            .expect("create task");
        let first_input = comment_input(
            "c_comment_idempotency",
            Some("comment-replay"),
            "operator",
            "agent",
            Some("executor"),
            "same body",
            "decision",
            r#"{"options":[{"slug":"keep","title":"Keep","detail":"Keep the existing path"}],"selected":"keep","reason":"Test the idempotency path"}"#,
            "e_comment_idempotency_first",
            500,
        );
        let first = store
            .create_comment(&task.id, first_input.clone())
            .await
            .expect("first comment");
        let mut replay_input = first_input;
        replay_input.id = "c_comment_idempotency_retry".to_owned();
        replay_input.event_id = "e_comment_idempotency_retry".to_owned();
        replay_input.created_at = 900;
        let replay = store
            .create_comment(&task.id, replay_input)
            .await
            .expect("replay comment");
        assert_eq!(replay, first);

        let mut changed_input = comment_input(
            "c_comment_idempotency_changed",
            Some("comment-replay"),
            "operator",
            "agent",
            Some("executor"),
            "changed body",
            "decision",
            r#"{"options":[{"slug":"keep","title":"Keep","detail":"Keep the existing path"}],"selected":"keep","reason":"Test the idempotency path"}"#,
            "e_comment_idempotency_changed",
            1_000,
        );
        changed_input.body = "different body".to_owned();
        let conflict = store
            .create_comment(&task.id, changed_input)
            .await
            .expect_err("changed payload must conflict");
        assert!(matches!(
            conflict,
            StoreError::IdempotencyConflict {
                board_id,
                key,
                existing_task_id
            } if board_id == "b_default" && key == "comment-replay" && existing_task_id == task.id
        ));

        let connection = store.connection().await.expect("connection");
        let comments = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_comments WHERE board_id = ?1 AND task_id = ?2",
                    ("b_default", task.id.as_str()),
                )
                .await
                .expect("comment count"),
        )
        .await
        .expect("comment count row");
        assert_eq!(
            integer_value(
                comments.get_value(0).expect("comment count"),
                "comment.count"
            )
            .expect("comment count integer"),
            1
        );
        let events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.comment.created'",
                    [task.id.as_str()],
                )
                .await
                .expect("comment event count"),
        )
        .await
        .expect("comment event count row");
        assert_eq!(
            integer_value(events.get_value(0).expect("event count"), "event.count")
                .expect("event count integer"),
            1
        );
    }

    #[tokio::test]
    async fn list_comments_resolves_task_board_orders_history_and_reads_archived_tasks() {
        let (_directory, store, _path) = store("comment-list-history").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_comment_list_other', 'comment-list-other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let task = store
            .create_task(
                "default",
                create_input("t_comment_list", None, "Comment list"),
            )
            .await
            .expect("create task");
        let other = store
            .create_task(
                "comment-list-other",
                create_input("t_comment_list_other", None, "Other comment list"),
            )
            .await
            .expect("create other task");

        for (id, created_at) in [("c_comment_list_late", 200), ("c_comment_list_b", 100)] {
            store
                .create_comment(
                    &task.id,
                    comment_input(
                        id,
                        None,
                        "operator",
                        "user",
                        None,
                        id,
                        "note",
                        "{}",
                        &format!("e_{id}"),
                        created_at,
                    ),
                )
                .await
                .expect("create comment");
        }
        store
            .create_comment(
                &task.id,
                comment_input(
                    "c_comment_list_a",
                    None,
                    "operator",
                    "user",
                    None,
                    "same timestamp",
                    "note",
                    "{}",
                    "e_c_comment_list_a",
                    100,
                ),
            )
            .await
            .expect("create same-timestamp comment");
        store
            .create_comment(
                &other.id,
                comment_input(
                    "c_comment_list_other",
                    None,
                    "operator",
                    "user",
                    None,
                    "other board",
                    "note",
                    "{}",
                    "e_c_comment_list_other",
                    50,
                ),
            )
            .await
            .expect("create other-board comment");

        let comments = store.list_comments(&task.id).await.expect("list comments");
        assert_eq!(
            comments
                .iter()
                .map(|comment| comment.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "c_comment_list_a",
                "c_comment_list_b",
                "c_comment_list_late"
            ]
        );
        assert!(
            comments
                .iter()
                .all(|comment| comment.board_id == task.board_id)
        );

        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 300 WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("archive task");
        assert_eq!(
            store
                .list_comments(&task.id)
                .await
                .expect("archived history")
                .len(),
            3
        );

        let unknown = store
            .list_comments("t_comment_list_unknown")
            .await
            .expect_err("unknown task must fail");
        assert!(matches!(unknown, StoreError::TaskNotFound(id) if id == "t_comment_list_unknown"));
    }

    #[tokio::test]
    async fn create_step_plans_parent_recomputes_status_and_lists_in_canonical_order() {
        let (_directory, store, _path) = store("step-create-list").await;
        store.initialize().await.expect("initialize");
        let parent = store
            .create_task(
                "default",
                create_input("t_step_parent", None, "Step parent"),
            )
            .await
            .expect("create parent");
        let first = store
            .create_step(
                &parent.id,
                step_input(
                    "step_first",
                    Some("step-replay"),
                    "  First step  ",
                    None,
                    "operator",
                    parent.lock_version,
                    "unplanned",
                    "ready",
                    "e_step_first",
                    "e_step_plan",
                    "e_step_recompute",
                    500,
                ),
            )
            .await
            .expect("create step");
        assert_eq!(first.id, "step_first");
        assert_eq!(first.title, "First step");
        assert_eq!(first.position, 1024);
        assert_eq!(first.status, "todo");
        let listed = store.list_steps(&parent.id).await.expect("list steps");
        assert_eq!(listed.steps.len(), 1);
        assert_eq!(listed.steps[0], first);
        assert_eq!(listed.execution_plan.state, "planned");
        let updated_parent = store
            .get_task_global(&parent.id)
            .await
            .expect("read recomputed parent");
        assert_eq!(updated_parent.status, "ready");
        assert_eq!(updated_parent.lock_version, 1);

        let connection = store.connection().await.expect("connection");
        let events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind IN ('task.step.created', 'task.execution_plan.planned', 'task.recomputed')",
                    [parent.id.as_str()],
                )
                .await
                .expect("step events"),
        )
        .await
        .expect("step event row");
        assert_eq!(
            integer_value(events.get_value(0).expect("event count"), "events")
                .expect("event integer"),
            3
        );
    }

    #[tokio::test]
    async fn create_step_replays_same_payload_without_events_and_rejects_conflicts_or_archived_parents()
     {
        let (_directory, store, _path) = store("step-idempotency").await;
        store.initialize().await.expect("initialize");
        let parent = store
            .create_task(
                "default",
                create_input("t_step_idempotent", None, "Step idempotent"),
            )
            .await
            .expect("create parent");
        let first_input = step_input(
            "step_idempotent",
            Some("step-replay"),
            "step",
            None,
            "operator",
            parent.lock_version,
            "unplanned",
            "ready",
            "e_step_idempotent",
            "e_step_idempotent_plan",
            "e_step_idempotent_recompute",
            500,
        );
        let first = store
            .create_step(&parent.id, first_input.clone())
            .await
            .expect("first step");
        let replay = store
            .create_step(
                &parent.id,
                CreateStepInput {
                    id: "step_retry_id".into(),
                    event_id: "e_step_retry".into(),
                    plan_event_id: "e_step_retry_plan".into(),
                    recompute_event_id: "e_step_retry_recompute".into(),
                    expected_lock_version: 1,
                    expected_plan_state: "planned".into(),
                    created_at: 900,
                    ..first_input.clone()
                },
            )
            .await
            .expect("idempotent replay");
        assert_eq!(replay, first);
        let changed = store
            .create_step(
                &parent.id,
                CreateStepInput {
                    title: "different".into(),
                    id: "step_changed".into(),
                    event_id: "e_step_changed".into(),
                    plan_event_id: "e_step_changed_plan".into(),
                    recompute_event_id: "e_step_changed_recompute".into(),
                    expected_lock_version: 1,
                    expected_plan_state: "planned".into(),
                    created_at: 1_000,
                    ..first_input.clone()
                },
            )
            .await
            .expect_err("changed payload must conflict");
        assert!(
            matches!(changed, StoreError::IdempotencyConflict { key, .. } if key == "step-replay")
        );
        let events = first_row(
            store
                .connection()
                .await
                .expect("connection")
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.step.created'",
                    [parent.id.as_str()],
                )
                .await
                .expect("step event count"),
        )
        .await
        .expect("step event row");
        assert_eq!(
            integer_value(events.get_value(0).expect("event count"), "events")
                .expect("event integer"),
            1
        );

        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 1000 WHERE id = ?1",
                [parent.id.as_str()],
            )
            .await
            .expect("archive parent");
        assert_eq!(
            store
                .list_steps(&parent.id)
                .await
                .expect("archived list")
                .steps
                .len(),
            1
        );
        let archived_error = store
            .create_step(
                &parent.id,
                CreateStepInput {
                    id: "step_archived".into(),
                    idempotency_key: None,
                    title: "archived".into(),
                    body: None,
                    linked_task_id: None,
                    position: Some(2048),
                    required: true,
                    created_by: "operator".into(),
                    event_id: "e_step_archived".into(),
                    plan_event_id: "e_step_archived_plan".into(),
                    recompute_event_id: "e_step_archived_recompute".into(),
                    created_at: 1_100,
                    expected_lock_version: 1,
                    expected_plan_state: "planned".into(),
                    target_status: "ready".into(),
                },
            )
            .await
            .expect_err("archived create must fail");
        assert!(
            matches!(archived_error, StoreError::InvalidTransition(message) if message.contains("archived"))
        );
    }

    #[tokio::test]
    async fn create_step_rejects_cross_board_and_self_links_and_rolls_back_event_conflicts() {
        let (_directory, store, _path) = store("step-guards").await;
        store.initialize().await.expect("initialize");
        let parent = store
            .create_task(
                "default",
                create_input("t_step_guard_parent", None, "Step guard parent"),
            )
            .await
            .expect("create parent");

        let self_error = store
            .create_step(
                &parent.id,
                CreateStepInput {
                    id: "step_self_link".into(),
                    idempotency_key: None,
                    linked_task_id: Some(parent.id.clone()),
                    event_id: "e_step_self_link".into(),
                    plan_event_id: "e_step_self_link_plan".into(),
                    recompute_event_id: "e_step_self_link_recompute".into(),
                    expected_lock_version: parent.lock_version,
                    expected_plan_state: "unplanned".into(),
                    target_status: "ready".into(),
                    created_at: 600,
                    ..step_input(
                        "step_self_link",
                        None,
                        "self link",
                        None,
                        "operator",
                        parent.lock_version,
                        "unplanned",
                        "ready",
                        "e_step_self_link",
                        "e_step_self_link_plan",
                        "e_step_self_link_recompute",
                        600,
                    )
                },
            )
            .await
            .expect_err("self link must fail");
        assert!(matches!(
            self_error,
            StoreError::InvalidInput(message) if message.contains("parent")
        ));

        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_step_other', 'step-other', 'Other steps', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let linked = store
            .create_task(
                "step-other",
                create_input("t_step_other", None, "Other linked task"),
            )
            .await
            .expect("create linked task");
        let cross_board_error = store
            .create_step(
                &parent.id,
                CreateStepInput {
                    id: "step_cross_board".into(),
                    idempotency_key: None,
                    linked_task_id: Some(linked.id),
                    event_id: "e_step_cross_board".into(),
                    plan_event_id: "e_step_cross_board_plan".into(),
                    recompute_event_id: "e_step_cross_board_recompute".into(),
                    created_at: 700,
                    ..step_input(
                        "step_cross_board",
                        None,
                        "cross board",
                        None,
                        "operator",
                        parent.lock_version,
                        "unplanned",
                        "ready",
                        "e_step_cross_board",
                        "e_step_cross_board_plan",
                        "e_step_cross_board_recompute",
                        700,
                    )
                },
            )
            .await
            .expect_err("cross-board link must fail");
        assert!(matches!(
            cross_board_error,
            StoreError::InvalidInput(message) if message.contains("parent board")
        ));

        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES ('e_step_rollback', 'b_default', ?1, NULL, 'test.event', 'tester', '{}', 800)",
                [parent.id.as_str()],
            )
            .await
            .expect("insert conflicting event");
        let rollback_error = store
            .create_step(
                &parent.id,
                CreateStepInput {
                    id: "step_rollback".into(),
                    idempotency_key: None,
                    event_id: "e_step_rollback".into(),
                    plan_event_id: "e_step_rollback_plan".into(),
                    recompute_event_id: "e_step_rollback_recompute".into(),
                    created_at: 900,
                    ..step_input(
                        "step_rollback",
                        None,
                        "rollback",
                        None,
                        "operator",
                        parent.lock_version,
                        "unplanned",
                        "ready",
                        "e_step_rollback",
                        "e_step_rollback_plan",
                        "e_step_rollback_recompute",
                        900,
                    )
                },
            )
            .await
            .expect_err("event conflict must abort the transaction");
        assert!(matches!(rollback_error, StoreError::Turso(_)));
        assert!(
            store
                .list_steps(&parent.id)
                .await
                .expect("list after rollback")
                .steps
                .is_empty()
        );
        let parent_after = store
            .get_task_global(&parent.id)
            .await
            .expect("parent after rollback");
        assert_eq!(parent_after.status, "todo");
        assert_eq!(parent_after.lock_version, parent.lock_version);
        assert_eq!(parent_after.execution_plan_state, "unplanned");
        let leftovers = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND event_id IN ('e_step_rollback_plan', 'e_step_rollback_recompute')",
                    [parent.id.as_str()],
                )
                .await
                .expect("rollback events query"),
        )
        .await
        .expect("rollback event row");
        assert_eq!(
            integer_value(leftovers.get_value(0).expect("leftover count"), "events")
                .expect("leftover event integer"),
            0
        );
    }

    #[tokio::test]
    async fn update_step_is_atomic_preserves_null_body_and_emits_strict_payload() {
        let (_directory, store, _path) = store("step-update").await;
        store.initialize().await.expect("initialize");
        let parent = store
            .create_task(
                "default",
                create_input("t_step_update_parent", None, "Step update parent"),
            )
            .await
            .expect("create parent");
        let created = store
            .create_step(
                &parent.id,
                step_input(
                    "step_update",
                    None,
                    "Original title",
                    Some(1024),
                    "planner",
                    parent.lock_version,
                    "unplanned",
                    "ready",
                    "e_step_update_create",
                    "e_step_update_plan",
                    "e_step_update_recompute",
                    500,
                ),
            )
            .await
            .expect("create step");
        assert_eq!(created.body.as_deref(), Some("body"));
        let updated = store
            .update_step(
                &parent.id,
                &created.id,
                UpdateStepInput {
                    title: Some(" Updated title ".into()),
                    body: None,
                    linked_task_id: None,
                    unlink_task: false,
                    position: Some(2048),
                    required: Some(false),
                    updated_by: " reviewer ".into(),
                    event_id: "e_step_update_success".into(),
                    updated_at: 600,
                    expected_lock_version: 1,
                },
            )
            .await
            .expect("update step");
        assert_eq!(updated.title, "Updated title");
        assert_eq!(updated.body.as_deref(), Some("body"));
        assert_eq!(updated.position, 2048);
        assert!(!updated.required);
        assert_eq!(updated.status, "todo");
        assert_eq!(updated.updated_by, "reviewer");

        let parent_after = store
            .get_task_global(&parent.id)
            .await
            .expect("parent after update");
        assert_eq!(parent_after.status, "ready");
        assert_eq!(parent_after.lock_version, 2);
        let connection = store.connection().await.expect("connection");
        let event = first_row(
            connection
                .query(
                    "SELECT kind, actor, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_step_update_success"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.step.updated"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "reviewer"
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"step_id":"step_update","linked_task_id":null,"position":2048,"required":false,"status":"todo"}"#
        );

        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES ('e_step_update_conflict', 'b_default', ?1, NULL, 'test.event', 'tester', '{}', 700)",
                [parent.id.as_str()],
            )
            .await
            .expect("insert conflicting event");
        let conflict = store
            .update_step(
                &parent.id,
                &created.id,
                UpdateStepInput {
                    title: Some("Should roll back".into()),
                    body: Some("changed".into()),
                    linked_task_id: None,
                    unlink_task: false,
                    position: None,
                    required: None,
                    updated_by: "reviewer".into(),
                    event_id: "e_step_update_conflict".into(),
                    updated_at: 800,
                    expected_lock_version: 2,
                },
            )
            .await
            .expect_err("event conflict must roll back update");
        assert!(matches!(conflict, StoreError::Turso(_)));
        let unchanged = store
            .list_steps(&parent.id)
            .await
            .expect("list after rollback");
        assert_eq!(unchanged.steps[0].title, "Updated title");
        assert_eq!(unchanged.steps[0].body.as_deref(), Some("body"));
        assert_eq!(
            store
                .get_task_global(&parent.id)
                .await
                .expect("parent after rollback")
                .lock_version,
            2
        );
    }

    #[tokio::test]
    async fn update_step_rejects_invalid_links_and_empty_patches() {
        let (_directory, store, _path) = store("step-update-guards").await;
        store.initialize().await.expect("initialize");
        let parent = store
            .create_task(
                "default",
                create_input("t_step_update_guard", None, "Step update guard"),
            )
            .await
            .expect("create parent");
        let created = store
            .create_step(
                &parent.id,
                step_input(
                    "step_update_guard",
                    None,
                    "Guard",
                    Some(1024),
                    "planner",
                    parent.lock_version,
                    "unplanned",
                    "ready",
                    "e_step_update_guard_create",
                    "e_step_update_guard_plan",
                    "e_step_update_guard_recompute",
                    500,
                ),
            )
            .await
            .expect("create step");
        let self_link = store
            .update_step(
                &parent.id,
                &created.id,
                UpdateStepInput {
                    title: None,
                    body: None,
                    linked_task_id: Some(parent.id.clone()),
                    unlink_task: false,
                    position: None,
                    required: None,
                    updated_by: "planner".into(),
                    event_id: "e_step_update_guard_self".into(),
                    updated_at: 600,
                    expected_lock_version: 1,
                },
            )
            .await
            .expect_err("self link must fail");
        assert!(
            matches!(self_link, StoreError::InvalidInput(message) if message.contains("parent"))
        );
        let empty = store
            .update_step(
                &parent.id,
                &created.id,
                UpdateStepInput {
                    title: None,
                    body: None,
                    linked_task_id: None,
                    unlink_task: false,
                    position: None,
                    required: None,
                    updated_by: "planner".into(),
                    event_id: "e_step_update_guard_empty".into(),
                    updated_at: 600,
                    expected_lock_version: 1,
                },
            )
            .await
            .expect_err("empty patch must fail");
        assert!(
            matches!(empty, StoreError::InvalidInput(message) if message.contains("at least one"))
        );
    }

    #[tokio::test]
    async fn create_comment_enforces_task_board_isolation_and_rolls_back_event_conflicts() {
        let (_directory, store, _path) = store("comment-create-isolation").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_comment_other', 'comment-other', 'Comment other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let other = store
            .create_task(
                "comment-other",
                create_input("t_comment_other", None, "Other comment task"),
            )
            .await
            .expect("create other task");
        let other_comment = store
            .create_comment(
                &other.id,
                comment_input(
                    "c_comment_other",
                    None,
                    "operator",
                    "user",
                    None,
                    "other board",
                    "note",
                    "{}",
                    "e_comment_other",
                    500,
                ),
            )
            .await
            .expect("create other-board comment");
        assert_eq!(other_comment.board_id, "b_comment_other");
        let other_event = first_row(
            connection
                .query(
                    "SELECT board_id FROM task_events WHERE event_id = ?1",
                    ["e_comment_other"],
                )
                .await
                .expect("other event query"),
        )
        .await
        .expect("other event row");
        assert_eq!(
            text_value(
                other_event.get_value(0).expect("event board"),
                "event.board_id"
            )
            .expect("event board text"),
            "b_comment_other"
        );

        let invalid_decision = store
            .create_comment(
                &other.id,
                comment_input(
                    "c_comment_invalid_decision",
                    None,
                    "operator",
                    "user",
                    None,
                    "invalid decision",
                    "decision",
                    "{}",
                    "e_comment_invalid_decision",
                    500,
                ),
            )
            .await
            .expect_err("decision metadata must be validated");
        assert!(matches!(
            invalid_decision,
            StoreError::InvalidInput(message) if message.contains("decision")
        ));

        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_comment_other', ?2, NULL, 'other.event', 'tester', '{}', 1)",
                ("e_comment_conflict", other.id.as_str()),
            )
            .await
            .expect("insert conflicting event");
        let event_error = store
            .create_comment(
                &other.id,
                comment_input(
                    "c_comment_conflict",
                    None,
                    "operator",
                    "user",
                    None,
                    "must roll back",
                    "note",
                    "{}",
                    "e_comment_conflict",
                    500,
                ),
            )
            .await
            .expect_err("event conflict must fail");
        assert!(matches!(event_error, StoreError::Turso(_)));
        let rolled_back_comments = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_comments WHERE id = ?1",
                    ["c_comment_conflict"],
                )
                .await
                .expect("rolled-back comment count"),
        )
        .await
        .expect("rolled-back comment count row");
        assert_eq!(
            integer_value(
                rolled_back_comments
                    .get_value(0)
                    .expect("rolled-back comment count"),
                "comment.count",
            )
            .expect("rolled-back comment count integer"),
            0
        );

        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 600 WHERE id = ?1",
                [other.id.as_str()],
            )
            .await
            .expect("archive task");
        let archived_error = store
            .create_comment(
                &other.id,
                comment_input(
                    "c_comment_archived",
                    None,
                    "operator",
                    "user",
                    None,
                    "archived task",
                    "note",
                    "{}",
                    "e_comment_archived",
                    700,
                ),
            )
            .await
            .expect_err("archived task must reject comments");
        assert!(matches!(
            archived_error,
            StoreError::InvalidTransition(message) if message.contains("archived")
        ));
    }

    #[tokio::test]
    async fn dependency_create_list_recomputes_atomically_and_rejects_cycles() {
        let (_directory, store, _path) = store("dependency-create-list").await;
        store.initialize().await.expect("initialize");
        let parent = store
            .create_task(
                "default",
                create_input("t_dependency_parent", None, "Dependency parent"),
            )
            .await
            .expect("create parent");
        let child = store
            .create_task(
                "default",
                create_input("t_dependency_child", None, "Dependency child"),
            )
            .await
            .expect("create child");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE tasks SET status = 'done', completed_at = 400 WHERE id = ?1",
                [parent.id.as_str()],
            )
            .await
            .expect("finish parent");

        let first = store
            .add_dependency(
                &child.id,
                &parent.id,
                AddDependencyInput {
                    expected_child_lock_version: child.lock_version,
                    target_child_status: "todo".to_owned(),
                    actor: " tester ".to_owned(),
                    event_id: "e_dependency_added".to_owned(),
                    recompute_event_id: "e_dependency_recomputed".to_owned(),
                    now: 500,
                },
            )
            .await
            .expect("add dependency");
        assert!(first.added);
        assert_eq!(first.dependencies.task.id, child.id);
        assert_eq!(
            first
                .dependencies
                .parents
                .iter()
                .map(|task| task.id.as_str())
                .collect::<Vec<_>>(),
            vec![parent.id.as_str()]
        );
        assert!(first.dependencies.children.is_empty());
        assert_eq!(first.dependencies.edges.len(), 1);
        assert_eq!(first.dependencies.edges[0].parent.id, parent.id);
        assert_eq!(first.dependencies.edges[0].child.id, child.id);
        assert_eq!(first.dependencies.task.status, "todo");

        let replay = store
            .add_dependency(
                &child.id,
                &parent.id,
                AddDependencyInput {
                    expected_child_lock_version: child.lock_version,
                    target_child_status: "todo".to_owned(),
                    actor: "different actor".to_owned(),
                    event_id: "e_dependency_added_retry".to_owned(),
                    recompute_event_id: "e_dependency_recomputed_retry".to_owned(),
                    now: 900,
                },
            )
            .await
            .expect("dependency replay");
        assert!(!replay.added);
        assert_eq!(replay.dependencies, first.dependencies);
        let events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind IN ('dependency.added', 'task.recomputed')",
                    [child.id.as_str()],
                )
                .await
                .expect("dependency event count"),
        )
        .await
        .expect("dependency event count row");
        assert_eq!(
            integer_value(events.get_value(0).expect("event count"), "event.count")
                .expect("event count integer"),
            1
        );

        let listed = store
            .list_dependencies(&child.id)
            .await
            .expect("list dependencies");
        assert_eq!(listed, first.dependencies);

        let cycle = store
            .add_dependency(
                &parent.id,
                &child.id,
                AddDependencyInput {
                    expected_child_lock_version: parent.lock_version,
                    target_child_status: "todo".to_owned(),
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_cycle".to_owned(),
                    recompute_event_id: "e_dependency_cycle_recompute".to_owned(),
                    now: 1_000,
                },
            )
            .await
            .expect_err("cycle must be rejected");
        assert!(matches!(cycle, StoreError::DependencyCycle(message) if message.contains("cycle")));
        let edge_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_dependencies WHERE board_id = 'b_default'",
                    (),
                )
                .await
                .expect("edge count"),
        )
        .await
        .expect("edge count row");
        assert_eq!(
            integer_value(edge_count.get_value(0).expect("edge count"), "edge.count")
                .expect("edge count integer"),
            1
        );

        let unknown = store
            .list_dependencies("t_dependency_unknown")
            .await
            .expect_err("unknown task must fail");
        assert!(matches!(unknown, StoreError::TaskNotFound(id) if id == "t_dependency_unknown"));
    }

    #[tokio::test]
    async fn dependency_create_enforces_board_and_running_guards_and_demotes_ready_children() {
        let (_directory, store, _path) = store("dependency-create-guards").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_dependency_other', 'dependency-other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let parent = store
            .create_task(
                "default",
                create_input("t_dependency_guard_parent", None, "Guard parent"),
            )
            .await
            .expect("create parent");
        let ready_child = store
            .create_task(
                "default",
                create_input("t_dependency_guard_ready", None, "Ready child"),
            )
            .await
            .expect("create ready child");
        connection
            .execute(
                "UPDATE tasks SET status = 'ready' WHERE id = ?1",
                [ready_child.id.as_str()],
            )
            .await
            .expect("make child ready");
        let ready_child = store
            .get_task_global(&ready_child.id)
            .await
            .expect("read ready child");
        let demoted = store
            .add_dependency(
                &ready_child.id,
                &parent.id,
                AddDependencyInput {
                    expected_child_lock_version: ready_child.lock_version,
                    target_child_status: "todo".to_owned(),
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_demoted".to_owned(),
                    recompute_event_id: "e_dependency_demoted_recompute".to_owned(),
                    now: 500,
                },
            )
            .await
            .expect("ready child should be demoted");
        assert!(demoted.added);
        assert_eq!(demoted.dependencies.task.status, "todo");
        assert_eq!(
            demoted.dependencies.task.lock_version,
            ready_child.lock_version + 1
        );
        let recompute_events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.recomputed'",
                    [ready_child.id.as_str()],
                )
                .await
                .expect("demotion recompute events"),
        )
        .await
        .expect("demotion recompute events row");
        assert_eq!(
            integer_value(
                recompute_events.get_value(0).expect("recompute count"),
                "event.count",
            )
            .expect("recompute count integer"),
            1
        );
        let dependency_events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'dependency.added'",
                    [ready_child.id.as_str()],
                )
                .await
                .expect("demotion dependency events"),
        )
        .await
        .expect("demotion dependency events row");
        assert_eq!(
            integer_value(
                dependency_events.get_value(0).expect("dependency count"),
                "event.count",
            )
            .expect("dependency count integer"),
            1
        );

        let running_child = store
            .create_task(
                "default",
                create_input("t_dependency_guard_running", None, "Running child"),
            )
            .await
            .expect("create running child");
        connection
            .execute(
                "UPDATE tasks SET status = 'running', claim_token = 'token-running', claim_owner = 'tester', claim_expires_at = 999999 WHERE id = ?1",
                [running_child.id.as_str()],
            )
            .await
            .expect("make child running");
        let running_error = store
            .add_dependency(
                &running_child.id,
                &parent.id,
                AddDependencyInput {
                    expected_child_lock_version: running_child.lock_version,
                    target_child_status: "running".to_owned(),
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_running".to_owned(),
                    recompute_event_id: "e_dependency_running_recompute".to_owned(),
                    now: 600,
                },
            )
            .await
            .expect_err("running child cannot receive unfinished parent");
        assert!(matches!(
            running_error,
            StoreError::InvalidTransition(message) if message.contains("running")
        ));

        let other_parent = store
            .create_task(
                "dependency-other",
                create_input("t_dependency_other_parent", None, "Other parent"),
            )
            .await
            .expect("create other parent");
        let cross_board = store
            .add_dependency(
                &ready_child.id,
                &other_parent.id,
                AddDependencyInput {
                    expected_child_lock_version: ready_child.lock_version + 1,
                    target_child_status: "todo".to_owned(),
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_cross_board".to_owned(),
                    recompute_event_id: "e_dependency_cross_board_recompute".to_owned(),
                    now: 700,
                },
            )
            .await
            .expect_err("cross-board dependency must be rejected");
        assert!(matches!(
            cross_board,
            StoreError::InvalidInput(message) if message.contains("cross-board")
        ));
    }

    #[tokio::test]
    async fn dependency_remove_is_atomic_idempotent_and_preserves_task_state() {
        let (_directory, store, _path) = store("dependency-remove").await;
        store.initialize().await.expect("initialize");
        let parent = store
            .create_task(
                "default",
                create_input("t_dependency_remove_parent", None, "Remove parent"),
            )
            .await
            .expect("create parent");
        let child = store
            .create_task(
                "default",
                create_input("t_dependency_remove_child", None, "Remove child"),
            )
            .await
            .expect("create child");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE tasks SET status = 'done', completed_at = 400 WHERE id = ?1",
                [parent.id.as_str()],
            )
            .await
            .expect("finish parent");
        store
            .add_dependency(
                &child.id,
                &parent.id,
                AddDependencyInput {
                    expected_child_lock_version: child.lock_version,
                    target_child_status: "todo".to_owned(),
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_remove_add".to_owned(),
                    recompute_event_id: "e_dependency_remove_recompute".to_owned(),
                    now: 500,
                },
            )
            .await
            .expect("add dependency");
        let before = store
            .get_task_global(&child.id)
            .await
            .expect("read child before remove");
        let removed = store
            .remove_dependency(
                &child.id,
                &parent.id,
                RemoveDependencyInput {
                    actor: " remover ".to_owned(),
                    event_id: "e_dependency_removed".to_owned(),
                    now: 600,
                },
            )
            .await
            .expect("remove dependency");
        assert!(removed.removed);
        assert!(removed.dependencies.parents.is_empty());
        assert!(removed.dependencies.edges.is_empty());
        let after = store
            .get_task_global(&child.id)
            .await
            .expect("read child after remove");
        assert_eq!(after.status, before.status);
        assert_eq!(after.lock_version, before.lock_version);
        let removed_events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'dependency.removed'",
                    [child.id.as_str()],
                )
                .await
                .expect("removed event count"),
        )
        .await
        .expect("removed event count row");
        assert_eq!(
            integer_value(
                removed_events.get_value(0).expect("removed count"),
                "event.count"
            )
            .expect("removed count integer"),
            1
        );
        let event = first_row(
            connection
                .query(
                    "SELECT actor, payload_json FROM task_events WHERE event_id = ?1",
                    ["e_dependency_removed"],
                )
                .await
                .expect("removed event query"),
        )
        .await
        .expect("removed event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "remover"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event payload"), "event.payload")
                .expect("event payload text"),
            format!(r#"{{"parent_task_id":"{}"}}"#, parent.id)
        );

        let replay = store
            .remove_dependency(
                &child.id,
                &parent.id,
                RemoveDependencyInput {
                    actor: "replay".to_owned(),
                    event_id: "e_dependency_removed_replay".to_owned(),
                    now: 700,
                },
            )
            .await
            .expect("missing edge replay");
        assert!(!replay.removed);
        assert_eq!(replay.dependencies, removed.dependencies);
        let replay_events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'dependency.removed'",
                    [child.id.as_str()],
                )
                .await
                .expect("replay event count"),
        )
        .await
        .expect("replay event count row");
        assert_eq!(
            integer_value(
                replay_events.get_value(0).expect("replay count"),
                "event.count"
            )
            .expect("replay count integer"),
            1
        );

        let second_parent = store
            .create_task(
                "default",
                create_input("t_dependency_remove_parent_two", None, "Remove parent two"),
            )
            .await
            .expect("create second parent");
        store
            .add_dependency(
                &child.id,
                &second_parent.id,
                AddDependencyInput {
                    expected_child_lock_version: after.lock_version,
                    target_child_status: "todo".to_owned(),
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_remove_add_two".to_owned(),
                    recompute_event_id: "e_dependency_remove_recompute_two".to_owned(),
                    now: 800,
                },
            )
            .await
            .expect("add second dependency");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', ?2, NULL, 'test.event', 'tester', '{}', 900)",
                ("e_dependency_remove_conflict", child.id.as_str()),
            )
            .await
            .expect("insert conflicting event");
        let conflict = store
            .remove_dependency(
                &child.id,
                &second_parent.id,
                RemoveDependencyInput {
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_remove_conflict".to_owned(),
                    now: 1_000,
                },
            )
            .await
            .expect_err("event conflict must roll back edge deletion");
        assert!(matches!(conflict, StoreError::Turso(_)));
        let remaining = store
            .list_dependencies(&child.id)
            .await
            .expect("list after rollback");
        assert_eq!(remaining.parents.len(), 1);
        assert_eq!(remaining.parents[0].id, second_parent.id);
        assert_eq!(
            store
                .get_task_global(&child.id)
                .await
                .expect("read child after rollback")
                .lock_version,
            after.lock_version
        );

        let unknown = store
            .remove_dependency(
                "t_dependency_remove_unknown",
                &parent.id,
                RemoveDependencyInput {
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_remove_unknown".to_owned(),
                    now: 1_100,
                },
            )
            .await
            .expect_err("unknown child must fail");
        assert!(
            matches!(unknown, StoreError::TaskNotFound(id) if id == "t_dependency_remove_unknown")
        );

        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_dependency_remove_other', 'dependency-remove-other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let other_parent = store
            .create_task(
                "dependency-remove-other",
                create_input("t_dependency_remove_other_parent", None, "Other parent"),
            )
            .await
            .expect("create other-board parent");
        let cross_board = store
            .remove_dependency(
                &child.id,
                &other_parent.id,
                RemoveDependencyInput {
                    actor: "tester".to_owned(),
                    event_id: "e_dependency_remove_cross_board".to_owned(),
                    now: 1_200,
                },
            )
            .await
            .expect_err("cross-board removal must fail");
        assert!(matches!(
            cross_board,
            StoreError::InvalidInput(message) if message.contains("cross-board")
        ));
    }

    #[tokio::test]
    async fn idempotency_and_board_column_constraints_are_enforced() {
        let (_directory, store, _path) = store("constraints").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");

        connection
            .execute(
                "INSERT INTO tasks(id, board_id, seq, idempotency_key, title, status, created_by, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 'todo', ?6, ?7, ?7)",
                ("t_one", "b_default", 1_i64, "client-1", "One", "test", 1_i64),
            )
            .await
            .expect("insert first task");
        let duplicate_idempotency = connection
            .execute(
                "INSERT INTO tasks(id, board_id, seq, idempotency_key, title, status, created_by, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 'todo', ?6, ?7, ?7)",
                ("t_two", "b_default", 2_i64, "client-1", "Two", "test", 2_i64),
            )
            .await;
        assert!(
            duplicate_idempotency.is_err(),
            "task idempotency must be unique per board"
        );

        let duplicate_column = connection
            .execute(
                "INSERT INTO board_columns(id, board_id, status, title, position, hidden, created_at, updated_at) VALUES (?1, ?2, 'todo', 'Duplicate', ?3, 0, ?4, ?4)",
                ("col_duplicate", "b_default", 200_i64, 2_i64),
            )
            .await;
        assert!(
            duplicate_column.is_err(),
            "board status columns must be unique"
        );
    }
}
