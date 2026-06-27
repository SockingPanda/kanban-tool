use crate::connect_file;

use super::{
    AttachSubtaskInput, CreateSubtaskInput, StepPlanState, TaskExecutionPlanRecord, TaskRecord,
    TaskSubtaskRecord, UpdateSubtaskInput, all, board_id, create_task, ensure_board_active,
    get_task_by_id, guarded_set_status, insert_event, recompute_ready_status, resolve_task, scalar,
    storage, with_immediate_tx,
};

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use kanban_core::{
    Clock, KanbanError, Result, SystemClock, TaskStatus, is_active_recomputable_status,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::json;

struct ExecutionPlanUpdate<'a> {
    board_id: &'a str,
    task_id: &'a str,
    state: StepPlanState,
    reason: Option<String>,
    actor: &'a str,
    now: i64,
    emit_event: bool,
}

pub fn create_subtask(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    input: CreateSubtaskInput,
) -> Result<TaskSubtaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let parent = resolve_task(&conn, &board_id, parent_ref)?;
    ensure_subtask_endpoint(&parent, &board_id, "parent")?;
    drop(conn);
    let child = create_task(path.as_ref(), board, actor, input.task)?;
    attach_subtask_with_event(
        path,
        board,
        actor,
        parent_ref,
        AttachSubtaskInput {
            child_ref: child.id,
            position: input.position,
            required: input.required,
        },
        "task.subtask.created",
    )
}

pub fn attach_subtask(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    input: AttachSubtaskInput,
) -> Result<TaskSubtaskRecord> {
    attach_subtask_with_event(
        path,
        board,
        actor,
        parent_ref,
        input,
        "task.subtask.attached",
    )
}

fn attach_subtask_with_event(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    input: AttachSubtaskInput,
    event_kind: &str,
) -> Result<TaskSubtaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        let parent = resolve_task(&conn, &board_id, parent_ref)?;
        let child = resolve_task(&conn, &board_id, &input.child_ref)?;
        ensure_subtask_relation_allowed(&conn, &board_id, &parent, &child)?;
        let position = input
            .position
            .map(Ok)
            .unwrap_or_else(|| next_subtask_position(&conn, &parent.id))?;
        let inserted = conn
            .execute(
                "INSERT OR IGNORE INTO task_subtasks(board_id,parent_task_id,child_task_id,position,required,created_by,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![board_id, parent.id, child.id, position, bool_to_i64(input.required), actor, now],
            )
            .map_err(storage)?;
        let record = subtask_record(&conn, &board_id, &parent.id, &child.id)?;
        if inserted == 0 {
            return Ok(record);
        }
        insert_subtask_event(
            &conn, &board_id, &parent.id, event_kind, actor, &record, now,
        )?;
        if input.required {
            upsert_execution_plan_state(
                &conn,
                ExecutionPlanUpdate {
                    board_id: &board_id,
                    task_id: &parent.id,
                    state: StepPlanState::Planned,
                    reason: None,
                    actor,
                    now,
                    emit_event: true,
                },
            )?;
            recompute_parent_after_plan_change(&conn, &board_id, actor, &parent.id, now)?;
        }
        Ok(record)
    })
}

pub fn detach_subtask(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    child_ref: &str,
) -> Result<()> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        let parent = resolve_task(&conn, &board_id, parent_ref)?;
        let child = resolve_task(&conn, &board_id, child_ref)?;
        let existing = subtask_record(&conn, &board_id, &parent.id, &child.id)?;
        conn.execute(
            "DELETE FROM task_subtasks WHERE parent_task_id=?1 AND child_task_id=?2",
            params![parent.id, child.id],
        )
        .map_err(storage)?;
        insert_subtask_event(
            &conn,
            &board_id,
            &parent.id,
            "task.subtask.removed",
            actor,
            &existing,
            now,
        )?;
        recompute_parent_after_plan_change(&conn, &board_id, actor, &parent.id, now)
    })
}

pub fn update_subtask(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    child_ref: &str,
    input: UpdateSubtaskInput,
) -> Result<TaskSubtaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        let parent = resolve_task(&conn, &board_id, parent_ref)?;
        let child = resolve_task(&conn, &board_id, child_ref)?;
        let existing = subtask_record(&conn, &board_id, &parent.id, &child.id)?;
        let position = input.position.unwrap_or(existing.position);
        let required = input.required.unwrap_or(existing.required);
        conn.execute(
            "UPDATE task_subtasks SET position=?1, required=?2 WHERE parent_task_id=?3 AND child_task_id=?4",
            params![position, bool_to_i64(required), parent.id, child.id],
        )
        .map_err(storage)?;
        let updated = subtask_record(&conn, &board_id, &parent.id, &child.id)?;
        if input.position.is_some() {
            insert_subtask_event(
                &conn,
                &board_id,
                &parent.id,
                "task.subtask.reordered",
                actor,
                &updated,
                now,
            )?;
        }
        if required {
            upsert_execution_plan_state(
                &conn,
                ExecutionPlanUpdate {
                    board_id: &board_id,
                    task_id: &parent.id,
                    state: StepPlanState::Planned,
                    reason: None,
                    actor,
                    now,
                    emit_event: true,
                },
            )?;
        }
        if input.required.is_some() {
            recompute_parent_after_plan_change(&conn, &board_id, actor, &parent.id, now)?;
        }
        Ok(updated)
    })
}

