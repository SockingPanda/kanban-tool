use std::{
    fs::File,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::Duration,
};

use kanban_core::{
    Clock, KanbanError, Result, SystemClock, TaskStatus, new_event_id, new_run_id, new_task_id,
    new_typed_id,
};
use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter, types::Value};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::connect_file;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub board_id: String,
    pub seq: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: i64,
    pub event_id: String,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub kind: String,
    pub actor: Option<String>,
    pub payload_json: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: String,
    pub task_id: String,
    pub status: String,
    pub worker_profile: Option<String>,
    pub worker_pid: Option<i64>,
    pub claim_token: String,
    pub claim_owner: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub exit_code: Option<i64>,
    pub summary: Option<String>,
    pub error: Option<String>,
    pub log_path: Option<String>,
    pub metadata_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardRecord {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardColumnRecord {
    pub id: String,
    pub board_id: String,
    pub status: TaskStatus,
    pub title: String,
    pub position: i64,
    pub hidden: bool,
    pub wip_limit: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTask {
    pub title: String,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub assignee: Option<String>,
    pub priority: i64,
    pub scheduled_at: Option<i64>,
    pub due_at: Option<i64>,
    pub metadata_json: String,
}

impl CreateTask {
    pub fn ready(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: Some("ready spec".to_owned()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub assignee: Option<Option<String>>,
    pub priority: Option<i64>,
    pub scheduled_at: Option<Option<i64>>,
    pub due_at: Option<Option<i64>>,
    pub metadata_json: Option<String>,
    pub expected_lock_version: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimResult {
    pub task: TaskRecord,
    pub claim_token: String,
    pub run_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishPolicy {
    Done,
    Review,
    Blocked,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchOptions {
    pub actor: String,
    pub command: String,
    pub worker_profile: String,
    pub claim_ttl_ms: i64,
    pub heartbeat_interval_ms: i64,
    pub on_success: FinishPolicy,
    pub on_failure: FinishPolicy,
    pub log_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchResult {
    pub claimed: usize,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskListSort {
    Position,
    PositionDesc,
    Priority,
    PriorityDesc,
    CreatedAt,
    CreatedAtDesc,
    UpdatedAt,
    UpdatedAtDesc,
    DueAt,
    DueAtDesc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListOptions {
    pub statuses: Vec<TaskStatus>,
    pub include_archived: bool,
    pub assignee: Option<String>,
    pub search: Option<String>,
    pub sort: TaskListSort,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListPage {
    pub tasks: Vec<TaskRecord>,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventListOptions {
    pub task_ref: Option<String>,
    pub after: i64,
    pub limit: usize,
}

pub fn list_boards(path: impl AsRef<Path>) -> Result<Vec<BoardRecord>> {
    let conn = connect_file(path.as_ref())?;
    let mut stmt = conn
        .prepare(
            "SELECT id,slug,name,description,created_at,updated_at,archived_at \
             FROM boards WHERE archived_at IS NULL ORDER BY created_at ASC, slug ASC",
        )
        .map_err(storage)?;
    let rows = stmt.query_map([], board_from_row).map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

pub fn get_board(path: impl AsRef<Path>, slug_or_id: &str) -> Result<BoardRecord> {
    let conn = connect_file(path.as_ref())?;
    get_board_conn(&conn, slug_or_id)
}

pub fn list_board_columns(
    path: impl AsRef<Path>,
    board_slug_or_id: &str,
) -> Result<Vec<BoardColumnRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board_slug_or_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT id,board_id,status,title,position,hidden,wip_limit,created_at,updated_at \
             FROM board_columns WHERE board_id=?1 ORDER BY position ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map([board_id], board_column_from_row)
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

pub fn create_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    input: CreateTask,
) -> Result<TaskRecord> {
    create_task_with_dependencies(path, board, actor, input, &[])
}

pub fn create_task_with_dependencies(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    input: CreateTask,
    depends_on: &[String],
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let title = input.title.trim().to_owned();
    if title.is_empty() {
        return Err(KanbanError::InvalidInput("title is required".into()));
    }
    if !json_valid(&conn, &input.metadata_json)? {
        return Err(KanbanError::InvalidInput(
            "metadata_json must be valid JSON".into(),
        ));
    }
    let status = initial_status(
        input.status,
        input.description.as_deref(),
        input.scheduled_at,
        now,
    )?;
    let id = new_task_id();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let seq: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(seq), 0) + 1 FROM tasks WHERE board_id=?1",
                [&board_id],
                |r| r.get(0),
            )
            .map_err(storage)?;
        conn.execute(
        "INSERT INTO tasks(id, board_id, seq, title, description, status, assignee, priority, position, scheduled_at, due_at, created_by, created_at, updated_at, metadata_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?3 * 1024, ?9, ?10, ?11, ?12, ?12, ?13)",
        params![id, board_id, seq, title, input.description, status.as_str(), input.assignee, input.priority, input.scheduled_at, input.due_at, actor, now, input.metadata_json],
        ).map_err(storage)?;
        let payload = json!({ "status": status.as_str() }).to_string();
        insert_event(
            &conn,
            &board_id,
            Some(&id),
            None,
            "task.created",
            actor,
            &payload,
            now,
        )?;
        for parent_ref in depends_on {
            let parent = resolve_task(&conn, &board_id, parent_ref)?;
            let child = get_task_by_id(&conn, &board_id, &id)?;
            add_dependency_in_current_tx(&conn, &board_id, actor, &parent, &child, now)?;
        }
        get_task_by_id(&conn, &board_id, &id)
    })
}

pub fn update_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    patch: TaskPatch,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    with_immediate_tx(&conn, || {
        let mut task = resolve_task(&conn, &board_id, task_ref)?;
        let scheduled_at_changed = patch.scheduled_at.is_some();
        if patch
            .expected_lock_version
            .is_some_and(|expected| task.lock_version != expected)
        {
            return Err(KanbanError::InvalidInput("lock_version mismatch".into()));
        }
        if let Some(title) = patch.title {
            if title.trim().is_empty() {
                return Err(KanbanError::InvalidInput("title is required".into()));
            }
            task.title = title;
        }
        if let Some(description) = patch.description {
            task.description = description;
        }
        if let Some(assignee) = patch.assignee {
            task.assignee = assignee;
        }
        if let Some(priority) = patch.priority {
            task.priority = priority;
        }
        if let Some(scheduled_at) = patch.scheduled_at {
            task.scheduled_at = scheduled_at;
        }
        if let Some(due_at) = patch.due_at {
            task.due_at = due_at;
        }
        if let Some(metadata_json) = patch.metadata_json {
            if !json_valid(&conn, &metadata_json)? {
                return Err(KanbanError::InvalidInput(
                    "metadata_json must be valid JSON".into(),
                ));
            }
            task.metadata_json = metadata_json;
        }
        if scheduled_at_changed
            && matches!(
                task.status,
                TaskStatus::Triage | TaskStatus::Todo | TaskStatus::Scheduled | TaskStatus::Ready
            )
        {
            task.status = recompute_ready_status(&conn, &task, now)?;
        }
        let changed = conn.execute(
        "UPDATE tasks SET title=?1, description=?2, status=?3, assignee=?4, priority=?5, scheduled_at=?6, due_at=?7, metadata_json=?8, updated_at=?9, lock_version=lock_version+1 WHERE id=?10 AND board_id=?11",
        params![task.title, task.description, task.status.as_str(), task.assignee, task.priority, task.scheduled_at, task.due_at, task.metadata_json, now, task.id, board_id],
        ).map_err(storage)?;
        if changed != 1 {
            return Err(KanbanError::InvalidTransition("task update failed".into()));
        }
        insert_event(
            &conn,
            &board_id,
            Some(&task.id),
            None,
            "task.updated",
            actor,
            "{}",
            now,
        )?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn specify_task(
    path: impl AsRef<Path>,
    actor: &str,
    task_id: &str,
    description: Option<String>,
    scheduled_at: Option<i64>,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id_for_task(&conn, task_id)?;
    let mut task = get_task_by_id(&conn, &board_id, task_id)?;
    if task.status != TaskStatus::Triage {
        return Err(KanbanError::InvalidTransition(format!(
            "cannot specify from {}",
            task.status.as_str()
        )));
    }
    if let Some(description) = description {
        task.description = Some(description);
    }
    if let Some(scheduled_at) = scheduled_at {
        task.scheduled_at = Some(scheduled_at);
    }
    if matches!(
        task.status,
        TaskStatus::Triage | TaskStatus::Todo | TaskStatus::Scheduled | TaskStatus::Ready
    ) {
        task.status = recompute_ready_status(&conn, &task, now)?;
    }
    with_immediate_tx(&conn, || {
        conn.execute(
            "UPDATE tasks SET description=?1, scheduled_at=?2, status=?3, updated_at=?4, lock_version=lock_version+1 WHERE id=?5 AND board_id=?6",
            params![task.description, task.scheduled_at, task.status.as_str(), now, task.id, board_id],
        )
        .map_err(storage)?;
        insert_event(
            &conn,
            &board_id,
            Some(&task.id),
            None,
            "task.specified",
            actor,
            &json!({ "to_status": task.status.as_str() }).to_string(),
            now,
        )?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn list_tasks(
    path: impl AsRef<Path>,
    board: &str,
    statuses: &[TaskStatus],
    include_archived: bool,
) -> Result<Vec<TaskRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let mut tasks = query_tasks(&conn, &board_id)?;
    if !include_archived {
        tasks.retain(|t| t.status != TaskStatus::Archived);
    }
    if !statuses.is_empty() {
        tasks.retain(|t| statuses.contains(&t.status));
    }
    Ok(tasks)
}

pub fn list_tasks_page(
    path: impl AsRef<Path>,
    board: &str,
    options: TaskListOptions,
) -> Result<TaskListPage> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let (where_sql, params) = task_query_where(&board_id, &options);
    let total_sql = format!("SELECT COUNT(*) FROM tasks {where_sql}");
    let total: i64 = conn
        .query_row(&total_sql, params_from_iter(params.iter()), |row| {
            row.get(0)
        })
        .map_err(storage)?;

    let mut page_params = params;
    page_params.push(Value::Integer(options.limit as i64));
    page_params.push(Value::Integer(options.offset as i64));
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks {where_sql} ORDER BY {} LIMIT ? OFFSET ?",
        task_order_by(options.sort)
    );
    let mut stmt = conn.prepare(&sql).map_err(storage)?;
    let rows = stmt
        .query_map(params_from_iter(page_params.iter()), task_from_row)
        .map_err(storage)?;
    let tasks = rows
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    Ok(TaskListPage {
        tasks,
        total: total as usize,
    })
}

pub fn get_task(path: impl AsRef<Path>, board: &str, task_ref: &str) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    resolve_task(&conn, &board_id, task_ref)
}

pub fn get_task_by_id_global(path: impl AsRef<Path>, task_id: &str) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id_for_task(&conn, task_id)?;
    get_task_by_id(&conn, &board_id, task_id)
}

pub fn update_task_by_id(
    path: impl AsRef<Path>,
    actor: &str,
    task_id: &str,
    patch: TaskPatch,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id_for_task(&conn, task_id)?;
    drop(conn);
    update_task(path, &board_id, actor, task_id, patch)
}

pub fn promote_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    ensure_dependencies_done(&conn, &task.id)?;
    if task.status == TaskStatus::Scheduled && task.scheduled_at.is_some_and(|t| t > now) {
        return Err(KanbanError::InvalidTransition(
            "scheduled_at is in the future".into(),
        ));
    }
    if !matches!(task.status, TaskStatus::Todo | TaskStatus::Scheduled) {
        return Err(KanbanError::InvalidTransition(format!(
            "cannot promote from {}",
            task.status.as_str()
        )));
    }
    with_immediate_tx(&conn, || {
        set_status(
            &conn,
            &board_id,
            &task.id,
            TaskStatus::Ready,
            actor,
            "task.promoted",
            now,
        )?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn claim_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    ttl_ms: i64,
) -> Result<ClaimResult> {
    claim_task_with_profile(path, board, actor, task_ref, ttl_ms, "manual")
}

pub fn claim_task_with_profile(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    ttl_ms: i64,
    worker_profile: &str,
) -> Result<ClaimResult> {
    claim_task_with_profile_and_metadata(path, board, actor, task_ref, ttl_ms, worker_profile, "{}")
}

pub fn claim_task_with_profile_and_metadata(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    ttl_ms: i64,
    worker_profile: &str,
    metadata_json: &str,
) -> Result<ClaimResult> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    if !json_valid(&conn, metadata_json)? {
        return Err(KanbanError::InvalidInput(
            "metadata_json must be valid JSON".into(),
        ));
    }
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    claim_task_conn(
        &conn,
        &board_id,
        actor,
        &task.id,
        ttl_ms,
        worker_profile,
        metadata_json,
        now,
    )
}

#[allow(clippy::too_many_arguments)]
fn claim_task_conn(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    task_id: &str,
    ttl_ms: i64,
    profile: &str,
    metadata_json: &str,
    now: i64,
) -> Result<ClaimResult> {
    conn.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
    match claim_task_in_current_tx(
        conn,
        board_id,
        actor,
        task_id,
        ttl_ms,
        profile,
        metadata_json,
        now,
    ) {
        Ok(claim) => {
            conn.execute_batch("COMMIT").map_err(storage)?;
            Ok(claim)
        }
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

fn claim_next_ready_conn(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    worker_profile: &str,
    ttl_ms: i64,
    now: i64,
) -> Result<Option<ClaimResult>> {
    conn.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
    let selected = conn
        .query_row(
            "SELECT id FROM tasks WHERE board_id=?1 AND status='ready' AND claim_token IS NULL AND (assignee IS NULL OR assignee=?2) AND NOT EXISTS (SELECT 1 FROM task_dependencies d JOIN tasks p ON p.id=d.parent_task_id WHERE d.child_task_id=tasks.id AND p.status != 'done') ORDER BY priority DESC, created_at ASC LIMIT 1",
            params![board_id, worker_profile],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage);
    let result = match selected {
        Ok(Some(task_id)) => claim_task_in_current_tx(
            conn,
            board_id,
            actor,
            &task_id,
            ttl_ms,
            worker_profile,
            "{}",
            now,
        )
        .map(Some),
        Ok(None) => Ok(None),
        Err(err) => Err(err),
    };
    match result {
        Ok(claim) => {
            conn.execute_batch("COMMIT").map_err(storage)?;
            Ok(claim)
        }
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn claim_task_in_current_tx(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    task_id: &str,
    ttl_ms: i64,
    profile: &str,
    metadata_json: &str,
    now: i64,
) -> Result<ClaimResult> {
    let task = get_task_by_id(conn, board_id, task_id)?;
    if task.status != TaskStatus::Ready || task.claim_token.is_some() {
        return Err(KanbanError::InvalidTransition(
            "task is not claimable".into(),
        ));
    }
    ensure_dependencies_done(conn, task_id)?;
    let token = new_typed_id("claim");
    let run_id = new_run_id();
    let expires = now + ttl_ms;
    let changed = conn.execute(
        "UPDATE tasks SET status='running', claim_token=?1, claim_owner=?2, claim_expires_at=?3, last_heartbeat_at=?4, started_at=COALESCE(started_at, ?4), updated_at=?4, lock_version=lock_version+1 WHERE id=?5 AND board_id=?6 AND status='ready' AND claim_token IS NULL AND NOT EXISTS (SELECT 1 FROM task_dependencies d JOIN tasks p ON p.id=d.parent_task_id WHERE d.child_task_id=tasks.id AND p.status != 'done')",
        params![token, actor, expires, now, task_id, board_id],
    ).map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition("claim conflict".into()));
    }
    conn.execute(
        "INSERT INTO task_runs(id, board_id, task_id, status, worker_profile, claim_token, claim_owner, claim_expires_at, started_at, last_heartbeat_at, metadata_json) VALUES (?1, ?2, ?3, 'running', ?4, ?5, ?6, ?7, ?8, ?8, ?9)",
        params![run_id, board_id, task_id, profile, token, actor, expires, now, metadata_json],
    ).map_err(storage)?;
    conn.execute(
        "UPDATE tasks SET current_run_id=?1 WHERE id=?2",
        params![run_id, task_id],
    )
    .map_err(storage)?;
    insert_event(
        conn,
        board_id,
        Some(task_id),
        Some(&run_id),
        "task.claimed",
        actor,
        &json!({
            "claim_owner": actor,
            "metadata": serde_json::from_str::<serde_json::Value>(metadata_json)
                .unwrap_or_else(|_| json!({})),
        })
        .to_string(),
        now,
    )?;
    Ok(ClaimResult {
        task: get_task_by_id(conn, board_id, task_id)?,
        claim_token: token,
        run_id,
    })
}

pub fn heartbeat_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: &str,
    ttl_ms: i64,
) -> Result<TaskRecord> {
    heartbeat_task_with_note(path, board, actor, task_ref, token, ttl_ms, None)
}

pub fn heartbeat_task_with_note(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: &str,
    ttl_ms: i64,
    note: Option<&str>,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    with_immediate_tx(&conn, || {
        heartbeat_task_conn(&conn, &board_id, actor, &task, token, ttl_ms, note, now)?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

#[allow(clippy::too_many_arguments)]
fn heartbeat_task_conn(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    task: &TaskRecord,
    token: &str,
    ttl_ms: i64,
    note: Option<&str>,
    now: i64,
) -> Result<()> {
    if task.status != TaskStatus::Running || task.claim_token.as_deref() != Some(token) {
        return Err(KanbanError::InvalidTransition(
            "heartbeat requires matching running claim".into(),
        ));
    }
    let expires = now + ttl_ms;
    let changed = conn
        .execute(
            "UPDATE tasks SET claim_expires_at=?1, last_heartbeat_at=?2, updated_at=?2, lock_version=lock_version+1 WHERE id=?3 AND board_id=?4 AND status='running' AND claim_token=?5 AND current_run_id IS ?6",
            params![expires, now, task.id, board_id, token, task.current_run_id],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition(
            "heartbeat requires matching running claim".into(),
        ));
    }
    if let Some(run_id) = &task.current_run_id {
        let changed = conn
            .execute(
                "UPDATE task_runs SET claim_expires_at=?1, last_heartbeat_at=?2 WHERE id=?3 AND board_id=?4 AND task_id=?5 AND status='running' AND claim_token=?6",
                params![expires, now, run_id, board_id, task.id, token],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(KanbanError::InvalidTransition(
                "heartbeat requires matching running run".into(),
            ));
        }
        insert_event(
            conn,
            board_id,
            Some(&task.id),
            Some(run_id),
            "task.heartbeat",
            actor,
            &json!({ "note": note }).to_string(),
            now,
        )?;
    }
    Ok(())
}

pub fn complete_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: Option<&str>,
    force: bool,
) -> Result<TaskRecord> {
    complete_task_with_summary_and_result(path, board, actor, task_ref, token, force, None, None)
}

pub fn complete_task_with_summary(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: Option<&str>,
    force: bool,
    summary: Option<&str>,
) -> Result<TaskRecord> {
    complete_task_with_summary_and_result(path, board, actor, task_ref, token, force, summary, None)
}

#[allow(clippy::too_many_arguments)]
pub fn complete_task_with_summary_and_result(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: Option<&str>,
    force: bool,
    summary: Option<&str>,
    result_json: Option<&str>,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    let result_json_is_invalid = match result_json {
        Some(value) => !json_valid(&conn, value)?,
        None => false,
    };
    if result_json_is_invalid {
        return Err(KanbanError::InvalidInput(
            "result_json must be valid JSON".into(),
        ));
    }
    if task.status == TaskStatus::Running && !force && task.claim_token.as_deref() != token {
        return Err(KanbanError::InvalidTransition(
            "claim token mismatch".into(),
        ));
    }
    if !matches!(task.status, TaskStatus::Running | TaskStatus::Review) {
        return Err(KanbanError::InvalidTransition(
            "complete requires running or review".into(),
        ));
    }
    with_immediate_tx(&conn, || {
        finish_running(
            &conn,
            &board_id,
            &task,
            TaskStatus::Done,
            actor,
            "task.completed",
            "succeeded",
            0,
            None,
            None,
            summary,
            result_json,
            now,
        )?;
        promote_children(&conn, &board_id, actor, &task.id, now)?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn submit_review_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: Option<&str>,
    force: bool,
) -> Result<TaskRecord> {
    submit_review_task_with_summary(path, board, actor, task_ref, token, force, None)
}

pub fn submit_review_task_with_summary(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: Option<&str>,
    force: bool,
    summary: Option<&str>,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    if task.status != TaskStatus::Running {
        return Err(KanbanError::InvalidTransition(
            "review requires running".into(),
        ));
    }
    if !force && task.claim_token.as_deref() != token {
        return Err(KanbanError::InvalidTransition(
            "claim token mismatch".into(),
        ));
    }
    with_immediate_tx(&conn, || {
        finish_running(
            &conn,
            &board_id,
            &task,
            TaskStatus::Review,
            actor,
            "task.submitted_for_review",
            "succeeded",
            0,
            None,
            None,
            summary,
            None,
            now,
        )?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn block_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    reason: &str,
    token: Option<&str>,
    force: bool,
) -> Result<TaskRecord> {
    let mut conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    if reason.trim().is_empty() {
        return Err(KanbanError::InvalidInput("block reason is required".into()));
    }
    if task.status == TaskStatus::Running && !force && task.claim_token.as_deref() != token {
        return Err(KanbanError::InvalidTransition(
            "claim token mismatch".into(),
        ));
    }
    if !matches!(
        task.status,
        TaskStatus::Triage
            | TaskStatus::Todo
            | TaskStatus::Scheduled
            | TaskStatus::Ready
            | TaskStatus::Running
            | TaskStatus::Review
    ) {
        return Err(KanbanError::InvalidTransition("cannot block task".into()));
    }
    if task.status == TaskStatus::Running {
        let tx = conn.transaction().map_err(storage)?;
        finish_running(
            &tx,
            &board_id,
            &task,
            TaskStatus::Blocked,
            actor,
            "task.blocked",
            "failed",
            1,
            Some(reason),
            None,
            None,
            None,
            now,
        )?;
        tx.commit().map_err(storage)?;
    } else {
        let tx = conn.transaction().map_err(storage)?;
        tx.execute("UPDATE tasks SET status='blocked', status_reason=?1, updated_at=?2, lock_version=lock_version+1 WHERE id=?3", params![reason, now, task.id]).map_err(storage)?;
        let payload = json!({ "reason": reason }).to_string();
        insert_event(
            &tx,
            &board_id,
            Some(&task.id),
            None,
            "task.blocked",
            actor,
            &payload,
            now,
        )?;
        tx.commit().map_err(storage)?;
    }
    get_task_by_id(&conn, &board_id, &task.id)
}

pub fn unblock_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    if task.status != TaskStatus::Blocked {
        return Err(KanbanError::InvalidTransition(
            "unblock requires blocked".into(),
        ));
    }
    let target = recompute_ready_status(&conn, &task, now)?;
    with_immediate_tx(&conn, || {
        conn.execute(
            "UPDATE tasks SET status=?1, status_reason=NULL, updated_at=?2, lock_version=lock_version+1 WHERE id=?3",
            params![target.as_str(), now, task.id],
        )
        .map_err(storage)?;
        let payload = json!({ "to_status": target.as_str() }).to_string();
        insert_event(
            &conn,
            &board_id,
            Some(&task.id),
            None,
            "task.unblocked",
            actor,
            &payload,
            now,
        )?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn reclaim_expired(path: impl AsRef<Path>, board: &str, actor: &str) -> Result<usize> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let expired: Vec<TaskRecord> = query_tasks(&conn, &board_id)?
        .into_iter()
        .filter(|t| t.status == TaskStatus::Running && t.claim_expires_at.is_some_and(|x| x <= now))
        .collect();
    let mut count = 0;
    for task in expired {
        let reclaimed = with_immediate_tx(&conn, || {
            let fresh = get_task_by_id(&conn, &board_id, &task.id)?;
            let tx_now = SystemClock.now_ms();
            if fresh.status != TaskStatus::Running
                || fresh
                    .claim_expires_at
                    .is_none_or(|expires| expires > tx_now)
            {
                return Ok(false);
            }
            retry_running_task(
                &conn,
                &board_id,
                &fresh,
                actor,
                "expired",
                None,
                "claim expired",
                tx_now,
                Some(tx_now),
            )?;
            Ok(true)
        })?;
        if reclaimed {
            count += 1;
        }
    }
    Ok(count)
}

pub fn reclaim_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    force: bool,
) -> Result<TaskRecord> {
    reclaim_task_to(path, board, actor, task_ref, force, TaskStatus::Ready, None)
}

pub fn reclaim_task_to(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    force: bool,
    to_status: TaskStatus,
    reason: Option<&str>,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    if !matches!(to_status, TaskStatus::Ready | TaskStatus::Blocked) {
        return Err(KanbanError::InvalidInput(
            "reclaim to_status must be ready or blocked".into(),
        ));
    }
    if to_status == TaskStatus::Blocked && reason.is_none_or(|value| value.trim().is_empty()) {
        return Err(KanbanError::InvalidInput(
            "reclaim reason is required when to_status is blocked".into(),
        ));
    }
    let task = resolve_task(&conn, &board_id, task_ref)?;
    with_immediate_tx(&conn, || {
        let fresh = get_task_by_id(&conn, &board_id, &task.id)?;
        let tx_now = SystemClock.now_ms();
        if fresh.status != TaskStatus::Running {
            return Err(KanbanError::InvalidTransition(
                "reclaim requires running".into(),
            ));
        }
        if !force
            && fresh
                .claim_expires_at
                .is_none_or(|expires| expires > tx_now)
        {
            return Err(KanbanError::InvalidTransition(
                "reclaim requires expired claim or force".into(),
            ));
        }
        let new_retry_count = fresh.retry_count + 1;
        let max_retries_reached = fresh
            .max_retries
            .is_some_and(|max_retries| new_retry_count >= max_retries);
        let effective_status = if max_retries_reached {
            TaskStatus::Blocked
        } else {
            to_status
        };
        let default_reason = if max_retries_reached {
            "max retries reached"
        } else if force {
            "force reclaimed"
        } else {
            "claim expired"
        };
        let effective_reason = reason.unwrap_or(default_reason);
        reclaim_running_task(
            &conn,
            &board_id,
            &fresh,
            actor,
            if force { "canceled" } else { "expired" },
            effective_reason,
            effective_status,
            tx_now,
            (!force).then_some(tx_now),
        )?;
        get_task_by_id(&conn, &board_id, &fresh.id)
    })
}

pub fn archive_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    force: bool,
) -> Result<TaskRecord> {
    let mut conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    if task.status == TaskStatus::Running && !force {
        return Err(KanbanError::InvalidTransition(
            "cannot archive running without force".into(),
        ));
    }
    let tx = conn.transaction().map_err(storage)?;
    if task.status == TaskStatus::Running {
        let run_id = task.current_run_id.as_deref().ok_or_else(|| {
            KanbanError::InvalidTransition("force archive requires active run".into())
        })?;
        let changed = tx
            .execute(
                "UPDATE task_runs SET status='canceled', finished_at=?1, error=COALESCE(error, ?2) WHERE id=?3 AND board_id=?4 AND task_id=?5 AND status='running'",
                params![now, "force archived", run_id, board_id, task.id],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(KanbanError::InvalidTransition(
                "force archive requires active running run".into(),
            ));
        }
    }
    tx.execute("UPDATE tasks SET status='archived', archived_at=?1, claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, updated_at=?1, lock_version=lock_version+1 WHERE id=?2", params![now, task.id]).map_err(storage)?;
    insert_event(
        &tx,
        &board_id,
        Some(&task.id),
        task.current_run_id.as_deref(),
        "task.archived",
        actor,
        "{}",
        now,
    )?;
    tx.commit().map_err(storage)?;
    get_task_by_id(&conn, &board_id, &task.id)
}

pub fn add_dependency(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    child_ref: &str,
) -> Result<()> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let parent = resolve_task(&conn, &board_id, parent_ref)?;
    let child = resolve_task(&conn, &board_id, child_ref)?;
    if parent.id == child.id {
        return Err(KanbanError::InvalidInput(
            "dependency cannot point to itself".into(),
        ));
    }
    if has_path(&conn, &child.id, &parent.id)? {
        return Err(KanbanError::InvalidInput(
            "dependency cycle detected".into(),
        ));
    }
    if child.status == TaskStatus::Running && parent.status != TaskStatus::Done {
        return Err(KanbanError::InvalidTransition(
            "cannot add incomplete dependency to running task".into(),
        ));
    }
    with_immediate_tx(&conn, || {
        add_dependency_in_current_tx(&conn, &board_id, actor, &parent, &child, now)
    })
}

fn add_dependency_in_current_tx(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    parent: &TaskRecord,
    child: &TaskRecord,
    now: i64,
) -> Result<()> {
    if parent.id == child.id {
        return Err(KanbanError::InvalidInput(
            "dependency cannot point to itself".into(),
        ));
    }
    if has_path(conn, &child.id, &parent.id)? {
        return Err(KanbanError::InvalidInput(
            "dependency cycle detected".into(),
        ));
    }
    if child.status == TaskStatus::Running && parent.status != TaskStatus::Done {
        return Err(KanbanError::InvalidTransition(
            "cannot add incomplete dependency to running task".into(),
        ));
    }
    conn.execute(
        "INSERT OR IGNORE INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![board_id, parent.id, child.id, now],
    )
    .map_err(storage)?;
    if child.status == TaskStatus::Ready && parent.status != TaskStatus::Done {
        conn.execute(
            "UPDATE tasks SET status='todo', updated_at=?1, lock_version=lock_version+1 WHERE id=?2",
            params![now, child.id],
        )
        .map_err(storage)?;
    }
    let payload = json!({ "parent_task_id": parent.id }).to_string();
    insert_event(
        conn,
        board_id,
        Some(&child.id),
        None,
        "dependency.added",
        actor,
        &payload,
        now,
    )
}

pub fn remove_dependency(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    child_ref: &str,
) -> Result<()> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let parent = resolve_task(&conn, &board_id, parent_ref)?;
    let child = resolve_task(&conn, &board_id, child_ref)?;
    with_immediate_tx(&conn, || {
        conn.execute(
            "DELETE FROM task_dependencies WHERE parent_task_id=?1 AND child_task_id=?2",
            params![parent.id, child.id],
        )
        .map_err(storage)?;
        let fresh_child = get_task_by_id(&conn, &board_id, &child.id)?;
        if matches!(
            fresh_child.status,
            TaskStatus::Triage | TaskStatus::Todo | TaskStatus::Scheduled | TaskStatus::Ready
        ) {
            let target = recompute_ready_status(&conn, &fresh_child, now)?;
            if target != fresh_child.status {
                set_status(
                    &conn,
                    &board_id,
                    &fresh_child.id,
                    target,
                    actor,
                    if target == TaskStatus::Ready {
                        "task.promoted"
                    } else {
                        "task.recomputed"
                    },
                    now,
                )?;
            }
        }
        let payload = json!({ "parent_task_id": parent.id }).to_string();
        insert_event(
            &conn,
            &board_id,
            Some(&child.id),
            None,
            "dependency.removed",
            actor,
            &payload,
            now,
        )
    })
}

pub fn list_dependencies(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: &str,
) -> Result<Vec<(String, String)>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    let mut stmt = conn.prepare("SELECT parent_task_id, child_task_id FROM task_dependencies WHERE parent_task_id=?1 OR child_task_id=?1 ORDER BY created_at ASC").map_err(storage)?;
    let rows = stmt
        .query_map([task.id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

pub fn list_events(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: Option<&str>,
) -> Result<Vec<EventRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task_id = task_ref
        .map(|r| resolve_task(&conn, &board_id, r).map(|t| t.id))
        .transpose()?;
    let sql = if task_id.is_some() {
        "SELECT id,event_id,task_id,run_id,kind,actor,payload_json,created_at FROM task_events WHERE board_id=?1 AND task_id=?2 ORDER BY id ASC"
    } else {
        "SELECT id,event_id,task_id,run_id,kind,actor,payload_json,created_at FROM task_events WHERE board_id=?1 ORDER BY id ASC"
    };
    let mut stmt = conn.prepare(sql).map_err(storage)?;
    let mut out = Vec::new();
    if let Some(task_id) = task_id {
        let rows = stmt
            .query_map(params![board_id, task_id], event_from_row)
            .map_err(storage)?;
        for row in rows {
            out.push(row.map_err(storage)?);
        }
    } else {
        let rows = stmt
            .query_map(params![board_id], event_from_row)
            .map_err(storage)?;
        for row in rows {
            out.push(row.map_err(storage)?);
        }
    }
    Ok(out)
}

pub fn list_events_after(
    path: impl AsRef<Path>,
    board: &str,
    options: EventListOptions,
) -> Result<Vec<EventRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task_id = options
        .task_ref
        .as_deref()
        .map(|r| resolve_task(&conn, &board_id, r).map(|t| t.id))
        .transpose()?;
    let mut params = vec![Value::Text(board_id), Value::Integer(options.after)];
    let mut where_sql = "WHERE board_id=? AND id>?".to_owned();
    if let Some(task_id) = task_id {
        where_sql.push_str(" AND task_id=?");
        params.push(Value::Text(task_id));
    }
    params.push(Value::Integer(options.limit as i64));
    let sql = format!(
        "SELECT id,event_id,task_id,run_id,kind,actor,payload_json,created_at FROM task_events {where_sql} ORDER BY id ASC LIMIT ?"
    );
    let mut stmt = conn.prepare(&sql).map_err(storage)?;
    let rows = stmt
        .query_map(params_from_iter(params.iter()), event_from_row)
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

pub fn list_runs(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: Option<&str>,
) -> Result<Vec<RunRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task_id = task_ref
        .map(|r| resolve_task(&conn, &board_id, r).map(|t| t.id))
        .transpose()?;
    let sql = if task_id.is_some() {
        "SELECT id,task_id,status,worker_profile,worker_pid,claim_token,claim_owner,started_at,finished_at,exit_code,summary,error,log_path,metadata_json FROM task_runs WHERE board_id=?1 AND task_id=?2 ORDER BY started_at DESC"
    } else {
        "SELECT id,task_id,status,worker_profile,worker_pid,claim_token,claim_owner,started_at,finished_at,exit_code,summary,error,log_path,metadata_json FROM task_runs WHERE board_id=?1 ORDER BY started_at DESC"
    };
    let mut stmt = conn.prepare(sql).map_err(storage)?;
    let mut out = Vec::new();
    if let Some(task_id) = task_id {
        let rows = stmt
            .query_map(params![board_id, task_id], run_from_row)
            .map_err(storage)?;
        for row in rows {
            out.push(row.map_err(storage)?);
        }
    } else {
        let rows = stmt
            .query_map(params![board_id], run_from_row)
            .map_err(storage)?;
        for row in rows {
            out.push(row.map_err(storage)?);
        }
    }
    Ok(out)
}

pub fn get_run_by_id_global(path: impl AsRef<Path>, run_id: &str) -> Result<RunRecord> {
    let conn = connect_file(path.as_ref())?;
    conn.query_row(
        "SELECT id,task_id,status,worker_profile,worker_pid,claim_token,claim_owner,started_at,finished_at,exit_code,summary,error,log_path,metadata_json FROM task_runs WHERE id=?1",
        [run_id],
        run_from_row,
    )
    .optional()
    .map_err(storage)?
    .ok_or_else(|| KanbanError::NotFound(format!("run {run_id}")))
}

pub fn dispatch_once(
    path: impl AsRef<Path>,
    board: &str,
    options: DispatchOptions,
) -> Result<DispatchResult> {
    validate_dispatch_options(&options)?;
    let path = path.as_ref();
    reclaim_expired(path, board, &options.actor)?;
    let conn = connect_file(path)?;
    let board_id = board_id(&conn, board)?;
    let now = SystemClock.now_ms();
    promote_due_tasks(&conn, &board_id, &options.actor, now)?;
    let Some(claim) = claim_next_ready_conn(
        &conn,
        &board_id,
        &options.actor,
        &options.worker_profile,
        options.claim_ttl_ms,
        now,
    )?
    else {
        return Ok(DispatchResult {
            claimed: 0,
            task_id: None,
            run_id: None,
            exit_code: None,
        });
    };
    std::fs::create_dir_all(&options.log_dir).map_err(|e| KanbanError::Storage(e.to_string()))?;
    let log_path = options.log_dir.join(format!("{}.log", claim.run_id));
    let output = run_worker_with_heartbeat(path, board, &options, &claim, &log_path)?;
    let exit = output.status.code().unwrap_or(1);
    let fresh = get_task_by_id(&conn, &board_id, &claim.task.id)?;
    let target = if output.status.success() {
        options.on_success
    } else {
        options.on_failure
    };
    with_immediate_tx(&conn, || {
        match target {
            FinishPolicy::Done => {
                finish_running(
                    &conn,
                    &board_id,
                    &fresh,
                    TaskStatus::Done,
                    &options.actor,
                    "task.completed",
                    "succeeded",
                    exit,
                    None,
                    Some(&log_path),
                    None,
                    None,
                    SystemClock.now_ms(),
                )?;
                promote_children(
                    &conn,
                    &board_id,
                    &options.actor,
                    &fresh.id,
                    SystemClock.now_ms(),
                )?;
            }
            FinishPolicy::Review => {
                finish_running(
                    &conn,
                    &board_id,
                    &fresh,
                    TaskStatus::Review,
                    &options.actor,
                    "task.submitted_for_review",
                    "succeeded",
                    exit,
                    None,
                    Some(&log_path),
                    None,
                    None,
                    SystemClock.now_ms(),
                )?;
            }
            FinishPolicy::Blocked => {
                finish_running(
                    &conn,
                    &board_id,
                    &fresh,
                    TaskStatus::Blocked,
                    &options.actor,
                    "task.blocked",
                    "failed",
                    exit,
                    Some("worker failed"),
                    Some(&log_path),
                    None,
                    None,
                    SystemClock.now_ms(),
                )?;
            }
            FinishPolicy::Ready => {
                retry_running_task(
                    &conn,
                    &board_id,
                    &fresh,
                    &options.actor,
                    "failed",
                    Some(exit),
                    "worker failed",
                    SystemClock.now_ms(),
                    None,
                )?;
                conn.execute(
                    "UPDATE task_runs SET log_path=?1 WHERE id=?2",
                    params![log_path.to_string_lossy(), claim.run_id],
                )
                .map_err(storage)?;
            }
        }
        Ok(())
    })?;
    Ok(DispatchResult {
        claimed: 1,
        task_id: Some(claim.task.id),
        run_id: Some(claim.run_id),
        exit_code: Some(exit),
    })
}

fn promote_due_tasks(conn: &Connection, board_id: &str, actor: &str, now: i64) -> Result<usize> {
    let candidates = query_tasks(conn, board_id)?
        .into_iter()
        .filter(|task| matches!(task.status, TaskStatus::Todo | TaskStatus::Scheduled))
        .collect::<Vec<_>>();
    let mut promoted = 0;
    for task in candidates {
        if recompute_ready_status(conn, &task, now)? == TaskStatus::Ready {
            with_immediate_tx(conn, || {
                set_status(
                    conn,
                    board_id,
                    &task.id,
                    TaskStatus::Ready,
                    actor,
                    "task.promoted",
                    now,
                )
            })?;
            promoted += 1;
        }
    }
    Ok(promoted)
}

struct WorkerOutput {
    status: ExitStatus,
}

fn validate_dispatch_options(options: &DispatchOptions) -> Result<()> {
    if options.claim_ttl_ms <= 0 {
        return Err(KanbanError::InvalidInput(
            "claim_ttl_ms must be positive".into(),
        ));
    }
    if options.heartbeat_interval_ms <= 0 {
        return Err(KanbanError::InvalidInput(
            "heartbeat_interval_ms must be positive".into(),
        ));
    }
    if options.heartbeat_interval_ms >= options.claim_ttl_ms {
        return Err(KanbanError::InvalidInput(
            "heartbeat_interval_ms must be less than claim_ttl_ms".into(),
        ));
    }
    Ok(())
}

fn run_worker_with_heartbeat(
    path: &Path,
    board: &str,
    options: &DispatchOptions,
    claim: &ClaimResult,
    log_path: &Path,
) -> Result<WorkerOutput> {
    let stdout = File::create(log_path).map_err(|e| KanbanError::Storage(e.to_string()))?;
    let stderr = stdout
        .try_clone()
        .map_err(|e| KanbanError::Storage(e.to_string()))?;
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&options.command)
        .env("KB_DB_PATH", path)
        .env("KB_BOARD_ID", &claim.task.board_id)
        .env("KB_BOARD_SLUG", board)
        .env("KB_TASK_ID", &claim.task.id)
        .env("KB_TASK_SEQ", claim.task.seq.to_string())
        .env("KB_TASK_TITLE", &claim.task.title)
        .env("KB_CLAIM_TOKEN", &claim.claim_token)
        .env("KB_RUN_ID", &claim.run_id)
        .env("KB_ACTOR", &options.actor)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|e| KanbanError::Storage(e.to_string()))?;

    let heartbeat_interval = Duration::from_millis(options.heartbeat_interval_ms as u64);
    let poll_interval = heartbeat_interval.min(Duration::from_millis(10));
    let mut elapsed_since_heartbeat = Duration::ZERO;
    loop {
        match child
            .try_wait()
            .map_err(|e| KanbanError::Storage(e.to_string()))?
        {
            Some(status) => return Ok(WorkerOutput { status }),
            None => {
                thread::sleep(poll_interval);
                elapsed_since_heartbeat += poll_interval;
                if elapsed_since_heartbeat < heartbeat_interval {
                    continue;
                }
                elapsed_since_heartbeat = Duration::ZERO;
                let conn = connect_file(path)?;
                let board_id = board_id(&conn, board)?;
                let task = get_task_by_id(&conn, &board_id, &claim.task.id)?;
                if let Err(err) = with_immediate_tx(&conn, || {
                    heartbeat_task_conn(
                        &conn,
                        &board_id,
                        &options.actor,
                        &task,
                        &claim.claim_token,
                        options.claim_ttl_ms,
                        None,
                        SystemClock.now_ms(),
                    )
                }) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(err);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn reclaim_running_task(
    conn: &Connection,
    board_id: &str,
    task: &TaskRecord,
    actor: &str,
    run_status: &str,
    reason: &str,
    target: TaskStatus,
    now: i64,
    expiry_guard: Option<i64>,
) -> Result<()> {
    if task.status != TaskStatus::Running
        || task.claim_token.is_none()
        || task.current_run_id.is_none()
    {
        return Err(KanbanError::InvalidTransition(
            "reclaim requires matching running claim".into(),
        ));
    }
    if let (Some(run_id), Some(token)) = (&task.current_run_id, &task.claim_token) {
        let changed = conn
            .execute(
                "UPDATE task_runs SET status=?1, finished_at=?2, error=?3 WHERE id=?4 AND board_id=?5 AND task_id=?6 AND status='running' AND claim_token=?7",
                params![run_status, now, reason, run_id, board_id, task.id, token],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(KanbanError::InvalidTransition(
                "reclaim requires matching running run".into(),
            ));
        }
    }
    let changed = conn
        .execute(
            "UPDATE tasks SET status=?1, status_reason=?2, claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, retry_count=retry_count+1, updated_at=?3, lock_version=lock_version+1 WHERE id=?4 AND board_id=?5 AND status='running' AND claim_token=?6 AND current_run_id=?7 AND (?8 IS NULL OR claim_expires_at <= ?8)",
            params![target.as_str(), (target == TaskStatus::Blocked).then_some(reason), now, task.id, board_id, task.claim_token, task.current_run_id, expiry_guard],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition(
            "reclaim requires matching running claim".into(),
        ));
    }
    let payload = json!({
        "retry_count": task.retry_count + 1,
        "max_retries": task.max_retries,
        "to_status": target.as_str(),
        "reason": reason,
    })
    .to_string();
    insert_event(
        conn,
        board_id,
        Some(&task.id),
        task.current_run_id.as_deref(),
        "task.reclaimed",
        actor,
        &payload,
        now,
    )
}

#[allow(clippy::too_many_arguments)]
fn retry_running_task(
    conn: &Connection,
    board_id: &str,
    task: &TaskRecord,
    actor: &str,
    run_status: &str,
    exit_code: Option<i32>,
    reason: &str,
    now: i64,
    expiry_guard: Option<i64>,
) -> Result<()> {
    if task.status != TaskStatus::Running
        || task.claim_token.is_none()
        || task.current_run_id.is_none()
    {
        return Err(KanbanError::InvalidTransition(
            "retry requires matching running claim".into(),
        ));
    }
    let new_retry_count = task.retry_count + 1;
    let blocked = task
        .max_retries
        .is_some_and(|max_retries| new_retry_count >= max_retries);
    let target = if blocked {
        TaskStatus::Blocked
    } else {
        TaskStatus::Ready
    };
    if let (Some(run_id), Some(token)) = (&task.current_run_id, &task.claim_token) {
        let changed = conn
            .execute(
                "UPDATE task_runs SET status=?1, finished_at=?2, exit_code=?3, error=?4 WHERE id=?5 AND board_id=?6 AND task_id=?7 AND status='running' AND claim_token=?8",
                params![run_status, now, exit_code, reason, run_id, board_id, task.id, token],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(KanbanError::InvalidTransition(
                "retry requires matching running run".into(),
            ));
        }
    }
    let changed = conn
        .execute(
            "UPDATE tasks SET status=?1, status_reason=?2, claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, retry_count=?3, updated_at=?4, lock_version=lock_version+1 WHERE id=?5 AND board_id=?6 AND status='running' AND claim_token=?7 AND current_run_id=?8 AND (?9 IS NULL OR claim_expires_at <= ?9)",
            params![target.as_str(), if blocked { Some(reason) } else { None }, new_retry_count, now, task.id, board_id, task.claim_token, task.current_run_id, expiry_guard],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition(
            "retry requires matching running claim".into(),
        ));
    }
    let payload = json!({
        "retry_count": new_retry_count,
        "max_retries": task.max_retries,
    })
    .to_string();
    insert_event(
        conn,
        board_id,
        Some(&task.id),
        task.current_run_id.as_deref(),
        if blocked {
            "task.blocked"
        } else {
            "task.reclaimed"
        },
        actor,
        &payload,
        now,
    )?;
    if blocked && reason == "claim expired" {
        insert_event(
            conn,
            board_id,
            Some(&task.id),
            task.current_run_id.as_deref(),
            "task.reclaimed",
            actor,
            &payload,
            now,
        )?;
    }
    Ok(())
}

fn initial_status(
    explicit: Option<TaskStatus>,
    description: Option<&str>,
    scheduled_at: Option<i64>,
    now: i64,
) -> Result<TaskStatus> {
    if let Some(status) = explicit {
        if !status.can_be_created() {
            return Err(KanbanError::InvalidInput(
                "initial status must be triage/todo/scheduled/ready".into(),
            ));
        }
        match status {
            TaskStatus::Scheduled if scheduled_at.is_none() => {
                return Err(KanbanError::InvalidInput(
                    "scheduled initial status requires scheduled_at".into(),
                ));
            }
            TaskStatus::Ready
                if description.is_none_or(|description| description.trim().is_empty()) =>
            {
                return Err(KanbanError::InvalidInput(
                    "ready requires description".into(),
                ));
            }
            TaskStatus::Ready if scheduled_at.is_some_and(|scheduled| scheduled > now) => {
                return Err(KanbanError::InvalidInput(
                    "ready requires scheduled_at to be due".into(),
                ));
            }
            _ => {
                return Ok(status);
            }
        }
    }
    if description.is_none_or(|d| d.trim().is_empty()) {
        return Ok(TaskStatus::Triage);
    }
    if scheduled_at.is_some_and(|t| t > now) {
        return Ok(TaskStatus::Scheduled);
    }
    Ok(TaskStatus::Ready)
}

#[allow(clippy::too_many_arguments)]
fn finish_running(
    conn: &Connection,
    board_id: &str,
    task: &TaskRecord,
    target: TaskStatus,
    actor: &str,
    event: &str,
    run_status: &str,
    exit_code: i32,
    reason: Option<&str>,
    log_path: Option<&Path>,
    summary: Option<&str>,
    result_json: Option<&str>,
    now: i64,
) -> Result<()> {
    let completed = if target == TaskStatus::Done {
        Some(now)
    } else {
        task.completed_at
    };
    if task.status != TaskStatus::Running
        && !(task.status == TaskStatus::Review && target == TaskStatus::Done)
    {
        return Err(KanbanError::InvalidTransition(
            "finish requires matching running claim".into(),
        ));
    }
    let changed = if task.status == TaskStatus::Running {
        if task.claim_token.is_none() || task.current_run_id.is_none() {
            return Err(KanbanError::InvalidTransition(
                "finish requires matching running claim".into(),
            ));
        }
        conn.execute(
            "UPDATE tasks SET status=?1, status_reason=?2, completed_at=?3, result_summary=COALESCE(?4, result_summary), result_json=COALESCE(?5, result_json), claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, updated_at=?6, lock_version=lock_version+1 WHERE id=?7 AND board_id=?8 AND status='running' AND claim_token=?9 AND current_run_id=?10",
            params![target.as_str(), reason, completed, summary, result_json, now, task.id, board_id, task.claim_token, task.current_run_id],
        )
    } else {
        conn.execute(
            "UPDATE tasks SET status=?1, status_reason=?2, completed_at=?3, result_summary=COALESCE(?4, result_summary), result_json=COALESCE(?5, result_json), claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, updated_at=?6, lock_version=lock_version+1 WHERE id=?7 AND board_id=?8 AND status='review'",
            params![target.as_str(), reason, completed, summary, result_json, now, task.id, board_id],
        )
    }
    .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition(
            "finish requires matching running claim".into(),
        ));
    }
    let event_payload = json!({
        "result": result_json.and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok()),
    })
    .to_string();
    if let Some(run_id) = &task.current_run_id {
        let changed = conn.execute(
            "UPDATE task_runs SET status=?1, finished_at=?2, exit_code=?3, error=?4, log_path=COALESCE(?5, log_path), summary=COALESCE(?6, summary) WHERE id=?7 AND board_id=?8 AND task_id=?9 AND status='running' AND claim_token IS ?10",
            params![run_status, now, exit_code, reason, log_path.map(|p| p.to_string_lossy().to_string()), summary, run_id, board_id, task.id, task.claim_token],
        ).map_err(storage)?;
        if task.status == TaskStatus::Running && changed != 1 {
            return Err(KanbanError::InvalidTransition(
                "finish requires matching running run".into(),
            ));
        }
        insert_event(
            conn,
            board_id,
            Some(&task.id),
            Some(run_id),
            event,
            actor,
            &event_payload,
            now,
        )?;
    } else {
        insert_event(
            conn,
            board_id,
            Some(&task.id),
            None,
            event,
            actor,
            &event_payload,
            now,
        )?;
    }
    Ok(())
}

fn set_status(
    conn: &Connection,
    board_id: &str,
    task_id: &str,
    status: TaskStatus,
    actor: &str,
    event: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE tasks SET status=?1, updated_at=?2, lock_version=lock_version+1 WHERE id=?3",
        params![status.as_str(), now, task_id],
    )
    .map_err(storage)?;
    let payload = json!({ "to_status": status.as_str() }).to_string();
    insert_event(
        conn,
        board_id,
        Some(task_id),
        None,
        event,
        actor,
        &payload,
        now,
    )
}

fn promote_children(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    parent_id: &str,
    now: i64,
) -> Result<()> {
    let mut stmt = conn
        .prepare("SELECT child_task_id FROM task_dependencies WHERE parent_task_id=?1")
        .map_err(storage)?;
    let child_ids = stmt
        .query_map([parent_id], |r| r.get::<_, String>(0))
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    for child_id in child_ids {
        let child = get_task_by_id(conn, board_id, &child_id)?;
        if matches!(child.status, TaskStatus::Todo | TaskStatus::Scheduled)
            && recompute_ready_status(conn, &child, now)? == TaskStatus::Ready
        {
            set_status(
                conn,
                board_id,
                &child_id,
                TaskStatus::Ready,
                actor,
                "task.promoted",
                now,
            )?;
        }
    }
    Ok(())
}

fn recompute_ready_status(conn: &Connection, task: &TaskRecord, now: i64) -> Result<TaskStatus> {
    if task.title.trim().is_empty()
        || task
            .description
            .as_deref()
            .is_none_or(|description| description.trim().is_empty())
    {
        return Ok(TaskStatus::Triage);
    }
    if task.scheduled_at.is_some_and(|t| t > now) {
        return Ok(TaskStatus::Scheduled);
    }
    if !dependencies_done(conn, &task.id)? {
        return Ok(TaskStatus::Todo);
    }
    Ok(TaskStatus::Ready)
}

fn ensure_dependencies_done(conn: &Connection, task_id: &str) -> Result<()> {
    if dependencies_done(conn, task_id)? {
        Ok(())
    } else {
        Err(KanbanError::InvalidTransition("dependency blocked".into()))
    }
}

fn dependencies_done(conn: &Connection, task_id: &str) -> Result<bool> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM task_dependencies d JOIN tasks p ON p.id=d.parent_task_id WHERE d.child_task_id=?1 AND p.status != 'done'", [task_id], |r| r.get(0)).map_err(storage)?;
    Ok(count == 0)
}

fn has_path(conn: &Connection, start: &str, goal: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "WITH RECURSIVE walk(id) AS (SELECT child_task_id FROM task_dependencies WHERE parent_task_id=?1 UNION SELECT d.child_task_id FROM task_dependencies d JOIN walk w ON d.parent_task_id=w.id) SELECT COUNT(*) FROM walk WHERE id=?2",
        params![start, goal], |r| r.get(0)).map_err(storage)?;
    Ok(count > 0)
}

const TASK_COLUMNS: &str = "id,board_id,seq,title,description,status,status_reason,assignee,priority,position,scheduled_at,due_at,created_by,created_at,updated_at,started_at,completed_at,archived_at,claim_token,claim_owner,claim_expires_at,last_heartbeat_at,current_run_id,retry_count,max_retries,result_summary,result_json,metadata_json,lock_version";

fn task_query_where(board_id: &str, options: &TaskListOptions) -> (String, Vec<Value>) {
    let mut clauses = vec!["WHERE board_id=?".to_owned()];
    let mut params = vec![Value::Text(board_id.to_owned())];
    if !options.include_archived {
        clauses.push("status != 'archived'".to_owned());
    }
    if !options.statuses.is_empty() {
        let placeholders = std::iter::repeat_n("?", options.statuses.len())
            .collect::<Vec<_>>()
            .join(",");
        clauses.push(format!("status IN ({placeholders})"));
        params.extend(
            options
                .statuses
                .iter()
                .map(|status| Value::Text(status.as_str().to_owned())),
        );
    }
    if let Some(assignee) = options
        .assignee
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        clauses.push("assignee=?".to_owned());
        params.push(Value::Text(assignee.to_owned()));
    }
    if let Some(search) = options
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let needle = format!("%{}%", search.to_lowercase());
        clauses.push("(lower(title) LIKE ? OR lower(COALESCE(description, '')) LIKE ?)".to_owned());
        params.push(Value::Text(needle.clone()));
        params.push(Value::Text(needle));
    }
    (clauses.join(" AND "), params)
}

fn task_order_by(sort: TaskListSort) -> &'static str {
    match sort {
        TaskListSort::Position => "position ASC, created_at ASC, seq ASC",
        TaskListSort::PositionDesc => "position DESC, created_at DESC, seq DESC",
        TaskListSort::Priority => "priority ASC, created_at ASC, seq ASC",
        TaskListSort::PriorityDesc => "priority DESC, created_at DESC, seq DESC",
        TaskListSort::CreatedAt => "created_at ASC, seq ASC",
        TaskListSort::CreatedAtDesc => "created_at DESC, seq DESC",
        TaskListSort::UpdatedAt => "updated_at ASC, seq ASC",
        TaskListSort::UpdatedAtDesc => "updated_at DESC, seq DESC",
        TaskListSort::DueAt => "COALESCE(due_at, 9223372036854775807) ASC, created_at ASC, seq ASC",
        TaskListSort::DueAtDesc => {
            "COALESCE(due_at, -9223372036854775808) DESC, created_at DESC, seq DESC"
        }
    }
}

fn query_tasks(conn: &Connection, board_id: &str) -> Result<Vec<TaskRecord>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {TASK_COLUMNS} FROM tasks WHERE board_id=?1 ORDER BY CASE status WHEN 'triage' THEN 10 WHEN 'todo' THEN 20 WHEN 'scheduled' THEN 30 WHEN 'ready' THEN 40 WHEN 'running' THEN 50 WHEN 'blocked' THEN 60 WHEN 'review' THEN 70 WHEN 'done' THEN 80 ELSE 90 END, position ASC, priority DESC, created_at ASC"
        ))
        .map_err(storage)?;
    let rows = stmt.query_map([board_id], task_from_row).map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn get_task_by_id(conn: &Connection, board_id: &str, task_id: &str) -> Result<TaskRecord> {
    conn.query_row(
        &format!("SELECT {TASK_COLUMNS} FROM tasks WHERE board_id=?1 AND id=?2"),
        params![board_id, task_id],
        task_from_row,
    )
    .optional()
    .map_err(storage)?
    .ok_or_else(|| KanbanError::NotFound(format!("task {task_id}")))
}

