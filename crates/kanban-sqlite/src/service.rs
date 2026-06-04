use std::{
    path::{Path, PathBuf},
    process::Command,
};

use kanban_core::{
    Clock, KanbanError, Result, SystemClock, TaskStatus, new_event_id, new_run_id, new_task_id,
    new_typed_id,
};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

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
            description: Some(String::new()),
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

pub fn create_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    input: CreateTask,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    if input.title.trim().is_empty() {
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
    let seq: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM tasks WHERE board_id=?1",
            [&board_id],
            |r| r.get(0),
        )
        .map_err(storage)?;
    let id = new_task_id();
    conn.execute(
        "INSERT INTO tasks(id, board_id, seq, title, description, status, assignee, priority, position, scheduled_at, due_at, created_by, created_at, updated_at, metadata_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?3 * 1024, ?9, ?10, ?11, ?12, ?12, ?13)",
        params![id, board_id, seq, input.title.trim(), input.description, status.as_str(), input.assignee, input.priority, input.scheduled_at, input.due_at, actor, now, input.metadata_json],
    ).map_err(storage)?;
    insert_event(
        &conn,
        &board_id,
        Some(&id),
        None,
        "task.created",
        actor,
        &format!(r#"{{"status":"{}"}}"#, status.as_str()),
        now,
    )?;
    get_task_by_id(&conn, &board_id, &id)
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
    let mut task = resolve_task(&conn, &board_id, task_ref)?;
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
    conn.execute(
        "UPDATE tasks SET title=?1, description=?2, assignee=?3, priority=?4, scheduled_at=?5, due_at=?6, metadata_json=?7, updated_at=?8, lock_version=lock_version+1 WHERE id=?9 AND board_id=?10",
        params![task.title, task.description, task.assignee, task.priority, task.scheduled_at, task.due_at, task.metadata_json, now, task.id, board_id],
    ).map_err(storage)?;
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

pub fn get_task(path: impl AsRef<Path>, board: &str, task_ref: &str) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    resolve_task(&conn, &board_id, task_ref)
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
}

pub fn claim_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    ttl_ms: i64,
) -> Result<ClaimResult> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    claim_task_conn(&conn, &board_id, actor, &task.id, ttl_ms, "manual", now)
}

fn claim_task_conn(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    task_id: &str,
    ttl_ms: i64,
    profile: &str,
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
        "UPDATE tasks SET status='running', claim_token=?1, claim_owner=?2, claim_expires_at=?3, last_heartbeat_at=?4, started_at=COALESCE(started_at, ?4), updated_at=?4, lock_version=lock_version+1 WHERE id=?5 AND board_id=?6 AND status='ready' AND claim_token IS NULL",
        params![token, actor, expires, now, task_id, board_id],
    ).map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition("claim conflict".into()));
    }
    conn.execute(
        "INSERT INTO task_runs(id, board_id, task_id, status, worker_profile, claim_token, claim_owner, claim_expires_at, started_at, last_heartbeat_at, metadata_json) VALUES (?1, ?2, ?3, 'running', ?4, ?5, ?6, ?7, ?8, ?8, '{}')",
        params![run_id, board_id, task_id, profile, token, actor, expires, now],
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
        &format!(r#"{{"claim_owner":"{}"}}"#, actor),
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
    _actor: &str,
    task_ref: &str,
    token: &str,
    ttl_ms: i64,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    if task.status != TaskStatus::Running || task.claim_token.as_deref() != Some(token) {
        return Err(KanbanError::InvalidTransition(
            "heartbeat requires matching running claim".into(),
        ));
    }
    let expires = now + ttl_ms;
    conn.execute(
        "UPDATE tasks SET claim_expires_at=?1, last_heartbeat_at=?2, updated_at=?2, lock_version=lock_version+1 WHERE id=?3",
        params![expires, now, task.id],
    )
    .map_err(storage)?;
    if let Some(run_id) = &task.current_run_id {
        conn.execute(
            "UPDATE task_runs SET claim_expires_at=?1, last_heartbeat_at=?2 WHERE id=?3",
            params![expires, now, run_id],
        )
        .map_err(storage)?;
    }
    get_task_by_id(&conn, &board_id, &task.id)
}

pub fn complete_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: Option<&str>,
    force: bool,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
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
        now,
    )?;
    promote_children(&conn, &board_id, actor, &task.id, now)?;
    get_task_by_id(&conn, &board_id, &task.id)
}

