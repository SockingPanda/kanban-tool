use crate::connect_file;

use super::{
    CreateTask, MAX_TASK_LIST_LIMIT, TaskListOptions, TaskListPage, TaskListSort, TaskPatch,
    TaskRecord, add_dependency_in_current_tx, board_id, board_id_any, insert_event, json_valid,
    recompute_ready_status, storage, validate_priority, with_immediate_tx,
};

use std::path::Path;

use kanban_core::{
    Clock, KanbanError, ReadinessFacts, Result, SystemClock, TaskStatus,
    initial_status as core_initial_status, is_active_recomputable_status, new_task_id,
};

use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter, types::Value};

use serde_json::json;

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
    validate_retry_policy(input.max_retries)?;
    validate_priority(input.priority)?;
    let title = input.title.trim().to_owned();
    if title.is_empty() {
        return Err(KanbanError::InvalidInput("title is required".into()));
    }
    if !json_valid(&conn, &input.metadata_json)? {
        return Err(KanbanError::InvalidInput(
            "metadata_json must be valid JSON".into(),
        ));
    }
    let status = core_initial_status(
        input.status,
        ReadinessFacts {
            title: &title,
            description: input.description.as_deref(),
            scheduled_at: input.scheduled_at,
            dependencies_done: true,
        },
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
        if input.max_retries.is_some() {
            update_retry_policy_in_current_tx(
                &conn,
                &board_id,
                actor,
                &id,
                input.max_retries,
                now,
            )?;
        }
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
    validate_retry_policy(patch.max_retries.flatten())?;
    if let Some(priority) = patch.priority {
        validate_priority(priority)?;
    }
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let mut task = resolve_task(&conn, &board_id, task_ref)?;
        let content_recompute_allowed = task.status != TaskStatus::Todo;
        let recompute_needed = patch.scheduled_at.is_some()
            || (content_recompute_allowed
                && (patch.title.is_some() || patch.description.is_some()));
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
        if recompute_needed && is_active_recomputable_status(task.status) {
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
        if let Some(max_retries) = patch.max_retries {
            update_retry_policy_in_current_tx(&conn, &board_id, actor, &task.id, max_retries, now)?;
        }
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
    with_immediate_tx(&conn, || {
        let board_id = active_board_id_for_task(&conn, task_id)?;
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
    validate_page_bounds(options.limit, MAX_TASK_LIST_LIMIT, options.offset)?;
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
    page_params.push(Value::Integer(
        options.limit.try_into().expect("validated limit"),
    ));
    page_params.push(Value::Integer(
        options.offset.try_into().expect("validated offset"),
    ));
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
    get_task_by_id_global_conn(&conn, task_id)
}

pub fn update_task_by_id(
    path: impl AsRef<Path>,
    actor: &str,
    task_id: &str,
    patch: TaskPatch,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = active_board_id_for_task(&conn, task_id)?;
    drop(conn);
    update_task(path, &board_id, actor, task_id, patch)
}

pub fn set_task_retry_policy_by_id(
    path: impl AsRef<Path>,
    actor: &str,
    task_id: &str,
    max_retries: Option<i64>,
) -> Result<TaskRecord> {
    validate_retry_policy(max_retries)?;
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = active_board_id_for_task(&conn, task_id)?;
        update_retry_policy_in_current_tx(&conn, &board_id, actor, task_id, max_retries, now)?;
        get_task_by_id(&conn, &board_id, task_id)
    })
}

fn validate_retry_policy(max_retries: Option<i64>) -> Result<()> {
    if max_retries.is_some_and(|value| value <= 0) {
        return Err(KanbanError::InvalidInput(
            "max_retries must be a positive integer".into(),
        ));
    }
    Ok(())
}

fn update_retry_policy_in_current_tx(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    task_id: &str,
    max_retries: Option<i64>,
    now: i64,
) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE tasks SET max_retries=?1, updated_at=?2, lock_version=lock_version+1 WHERE id=?3 AND board_id=?4",
            params![max_retries, now, task_id, board_id],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition(
            "retry policy update failed".into(),
        ));
    }
    insert_event(
        conn,
        board_id,
        Some(task_id),
        None,
        "task.retry_policy.updated",
        actor,
        &json!({ "max_retries": max_retries }).to_string(),
        now,
    )
}