fn resolve_task(conn: &Connection, board_id: &str, task_ref: &str) -> Result<TaskRecord> {
    if let Some(seq) = task_ref.strip_prefix('#') {
        let seq: i64 = seq
            .parse()
            .map_err(|_| KanbanError::InvalidInput("invalid task seq".into()))?;
        conn.query_row("SELECT id,board_id,seq,title,description,status,status_reason,assignee,priority,position,scheduled_at,due_at,created_by,created_at,updated_at,started_at,completed_at,archived_at,claim_token,claim_owner,claim_expires_at,last_heartbeat_at,current_run_id,retry_count,max_retries,result_summary,result_json,metadata_json,lock_version FROM tasks WHERE board_id=?1 AND seq=?2", params![board_id, seq], task_from_row).optional().map_err(storage)?.ok_or_else(|| KanbanError::NotFound(format!("task #{seq}")))
    } else {
        get_task_by_id(conn, board_id, task_ref)
    }
}

fn task_from_row(row: &Row<'_>) -> rusqlite::Result<TaskRecord> {
    let status: String = row.get(5)?;
    Ok(TaskRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        seq: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        status: TaskStatus::try_from(status.as_str())
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        status_reason: row.get(6)?,
        assignee: row.get(7)?,
        priority: row.get(8)?,
        position: row.get(9)?,
        scheduled_at: row.get(10)?,
        due_at: row.get(11)?,
        created_by: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        started_at: row.get(15)?,
        completed_at: row.get(16)?,
        archived_at: row.get(17)?,
        claim_token: row.get(18)?,
        claim_owner: row.get(19)?,
        claim_expires_at: row.get(20)?,
        last_heartbeat_at: row.get(21)?,
        current_run_id: row.get(22)?,
        retry_count: row.get(23)?,
        max_retries: row.get(24)?,
        result_summary: row.get(25)?,
        result_json: row.get(26)?,
        metadata_json: row.get(27)?,
        lock_version: row.get(28)?,
    })
}