pub fn list_subtasks(
    path: impl AsRef<Path>,
    board: &str,
    parent_ref: &str,
) -> Result<Vec<TaskSubtaskRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let parent = resolve_task(&conn, &board_id, parent_ref)?;
    all(
        &conn,
        "SELECT child_task_id FROM task_subtasks WHERE board_id=?1 AND parent_task_id=?2 ORDER BY position ASC, created_at ASC, child_task_id ASC",
        params![board_id, parent.id],
        |row| row.get::<_, String>(0),
    )?
    .into_iter()
    .map(|child_id| subtask_record(&conn, &board_id, &parent.id, &child_id))
    .collect()
}

pub fn execution_plan(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: &str,
) -> Result<TaskExecutionPlanRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    derive_execution_plan(&conn, &board_id, &task.id)
}

pub fn mark_execution_plan_not_required(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    reason: &str,
) -> Result<TaskExecutionPlanRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let reason = reason.trim().to_owned();
    if reason.is_empty() {
        return Err(KanbanError::InvalidInput(
            "execution plan not_required reason is required".into(),
        ));
    }
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        let task = resolve_task(&conn, &board_id, task_ref)?;
        ensure_subtask_endpoint(&task, &board_id, "task")?;
        let required_count: i64 = scalar(
            &conn,
            "SELECT COUNT(*) FROM task_subtasks WHERE board_id=?1 AND parent_task_id=?2 AND required=1",
            params![board_id, task.id],
            |row| row.get(0),
        )?;
        if required_count > 0 {
            return Err(KanbanError::InvalidInput(
                "cannot mark execution plan not_required while required subtasks exist".into(),
            ));
        }
        upsert_execution_plan_state(
            &conn,
            ExecutionPlanUpdate {
                board_id: &board_id,
                task_id: &task.id,
                state: StepPlanState::NotRequired,
                reason: Some(reason),
                actor,
                now,
                emit_event: true,
            },
        )?;
        recompute_parent_after_plan_change(&conn, &board_id, actor, &task.id, now)?;
        derive_execution_plan(&conn, &board_id, &task.id)
    })
}

pub(crate) fn count_subtask_cycles(conn: &Connection) -> Result<i64> {
    let mut stmt = conn
        .prepare("SELECT parent_task_id, child_task_id FROM task_subtasks")
        .map_err(storage)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(storage)?;
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    let mut nodes: HashSet<String> = HashSet::new();
    for row in rows {
        let (parent, child) = row.map_err(storage)?;
        nodes.insert(parent.clone());
        nodes.insert(child.clone());
        graph.entry(parent).or_default().push(child);
    }
    Ok(super::count_cyclic_components(&nodes, &graph))
}

fn recompute_parent_after_plan_change(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    task_id: &str,
    now: i64,
) -> Result<()> {
    let task = get_task_by_id(conn, board_id, task_id)?;
    if !is_active_recomputable_status(task.status) {
        return Ok(());
    }
    let target = recompute_ready_status(conn, &task, now)?;
    if target != task.status {
        guarded_set_status(conn, board_id, &task, target, actor, "task.recomputed", now)?;
    }
    Ok(())
}

fn ensure_subtask_relation_allowed(
    conn: &Connection,
    board_id: &str,
    parent: &TaskRecord,
    child: &TaskRecord,
) -> Result<()> {
    if parent.id == child.id {
        return Err(KanbanError::InvalidInput(
            "task cannot be its own subtask".into(),
        ));
    }
    ensure_subtask_endpoint(parent, board_id, "parent")?;
    ensure_subtask_endpoint(child, board_id, "child")?;
    if has_subtask_path(conn, &child.id, &parent.id)? {
        return Err(KanbanError::InvalidInput("subtask cycle detected".into()));
    }
    Ok(())
}

fn ensure_subtask_endpoint(task: &TaskRecord, board_id: &str, role: &str) -> Result<()> {
    if task.board_id != board_id {
        return Err(KanbanError::InvalidInput(format!(
            "cross-board subtask {role} is not allowed"
        )));
    }
    if task.status == TaskStatus::Archived || task.archived_at.is_some() {
        return Err(KanbanError::InvalidInput(format!(
            "archived subtask {role} is not allowed"
        )));
    }
    Ok(())
}

fn next_subtask_position(conn: &Connection, parent_task_id: &str) -> Result<i64> {
    scalar(
        conn,
        "SELECT COALESCE(MAX(position), 0) + 1024 FROM task_subtasks WHERE parent_task_id=?1",
        params![parent_task_id],
        |row| row.get(0),
    )
}

