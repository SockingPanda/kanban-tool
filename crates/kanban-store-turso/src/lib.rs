mod schema;

use std::{
    error::Error,
    fmt::{Display, Formatter},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use turso::{Builder, Connection, Database, Row, Rows, Value, transaction::TransactionBehavior};

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
    InvalidStoredValue {
        field: &'static str,
    },
    BoardNotFound(String),
    TaskNotFound(String),
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
            Self::InvalidStoredValue { field } => {
                write!(formatter, "invalid stored value for {field}")
            }
            Self::BoardNotFound(selector) => write!(formatter, "board not found: {selector}"),
            Self::TaskNotFound(task_id) => write!(formatter, "task not found: {task_id}"),
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