fn event_from_row(row: &Row<'_>) -> rusqlite::Result<EventRecord> {
    Ok(EventRecord {
        id: row.get(0)?,
        event_id: row.get(1)?,
        task_id: row.get(2)?,
        run_id: row.get(3)?,
        kind: row.get(4)?,
        actor: row.get(5)?,
        payload_json: row.get(6)?,
        created_at: row.get(7)?,
    })
}
fn run_from_row(row: &Row<'_>) -> rusqlite::Result<RunRecord> {
    Ok(RunRecord {
        id: row.get(0)?,
        task_id: row.get(1)?,
        status: row.get(2)?,
        worker_profile: row.get(3)?,
        worker_pid: row.get(4)?,
        claim_token: row.get(5)?,
        claim_owner: row.get(6)?,
        started_at: row.get(7)?,
        finished_at: row.get(8)?,
        exit_code: row.get(9)?,
        summary: row.get(10)?,
        error: row.get(11)?,
        log_path: row.get(12)?,
        metadata_json: row.get(13)?,
    })
}

fn get_board_conn(conn: &Connection, slug_or_id: &str) -> Result<BoardRecord> {
    let sql = if slug_or_id.starts_with("b_") {
        "SELECT id,slug,name,description,created_at,updated_at,archived_at FROM boards WHERE id=?1 AND archived_at IS NULL"
    } else {
        "SELECT id,slug,name,description,created_at,updated_at,archived_at FROM boards WHERE slug=?1 AND archived_at IS NULL"
    };
    conn.query_row(sql, [slug_or_id], board_from_row)
        .optional()
        .map_err(storage)?
        .ok_or_else(|| KanbanError::NotFound(format!("board {slug_or_id}")))
}