fn subtask_record(
    conn: &Connection,
    board_id: &str,
    parent_task_id: &str,
    child_task_id: &str,
) -> Result<TaskSubtaskRecord> {
    let (position, required, created_by, created_at): (i64, i64, String, i64) = conn
        .query_row(
            "SELECT position, required, created_by, created_at FROM task_subtasks WHERE board_id=?1 AND parent_task_id=?2 AND child_task_id=?3",
            params![board_id, parent_task_id, child_task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(storage)?
        .ok_or_else(|| KanbanError::NotFound(format!("subtask {parent_task_id}->{child_task_id}")))?;
    Ok(TaskSubtaskRecord {
        parent_task_id: parent_task_id.to_owned(),
        child_task: get_task_by_id(conn, board_id, child_task_id)?,
        position,
        required: required != 0,
        created_by,
        created_at,
    })
}

fn derive_execution_plan(
    conn: &Connection,
    board_id: &str,
    task_id: &str,
) -> Result<TaskExecutionPlanRecord> {
    let required_count: i64 = scalar(
        conn,
        "SELECT COUNT(*) FROM task_subtasks WHERE board_id=?1 AND parent_task_id=?2 AND required=1",
        params![board_id, task_id],
        |row| row.get(0),
    )?;
    if required_count > 0 {
        return Ok(TaskExecutionPlanRecord {
            board_id: board_id.to_owned(),
            task_id: task_id.to_owned(),
            state: StepPlanState::Planned,
            reason: None,
            updated_by: "system".to_owned(),
            updated_at: 0,
        });
    }
    let stored = conn
        .query_row(
            "SELECT state, reason, updated_by, updated_at FROM task_execution_plans WHERE board_id=?1 AND task_id=?2",
            params![board_id, task_id],
            |row| {
                let state: String = row.get(0)?;
                Ok((state, row.get::<_, Option<String>>(1)?, row.get::<_, String>(2)?, row.get::<_, i64>(3)?))
            },
        )
        .optional()
        .map_err(storage)?;
    if let Some((state, reason, updated_by, updated_at)) = stored {
        let state = StepPlanState::from_db_str(&state).ok_or_else(|| {
            KanbanError::Storage(format!("invalid execution plan state: {state}"))
        })?;
        if state != StepPlanState::NotRequired {
            return Ok(TaskExecutionPlanRecord {
                board_id: board_id.to_owned(),
                task_id: task_id.to_owned(),
                state: StepPlanState::Unplanned,
                reason: None,
                updated_by: "system".to_owned(),
                updated_at: 0,
            });
        }
        return Ok(TaskExecutionPlanRecord {
            board_id: board_id.to_owned(),
            task_id: task_id.to_owned(),
            state,
            reason,
            updated_by,
            updated_at,
        });
    }
    Ok(TaskExecutionPlanRecord {
        board_id: board_id.to_owned(),
        task_id: task_id.to_owned(),
        state: StepPlanState::Unplanned,
        reason: None,
        updated_by: "system".to_owned(),
        updated_at: 0,
    })
}

fn upsert_execution_plan_state(conn: &Connection, update: ExecutionPlanUpdate<'_>) -> Result<()> {
    let previous = conn
        .query_row(
            "SELECT state FROM task_execution_plans WHERE board_id=?1 AND task_id=?2",
            params![update.board_id, update.task_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage)?;
    conn.execute(
        "INSERT INTO task_execution_plans(board_id,task_id,state,reason,updated_by,updated_at) VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(task_id) DO UPDATE SET board_id=excluded.board_id,state=excluded.state,reason=excluded.reason,updated_by=excluded.updated_by,updated_at=excluded.updated_at",
        params![
            update.board_id,
            update.task_id,
            update.state.as_str(),
            update.reason,
            update.actor,
            update.now
        ],
    )
    .map_err(storage)?;
    if update.emit_event && previous.as_deref() != Some(update.state.as_str()) {
        let kind = match update.state {
            StepPlanState::Planned => "task.execution_plan.planned",
            StepPlanState::NotRequired => "task.execution_plan.not_required",
            StepPlanState::Unplanned => "task.execution_plan.unplanned",
        };
        insert_event(
            conn,
            update.board_id,
            Some(update.task_id),
            None,
            kind,
            update.actor,
            &json!({ "state": update.state.as_str() }).to_string(),
            update.now,
        )?;
    }
    Ok(())
}

fn insert_subtask_event(
    conn: &Connection,
    board_id: &str,
    parent_task_id: &str,
    kind: &str,
    actor: &str,
    record: &TaskSubtaskRecord,
    now: i64,
) -> Result<()> {
    insert_event(
        conn,
        board_id,
        Some(parent_task_id),
        None,
        kind,
        actor,
        &json!({
            "child_task_id": record.child_task.id,
            "position": record.position,
            "required": record.required,
        })
        .to_string(),
        now,
    )
}

fn has_subtask_path(conn: &Connection, start: &str, goal: &str) -> Result<bool> {
    conn.query_row(
        "WITH RECURSIVE walk(id) AS (\n           SELECT child_task_id FROM task_subtasks WHERE parent_task_id=?1\n           UNION\n           SELECT s.child_task_id FROM task_subtasks s JOIN walk w ON s.parent_task_id=w.id\n         ) SELECT EXISTS(SELECT 1 FROM walk WHERE id=?2)",
        params![start, goal],
        |row| row.get(0),
    )
    .map_err(storage)
}

fn bool_to_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
}