pub fn submit_review_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: Option<&str>,
    force: bool,
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
        now,
    )?;
    get_task_by_id(&conn, &board_id, &task.id)
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
    let conn = connect_file(path.as_ref())?;
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
        finish_running(
            &conn,
            &board_id,
            &task,
            TaskStatus::Blocked,
            actor,
            "task.blocked",
            "failed",
            1,
            Some(reason),
            None,
            now,
        )?;
    } else {
        conn.execute("UPDATE tasks SET status='blocked', status_reason=?1, updated_at=?2, lock_version=lock_version+1 WHERE id=?3", params![reason, now, task.id]).map_err(storage)?;
        insert_event(
            &conn,
            &board_id,
            Some(&task.id),
            None,
            "task.blocked",
            actor,
            &format!(r#"{{"reason":"{}"}}"#, escape_json(reason)),
            now,
        )?;
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
    conn.execute("UPDATE tasks SET status=?1, status_reason=NULL, updated_at=?2, lock_version=lock_version+1 WHERE id=?3", params![target.as_str(), now, task.id]).map_err(storage)?;
    insert_event(
        &conn,
        &board_id,
        Some(&task.id),
        None,
        "task.unblocked",
        actor,
        &format!(r#"{{"to_status":"{}"}}"#, target.as_str()),
        now,
    )?;
    get_task_by_id(&conn, &board_id, &task.id)
}

pub fn reclaim_expired(path: impl AsRef<Path>, board: &str, actor: &str) -> Result<usize> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let expired: Vec<TaskRecord> = query_tasks(&conn, &board_id)?
        .into_iter()
        .filter(|t| t.status == TaskStatus::Running && t.claim_expires_at.is_some_and(|x| x <= now))
        .collect();
    let count = expired.len();
    for task in expired {
        conn.execute("UPDATE task_runs SET status='expired', finished_at=?1 WHERE id=?2 AND status='running'", params![now, task.current_run_id]).map_err(storage)?;
        conn.execute("UPDATE tasks SET status='ready', claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, retry_count=retry_count+1, updated_at=?1, lock_version=lock_version+1 WHERE id=?2", params![now, task.id]).map_err(storage)?;
        insert_event(
            &conn,
            &board_id,
            Some(&task.id),
            task.current_run_id.as_deref(),
            "task.reclaimed",
            actor,
            "{}",
            now,
        )?;
    }
    Ok(count)
}

pub fn archive_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    force: bool,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    if task.status == TaskStatus::Running && !force {
        return Err(KanbanError::InvalidTransition(
            "cannot archive running without force".into(),
        ));
    }
    conn.execute("UPDATE tasks SET status='archived', archived_at=?1, claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, updated_at=?1, lock_version=lock_version+1 WHERE id=?2", params![now, task.id]).map_err(storage)?;
    insert_event(
        &conn,
        &board_id,
        Some(&task.id),
        task.current_run_id.as_deref(),
        "task.archived",
        actor,
        "{}",
        now,
    )?;
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
    conn.execute("INSERT OR IGNORE INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES (?1, ?2, ?3, ?4)", params![board_id, parent.id, child.id, now]).map_err(storage)?;
    if child.status == TaskStatus::Ready && parent.status != TaskStatus::Done {
        conn.execute("UPDATE tasks SET status='todo', updated_at=?1, lock_version=lock_version+1 WHERE id=?2", params![now, child.id]).map_err(storage)?;
    }
    insert_event(
        &conn,
        &board_id,
        Some(&child.id),
        None,
        "dependency.added",
        actor,
        &format!(r#"{{"parent_task_id":"{}"}}"#, parent.id),
        now,
    )?;
    Ok(())
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
    conn.execute(
        "DELETE FROM task_dependencies WHERE parent_task_id=?1 AND child_task_id=?2",
        params![parent.id, child.id],
    )
    .map_err(storage)?;
    insert_event(
        &conn,
        &board_id,
        Some(&child.id),
        None,
        "dependency.removed",
        actor,
        &format!(r#"{{"parent_task_id":"{}"}}"#, parent.id),
        now,
    )?;
    Ok(())
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
        "SELECT id,task_id,status,worker_profile,worker_pid,claim_token,claim_owner,started_at,finished_at,exit_code,summary,error,log_path FROM task_runs WHERE board_id=?1 AND task_id=?2 ORDER BY started_at DESC"
    } else {
        "SELECT id,task_id,status,worker_profile,worker_pid,claim_token,claim_owner,started_at,finished_at,exit_code,summary,error,log_path FROM task_runs WHERE board_id=?1 ORDER BY started_at DESC"
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

pub fn dispatch_once(
    path: impl AsRef<Path>,
    board: &str,
    options: DispatchOptions,
) -> Result<DispatchResult> {
    let path = path.as_ref();
    reclaim_expired(path, board, &options.actor)?;
    let conn = connect_file(path)?;
    let board_id = board_id(&conn, board)?;
    let ready = query_tasks(&conn, &board_id)?.into_iter().find(|t| {
        t.status == TaskStatus::Ready
            && t.claim_token.is_none()
            && (t.assignee.is_none()
                || t.assignee.as_deref() == Some(options.worker_profile.as_str()))
            && dependencies_done(&conn, &t.id).unwrap_or(false)
    });
    let Some(task) = ready else {
        return Ok(DispatchResult {
            claimed: 0,
            task_id: None,
            run_id: None,
            exit_code: None,
        });
    };
    let now = SystemClock.now_ms();
    let claim = claim_task_conn(
        &conn,
        &board_id,
        &options.actor,
        &task.id,
        options.claim_ttl_ms,
        &options.worker_profile,
        now,
    )?;
    std::fs::create_dir_all(&options.log_dir).map_err(|e| KanbanError::Storage(e.to_string()))?;
    let log_path = options.log_dir.join(format!("{}.log", claim.run_id));
    let output = Command::new("sh")
        .arg("-c")
        .arg(&options.command)
        .env("KB_TASK_ID", &claim.task.id)
        .env("KB_TASK_SEQ", claim.task.seq.to_string())
        .env("KB_CLAIM_TOKEN", &claim.claim_token)
        .env("KB_RUN_ID", &claim.run_id)
        .output()
        .map_err(|e| KanbanError::Storage(e.to_string()))?;
    let mut log = output.stdout;
    log.extend_from_slice(&output.stderr);
    std::fs::write(&log_path, log).map_err(|e| KanbanError::Storage(e.to_string()))?;
    let exit = output.status.code().unwrap_or(1);
    let fresh = get_task_by_id(&conn, &board_id, &claim.task.id)?;
    let target = if output.status.success() {
        options.on_success
    } else {
        options.on_failure
    };
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
                SystemClock.now_ms(),
            )?;
        }
        FinishPolicy::Ready => {
            conn.execute("UPDATE task_runs SET status='failed', finished_at=?1, exit_code=?2, log_path=?3 WHERE id=?4", params![SystemClock.now_ms(), exit, log_path.to_string_lossy(), claim.run_id]).map_err(storage)?;
            conn.execute("UPDATE tasks SET status='ready', claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, updated_at=?1, lock_version=lock_version+1 WHERE id=?2", params![SystemClock.now_ms(), fresh.id]).map_err(storage)?;
        }
    }
    Ok(DispatchResult {
        claimed: 1,
        task_id: Some(claim.task.id),
        run_id: Some(claim.run_id),
        exit_code: Some(exit),
    })
}