fn board_from_row(row: &Row<'_>) -> rusqlite::Result<BoardRecord> {
    Ok(BoardRecord {
        id: row.get(0)?,
        slug: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        archived_at: row.get(6)?,
    })
}

fn board_column_from_row(row: &Row<'_>) -> rusqlite::Result<BoardColumnRecord> {
    let status: String = row.get(2)?;
    let hidden: i64 = row.get(5)?;
    Ok(BoardColumnRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        status: TaskStatus::try_from(status.as_str())
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        title: row.get(3)?,
        position: row.get(4)?,
        hidden: hidden != 0,
        wip_limit: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn board_id(conn: &Connection, slug_or_id: &str) -> Result<String> {
    let sql = if slug_or_id.starts_with("b_") {
        "SELECT id FROM boards WHERE id=?1"
    } else {
        "SELECT id FROM boards WHERE slug=?1"
    };
    conn.query_row(sql, [slug_or_id], |r| r.get(0))
        .optional()
        .map_err(storage)?
        .ok_or_else(|| KanbanError::NotFound(format!("board {slug_or_id}")))
}

fn board_id_for_task(conn: &Connection, task_id: &str) -> Result<String> {
    conn.query_row("SELECT board_id FROM tasks WHERE id=?1", [task_id], |r| {
        r.get(0)
    })
    .optional()
    .map_err(storage)?
    .ok_or_else(|| KanbanError::NotFound(format!("task {task_id}")))
}

#[allow(clippy::too_many_arguments)]
fn insert_event(
    conn: &Connection,
    board_id: &str,
    task_id: Option<&str>,
    run_id: Option<&str>,
    kind: &str,
    actor: &str,
    payload: &str,
    now: i64,
) -> Result<()> {
    conn.execute("INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", params![new_event_id(), board_id, task_id, run_id, kind, actor, payload, now]).map_err(storage)?;
    Ok(())
}

fn json_valid(conn: &Connection, json: &str) -> Result<bool> {
    conn.query_row("SELECT json_valid(?1)", [json], |r| r.get::<_, i64>(0))
        .map(|v| v == 1)
        .map_err(storage)
}

fn with_immediate_tx<T>(conn: &Connection, f: impl FnOnce() -> Result<T>) -> Result<T> {
    conn.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
    match f() {
        Ok(value) => {
            conn.execute_batch("COMMIT").map_err(storage)?;
            Ok(value)
        }
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

fn storage(err: rusqlite::Error) -> KanbanError {
    KanbanError::Storage(err.to_string())
}