pub(crate) fn task_query_where(board_id: &str, options: &TaskListOptions) -> (String, Vec<Value>) {
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
    if !options.priorities.is_empty() {
        let placeholders = std::iter::repeat_n("?", options.priorities.len())
            .collect::<Vec<_>>()
            .join(",");
        clauses.push(format!("priority IN ({placeholders})"));
        params.extend(options.priorities.iter().copied().map(Value::Integer));
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
        if let Some((clause, search_params)) = task_ref_filter(search, "tasks.") {
            clauses.push(clause);
            params.extend(search_params);
        } else {
            let needle = format!("%{}%", sqlite_like_literal(&search.to_lowercase()));
            clauses.push(
                "(lower(title) LIKE ? ESCAPE '\\' OR lower(COALESCE(description, '')) LIKE ? ESCAPE '\\')"
                    .to_owned(),
            );
            params.push(Value::Text(needle.clone()));
            params.push(Value::Text(needle));
        }
    }
    (clauses.join(" AND "), params)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskSearchQuery {
    TaskId(String),
    Seq(i64),
    BoardSeq { board: String, seq: i64 },
    RefNoMatch,
    Text,
}

pub(crate) fn classify_task_search_query(search: &str) -> TaskSearchQuery {
    let search = search.trim();
    if search.starts_with("t_") {
        return TaskSearchQuery::TaskId(search.to_owned());
    }
    if let Some((board, seq_ref)) = split_board_seq_ref(search) {
        return parse_task_search_seq(seq_ref)
            .map(|seq| TaskSearchQuery::BoardSeq {
                board: board.to_owned(),
                seq,
            })
            .unwrap_or(TaskSearchQuery::RefNoMatch);
    }
    if search.starts_with('#') {
        return parse_task_search_seq(search)
            .map(TaskSearchQuery::Seq)
            .unwrap_or(TaskSearchQuery::RefNoMatch);
    }
    if search.chars().all(|ch| ch.is_ascii_digit()) {
        return parse_task_search_seq(search)
            .map(TaskSearchQuery::Seq)
            .unwrap_or(TaskSearchQuery::RefNoMatch);
    }
    TaskSearchQuery::Text
}

pub(crate) fn task_ref_filter(search: &str, task_prefix: &str) -> Option<(String, Vec<Value>)> {
    match classify_task_search_query(search) {
        TaskSearchQuery::TaskId(task_id) => {
            Some((format!("{task_prefix}id=?"), vec![Value::Text(task_id)]))
        }
        TaskSearchQuery::Seq(seq) => {
            Some((format!("{task_prefix}seq=?"), vec![Value::Integer(seq)]))
        }
        TaskSearchQuery::BoardSeq { board, seq } => Some((
            format!(
                "{task_prefix}seq=? AND EXISTS (SELECT 1 FROM boards b WHERE b.id={task_prefix}board_id AND (b.slug=? OR b.id=?))"
            ),
            vec![
                Value::Integer(seq),
                Value::Text(board.clone()),
                Value::Text(board),
            ],
        )),
        TaskSearchQuery::RefNoMatch => Some(("0=1".to_owned(), Vec::new())),
        TaskSearchQuery::Text => None,
    }
}

fn parse_task_search_seq(seq_ref: &str) -> Option<i64> {
    let seq = seq_ref.strip_prefix('#').unwrap_or(seq_ref);
    if seq.is_empty() || !seq.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    seq.parse().ok()
}

pub(crate) fn validate_page_bounds(limit: usize, max_limit: usize, offset: usize) -> Result<()> {
    if limit > max_limit {
        return Err(KanbanError::InvalidInput(format!(
            "limit must be <= {max_limit}"
        )));
    }
    if offset > i64::MAX as usize {
        return Err(KanbanError::InvalidInput(format!(
            "offset must be <= {}",
            i64::MAX
        )));
    }
    Ok(())
}

pub(crate) fn sqlite_like_literal(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '%' | '_' | '\\' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

pub(crate) fn task_order_by(sort: TaskListSort) -> &'static str {
    match sort {
        TaskListSort::Seq => "seq ASC",
        TaskListSort::SeqDesc => "seq DESC",
        TaskListSort::Title => "lower(title) ASC, seq ASC",
        TaskListSort::TitleDesc => "lower(title) DESC, seq DESC",
        TaskListSort::Status => {
            "CASE status WHEN 'triage' THEN 10 WHEN 'todo' THEN 20 WHEN 'scheduled' THEN 30 WHEN 'ready' THEN 40 WHEN 'running' THEN 50 WHEN 'blocked' THEN 60 WHEN 'review' THEN 70 WHEN 'done' THEN 80 ELSE 90 END ASC, position ASC, seq ASC"
        }
        TaskListSort::StatusDesc => {
            "CASE status WHEN 'triage' THEN 10 WHEN 'todo' THEN 20 WHEN 'scheduled' THEN 30 WHEN 'ready' THEN 40 WHEN 'running' THEN 50 WHEN 'blocked' THEN 60 WHEN 'review' THEN 70 WHEN 'done' THEN 80 ELSE 90 END DESC, position DESC, seq DESC"
        }
        TaskListSort::Position => "position ASC, created_at ASC, seq ASC",
        TaskListSort::PositionDesc => "position DESC, created_at DESC, seq DESC",
        TaskListSort::Priority => "priority ASC, created_at ASC, seq ASC",
        TaskListSort::PriorityDesc => "priority DESC, created_at DESC, seq DESC",
        TaskListSort::Assignee => "COALESCE(assignee, claim_owner, '') ASC, seq ASC",
        TaskListSort::AssigneeDesc => "COALESCE(assignee, claim_owner, '') DESC, seq DESC",
        TaskListSort::ScheduledAt => {
            "COALESCE(scheduled_at, 9223372036854775807) ASC, created_at ASC, seq ASC"
        }
        TaskListSort::ScheduledAtDesc => {
            "COALESCE(scheduled_at, -9223372036854775808) DESC, created_at DESC, seq DESC"
        }
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

pub(crate) const TASK_COLUMNS: &str = "id,board_id,(SELECT slug FROM boards WHERE boards.id=tasks.board_id) AS board_slug,((SELECT slug FROM boards WHERE boards.id=tasks.board_id) || '#' || seq) AS task_ref,seq,title,description,status,status_reason,assignee,priority,position,scheduled_at,due_at,created_by,created_at,updated_at,started_at,completed_at,archived_at,claim_token,claim_owner,claim_expires_at,last_heartbeat_at,current_run_id,retry_count,max_retries,result_summary,result_json,metadata_json,lock_version,EXISTS(SELECT 1 FROM task_dependencies d JOIN tasks p ON p.id=d.parent_task_id WHERE d.child_task_id=tasks.id AND p.status NOT IN ('done','archived')) AS dependency_blocked,(SELECT COUNT(*) FROM task_dependencies d JOIN tasks p ON p.id=d.parent_task_id WHERE d.child_task_id=tasks.id AND p.status NOT IN ('done','archived')) AS unfinished_parent_count";

pub(crate) fn query_tasks(conn: &Connection, board_id: &str) -> Result<Vec<TaskRecord>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {TASK_COLUMNS} FROM tasks WHERE board_id=?1 ORDER BY CASE status WHEN 'triage' THEN 10 WHEN 'todo' THEN 20 WHEN 'scheduled' THEN 30 WHEN 'ready' THEN 40 WHEN 'running' THEN 50 WHEN 'blocked' THEN 60 WHEN 'review' THEN 70 WHEN 'done' THEN 80 ELSE 90 END, position ASC, priority ASC, created_at ASC"
        ))
        .map_err(storage)?;
    let rows = stmt.query_map([board_id], task_from_row).map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

pub(crate) fn get_task_by_id(
    conn: &Connection,
    board_id: &str,
    task_id: &str,
) -> Result<TaskRecord> {
    conn.query_row(
        &format!("SELECT {TASK_COLUMNS} FROM tasks WHERE board_id=?1 AND id=?2"),
        params![board_id, task_id],
        task_from_row,
    )
    .optional()
    .map_err(storage)?
    .ok_or_else(|| KanbanError::NotFound(format!("task {task_id}")))
}

pub(crate) fn get_task_by_id_global_conn(conn: &Connection, task_id: &str) -> Result<TaskRecord> {
    conn.query_row(
        &format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id=?1"),
        [task_id],
        task_from_row,
    )
    .optional()
    .map_err(storage)?
    .ok_or_else(|| KanbanError::NotFound(format!("task {task_id}")))
}

pub(crate) fn resolve_task(
    conn: &Connection,
    active_board_id: &str,
    task_ref: &str,
) -> Result<TaskRecord> {
    if task_ref.starts_with("t_") {
        return get_task_by_id_global_conn(conn, task_ref);
    }
    if let Some((board_ref, seq_ref)) = split_board_seq_ref(task_ref) {
        let board_id = board_id(conn, board_ref)?;
        let seq = parse_seq_ref(seq_ref)?;
        return get_task_by_seq(conn, &board_id, seq, task_ref);
    }
    let seq = parse_seq_ref(task_ref)?;
    get_task_by_seq(conn, active_board_id, seq, task_ref)
}

pub(crate) fn resolve_task_any(
    conn: &Connection,
    active_board_id: &str,
    task_ref: &str,
) -> Result<TaskRecord> {
    if task_ref.starts_with("t_") {
        return get_task_by_id_global_conn(conn, task_ref);
    }
    if let Some((board_ref, seq_ref)) = split_board_seq_ref(task_ref) {
        let board_id = board_id_any(conn, board_ref)?;
        let seq = parse_seq_ref(seq_ref)?;
        return get_task_by_seq(conn, &board_id, seq, task_ref);
    }
    let seq = parse_seq_ref(task_ref)?;
    get_task_by_seq(conn, active_board_id, seq, task_ref)
}

pub(crate) fn resolve_task_without_active_board(
    conn: &Connection,
    task_ref: &str,
) -> Result<TaskRecord> {
    if task_ref.starts_with("t_") {
        return get_task_by_id_global_conn(conn, task_ref);
    }
    if let Some((board_ref, seq_ref)) = split_board_seq_ref(task_ref) {
        let board_id = board_id_any(conn, board_ref)?;
        let seq = parse_seq_ref(seq_ref)?;
        return get_task_by_seq(conn, &board_id, seq, task_ref);
    }
    Err(KanbanError::InvalidInput(
        "task ref must be a task id or board-qualified ref".into(),
    ))
}

pub(crate) fn split_board_seq_ref(task_ref: &str) -> Option<(&str, &str)> {
    task_ref
        .split_once("/#")
        .or_else(|| task_ref.split_once('#'))
        .filter(|(board, seq)| !board.is_empty() && !seq.is_empty())
}

pub(crate) fn parse_seq_ref(seq_ref: &str) -> Result<i64> {
    let seq = seq_ref.strip_prefix('#').unwrap_or(seq_ref);
    seq.parse()
        .map_err(|_| KanbanError::InvalidInput("invalid task seq".into()))
}

pub(crate) fn get_task_by_seq(
    conn: &Connection,
    board_id: &str,
    seq: i64,
    display_ref: &str,
) -> Result<TaskRecord> {
    conn.query_row(
        &format!("SELECT {TASK_COLUMNS} FROM tasks WHERE board_id=?1 AND seq=?2"),
        params![board_id, seq],
        task_from_row,
    )
    .optional()
    .map_err(storage)?
    .ok_or_else(|| KanbanError::NotFound(format!("task {display_ref}")))
}

pub(crate) fn task_from_row(row: &Row<'_>) -> rusqlite::Result<TaskRecord> {
    let status: String = row.get(7)?;
    Ok(TaskRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        board_slug: row.get(2)?,
        task_ref: row.get(3)?,
        seq: row.get(4)?,
        title: row.get(5)?,
        description: row.get(6)?,
        status: TaskStatus::try_from(status.as_str())
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        status_reason: row.get(8)?,
        assignee: row.get(9)?,
        priority: row.get(10)?,
        position: row.get(11)?,
        scheduled_at: row.get(12)?,
        due_at: row.get(13)?,
        created_by: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
        started_at: row.get(17)?,
        completed_at: row.get(18)?,
        archived_at: row.get(19)?,
        claim_token: row.get(20)?,
        claim_owner: row.get(21)?,
        claim_expires_at: row.get(22)?,
        last_heartbeat_at: row.get(23)?,
        current_run_id: row.get(24)?,
        retry_count: row.get(25)?,
        max_retries: row.get(26)?,
        result_summary: row.get(27)?,
        result_json: row.get(28)?,
        metadata_json: row.get(29)?,
        lock_version: row.get(30)?,
        dependency_blocked: row.get(31)?,
        unfinished_parent_count: row.get(32)?,
    })
}

pub(crate) fn active_board_id_for_task(conn: &Connection, task_id: &str) -> Result<String> {
    conn.query_row(
        "SELECT tasks.board_id FROM tasks JOIN boards ON boards.id=tasks.board_id WHERE tasks.id=?1 AND boards.archived_at IS NULL",
        [task_id],
        |r| r.get(0),
    )
    .optional()
    .map_err(storage)?
    .ok_or_else(|| KanbanError::NotFound(format!("task {task_id}")))
}