fn initial_status(
    explicit: Option<TaskStatus>,
    description: Option<&str>,
    scheduled_at: Option<i64>,
    now: i64,
) -> Result<TaskStatus> {
    if let Some(status) = explicit {
        if status.can_be_created() {
            return Ok(status);
        }
        return Err(KanbanError::InvalidInput(
            "initial status must be triage/todo/scheduled/ready".into(),
        ));
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
    now: i64,
) -> Result<()> {
    let completed = if target == TaskStatus::Done {
        Some(now)
    } else {
        task.completed_at
    };
    conn.execute(
        "UPDATE tasks SET status=?1, status_reason=?2, completed_at=?3, claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, updated_at=?4, lock_version=lock_version+1 WHERE id=?5",
        params![target.as_str(), reason, completed, now, task.id],
    ).map_err(storage)?;
    if let Some(run_id) = &task.current_run_id {
        conn.execute("UPDATE task_runs SET status=?1, finished_at=?2, exit_code=?3, error=?4, log_path=COALESCE(?5, log_path) WHERE id=?6", params![run_status, now, exit_code, reason, log_path.map(|p| p.to_string_lossy().to_string()), run_id]).map_err(storage)?;
        insert_event(
            conn,
            board_id,
            Some(&task.id),
            Some(run_id),
            event,
            actor,
            "{}",
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
            "{}",
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
    insert_event(
        conn,
        board_id,
        Some(task_id),
        None,
        event,
        actor,
        &format!(r#"{{"to_status":"{}"}}"#, status.as_str()),
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
    if task.title.trim().is_empty() {
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

fn query_tasks(conn: &Connection, board_id: &str) -> Result<Vec<TaskRecord>> {
    let mut stmt = conn.prepare("SELECT id,board_id,seq,title,description,status,status_reason,assignee,priority,position,scheduled_at,due_at,created_by,created_at,updated_at,started_at,completed_at,archived_at,claim_token,claim_owner,claim_expires_at,last_heartbeat_at,current_run_id,retry_count,max_retries,result_summary,metadata_json,lock_version FROM tasks WHERE board_id=?1 ORDER BY CASE status WHEN 'triage' THEN 10 WHEN 'todo' THEN 20 WHEN 'scheduled' THEN 30 WHEN 'ready' THEN 40 WHEN 'running' THEN 50 WHEN 'blocked' THEN 60 WHEN 'review' THEN 70 WHEN 'done' THEN 80 ELSE 90 END, position ASC, priority DESC, created_at ASC").map_err(storage)?;
    let rows = stmt.query_map([board_id], task_from_row).map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn get_task_by_id(conn: &Connection, board_id: &str, task_id: &str) -> Result<TaskRecord> {
    conn.query_row("SELECT id,board_id,seq,title,description,status,status_reason,assignee,priority,position,scheduled_at,due_at,created_by,created_at,updated_at,started_at,completed_at,archived_at,claim_token,claim_owner,claim_expires_at,last_heartbeat_at,current_run_id,retry_count,max_retries,result_summary,metadata_json,lock_version FROM tasks WHERE board_id=?1 AND id=?2", params![board_id, task_id], task_from_row).optional().map_err(storage)?.ok_or_else(|| KanbanError::NotFound(format!("task {task_id}")))
}

fn resolve_task(conn: &Connection, board_id: &str, task_ref: &str) -> Result<TaskRecord> {
    if let Some(seq) = task_ref.strip_prefix('#') {
        let seq: i64 = seq
            .parse()
            .map_err(|_| KanbanError::InvalidInput("invalid task seq".into()))?;
        conn.query_row("SELECT id,board_id,seq,title,description,status,status_reason,assignee,priority,position,scheduled_at,due_at,created_by,created_at,updated_at,started_at,completed_at,archived_at,claim_token,claim_owner,claim_expires_at,last_heartbeat_at,current_run_id,retry_count,max_retries,result_summary,metadata_json,lock_version FROM tasks WHERE board_id=?1 AND seq=?2", params![board_id, seq], task_from_row).optional().map_err(storage)?.ok_or_else(|| KanbanError::NotFound(format!("task #{seq}")))
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
        metadata_json: row.get(26)?,
        lock_version: row.get(27)?,
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

fn storage(err: rusqlite::Error) -> KanbanError {
    KanbanError::Storage(err.to_string())
}
fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
