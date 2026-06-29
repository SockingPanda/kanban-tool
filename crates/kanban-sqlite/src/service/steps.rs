use crate::connect_file;

use super::{
    CreateStepInput, StepPlanState, StepStatus, TaskExecutionPlanRecord, TaskRecord,
    TaskStepRecord, UpdateStepInput, all, board_id, ensure_board_active, get_task_by_id,
    guarded_set_status, insert_event, recompute_ready_status, resolve_task, scalar, storage,
    with_immediate_tx,
};

use std::path::Path;

use kanban_core::{
    Clock, KanbanError, Result, SystemClock, TaskStatus, is_active_recomputable_status,
    new_typed_id,
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

struct StepResolution<'a> {
    status: StepStatus,
    note: &'a str,
    event_kind: &'a str,
}

pub fn create_step(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    input: CreateStepInput,
) -> Result<TaskStepRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let title = normalize_step_title(input.title)?;
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        let parent = resolve_task(&conn, &board_id, parent_ref)?;
        ensure_step_task(&parent, &board_id, "parent")?;
        let linked_task_id = match input.linked_task_ref {
            Some(link_ref) => {
                Some(resolve_linked_task(&conn, &board_id, &parent.id, &link_ref)?.id)
            }
            None => None,
        };
        let position = input
            .position
            .map(Ok)
            .unwrap_or_else(|| next_step_position(&conn, &parent.id))?;
        let step_id = new_typed_id("step");
        conn.execute(
            "INSERT INTO task_steps(id,board_id,parent_task_id,position,title,body,linked_task_id,required,status,created_by,created_at,updated_by,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'todo',?9,?10,?9,?10)",
            params![step_id, board_id, parent.id, position, title, input.body, linked_task_id, bool_to_i64(input.required), actor, now],
        )
        .map_err(storage)?;
        let record = step_record(&conn, &board_id, &parent.id, &step_id)?;
        insert_step_event(
            &conn,
            &board_id,
            &parent.id,
            "task.step.created",
            actor,
            &record,
            now,
        )?;
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
        Ok(record)
    })
}

pub fn list_steps(
    path: impl AsRef<Path>,
    board: &str,
    parent_ref: &str,
) -> Result<Vec<TaskStepRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let parent = resolve_task(&conn, &board_id, parent_ref)?;
    all(
        &conn,
        "SELECT id FROM task_steps WHERE board_id=?1 AND parent_task_id=?2 ORDER BY position ASC, created_at ASC, id ASC",
        params![board_id, parent.id],
        |row| row.get::<_, String>(0),
    )?
    .into_iter()
    .map(|step_id| step_record(&conn, &board_id, &parent.id, &step_id))
    .collect()
}

pub fn update_step(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    step_ref: &str,
    input: UpdateStepInput,
) -> Result<TaskStepRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        let parent = resolve_task(&conn, &board_id, parent_ref)?;
        let step_id = resolve_step_id(&conn, &board_id, &parent.id, step_ref)?;
        let existing = step_record(&conn, &board_id, &parent.id, &step_id)?;
        let title = match input.title {
            Some(title) => normalize_step_title(title)?,
            None => existing.title,
        };
        let body = input.body.unwrap_or(existing.body);
        let position = input.position.unwrap_or(existing.position);
        let required = input.required.unwrap_or(existing.required);
        let linked_task_id = if input.unlink_task {
            None
        } else if let Some(link_ref) = input.linked_task_ref {
            Some(resolve_linked_task(&conn, &board_id, &parent.id, &link_ref)?.id)
        } else {
            existing.linked_task.map(|task| task.id)
        };
        conn.execute(
            "UPDATE task_steps SET title=?1, body=?2, linked_task_id=?3, position=?4, required=?5, updated_by=?6, updated_at=?7 WHERE board_id=?8 AND parent_task_id=?9 AND id=?10",
            params![title, body, linked_task_id, position, bool_to_i64(required), actor, now, board_id, parent.id, step_id],
        )
        .map_err(storage)?;
        let updated = step_record(&conn, &board_id, &parent.id, &step_id)?;
        insert_step_event(
            &conn,
            &board_id,
            &parent.id,
            "task.step.updated",
            actor,
            &updated,
            now,
        )?;
        if input.required.is_some() {
            recompute_parent_after_plan_change(&conn, &board_id, actor, &parent.id, now)?;
        }
        Ok(updated)
    })
}

pub fn remove_step(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    step_ref: &str,
) -> Result<TaskStepRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        let parent = resolve_task(&conn, &board_id, parent_ref)?;
        let step_id = resolve_step_id(&conn, &board_id, &parent.id, step_ref)?;
        let existing = step_record(&conn, &board_id, &parent.id, &step_id)?;
        conn.execute(
            "DELETE FROM task_steps WHERE board_id=?1 AND parent_task_id=?2 AND id=?3",
            params![board_id, parent.id, step_id],
        )
        .map_err(storage)?;
        insert_step_event(
            &conn,
            &board_id,
            &parent.id,
            "task.step.removed",
            actor,
            &existing,
            now,
        )?;
        recompute_parent_after_plan_change(&conn, &board_id, actor, &parent.id, now)?;
        Ok(existing)
    })
}

pub fn complete_step(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    step_ref: &str,
    note: &str,
) -> Result<TaskStepRecord> {
    resolve_step(
        path,
        board,
        actor,
        parent_ref,
        step_ref,
        StepResolution {
            status: StepStatus::Done,
            note,
            event_kind: "task.step.done",
        },
    )
}

pub fn skip_step(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    step_ref: &str,
    reason: &str,
) -> Result<TaskStepRecord> {
    resolve_step(
        path,
        board,
        actor,
        parent_ref,
        step_ref,
        StepResolution {
            status: StepStatus::Skipped,
            note: reason,
            event_kind: "task.step.skipped",
        },
    )
}

pub fn reopen_step(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    step_ref: &str,
    reason: &str,
) -> Result<TaskStepRecord> {
    resolve_step(
        path,
        board,
        actor,
        parent_ref,
        step_ref,
        StepResolution {
            status: StepStatus::Todo,
            note: reason,
            event_kind: "task.step.reopened",
        },
    )
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
        ensure_step_task(&task, &board_id, "task")?;
        let step_count: i64 = scalar(
            &conn,
            "SELECT COUNT(*) FROM task_steps WHERE board_id=?1 AND parent_task_id=?2",
            params![board_id, task.id],
            |row| row.get(0),
        )?;
        if step_count > 0 {
            return Err(KanbanError::InvalidInput(
                "cannot mark execution plan not_required while steps exist".into(),
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

fn resolve_step(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    step_ref: &str,
    resolution: StepResolution<'_>,
) -> Result<TaskStepRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let note = resolution.note.trim().to_owned();
    if note.is_empty() {
        return Err(KanbanError::InvalidInput(
            "step resolution note/reason is required".into(),
        ));
    }
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        let parent = resolve_task(&conn, &board_id, parent_ref)?;
        let step_id = resolve_step_id(&conn, &board_id, &parent.id, step_ref)?;
        let (resolution_note, resolved_by, resolved_at) = if resolution.status == StepStatus::Todo {
            (None, None, None)
        } else {
            (Some(note), Some(actor.to_owned()), Some(now))
        };
        conn.execute(
            "UPDATE task_steps SET status=?1, resolution_note=?2, resolved_by=?3, resolved_at=?4, updated_by=?5, updated_at=?6 WHERE board_id=?7 AND parent_task_id=?8 AND id=?9",
            params![resolution.status.as_str(), resolution_note, resolved_by, resolved_at, actor, now, board_id, parent.id, step_id],
        )
        .map_err(storage)?;
        let updated = step_record(&conn, &board_id, &parent.id, &step_id)?;
        insert_step_event(
            &conn,
            &board_id,
            &parent.id,
            resolution.event_kind,
            actor,
            &updated,
            now,
        )?;
        Ok(updated)
    })
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

fn ensure_step_task(task: &TaskRecord, board_id: &str, role: &str) -> Result<()> {
    if task.board_id != board_id {
        return Err(KanbanError::InvalidInput(format!(
            "cross-board step {role} is not allowed"
        )));
    }
    if task.status == TaskStatus::Archived || task.archived_at.is_some() {
        return Err(KanbanError::InvalidInput(format!(
            "archived step {role} is not allowed"
        )));
    }
    Ok(())
}

fn resolve_linked_task(
    conn: &Connection,
    board_id: &str,
    parent_task_id: &str,
    linked_ref: &str,
) -> Result<TaskRecord> {
    let linked = resolve_task(conn, board_id, linked_ref)?;
    if linked.id == parent_task_id {
        return Err(KanbanError::InvalidInput(
            "step cannot link to its parent task".into(),
        ));
    }
    ensure_step_task(&linked, board_id, "linked task")?;
    Ok(linked)
}

fn next_step_position(conn: &Connection, parent_task_id: &str) -> Result<i64> {
    scalar(
        conn,
        "SELECT COALESCE(MAX(position), 0) + 1024 FROM task_steps WHERE parent_task_id=?1",
        params![parent_task_id],
        |row| row.get(0),
    )
}

fn resolve_step_id(
    conn: &Connection,
    board_id: &str,
    parent_task_id: &str,
    step_ref: &str,
) -> Result<String> {
    let trimmed = step_ref.trim();
    if trimmed.len() > 1
        && matches!(trimmed.as_bytes()[0], b's' | b'S')
        && let Ok(ordinal) = trimmed[1..].parse::<usize>()
    {
        if ordinal == 0 {
            return Err(KanbanError::InvalidInput(
                "step ordinal starts at S1".into(),
            ));
        }
        return conn
            .query_row(
                "SELECT id FROM task_steps WHERE board_id=?1 AND parent_task_id=?2 ORDER BY position ASC, created_at ASC, id ASC LIMIT 1 OFFSET ?3",
                params![board_id, parent_task_id, (ordinal - 1) as i64],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage)?
            .ok_or_else(|| KanbanError::NotFound(format!("step {trimmed}")));
    }
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM task_steps WHERE board_id=?1 AND parent_task_id=?2 AND id=?3)",
            params![board_id, parent_task_id, trimmed],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if exists {
        Ok(trimmed.to_owned())
    } else {
        Err(KanbanError::NotFound(format!("step {trimmed}")))
    }
}

fn step_record(
    conn: &Connection,
    board_id: &str,
    parent_task_id: &str,
    step_id: &str,
) -> Result<TaskStepRecord> {
    let row = conn
        .query_row(
            "SELECT id, position, title, body, linked_task_id, required, status, resolution_note, resolved_by, resolved_at, created_by, created_at, updated_by, updated_at FROM task_steps WHERE board_id=?1 AND parent_task_id=?2 AND id=?3",
            params![board_id, parent_task_id, step_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<i64>>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, i64>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, i64>(13)?,
                ))
            },
        )
        .optional()
        .map_err(storage)?
        .ok_or_else(|| KanbanError::NotFound(format!("step {step_id}")))?;
    let (
        id,
        position,
        title,
        body,
        linked_task_id,
        required,
        status,
        resolution_note,
        resolved_by,
        resolved_at,
        created_by,
        created_at,
        updated_by,
        updated_at,
    ) = row;
    let linked_task = linked_task_id
        .as_deref()
        .map(|task_id| get_task_by_id(conn, board_id, task_id))
        .transpose()?;
    let status = StepStatus::from_db_str(&status)
        .ok_or_else(|| KanbanError::Storage(format!("invalid step status: {status}")))?;
    Ok(TaskStepRecord {
        id,
        parent_task_id: parent_task_id.to_owned(),
        title,
        body,
        linked_task,
        position,
        required: required != 0,
        status,
        resolution_note,
        resolved_by,
        resolved_at,
        created_by,
        created_at,
        updated_by,
        updated_at,
    })
}

fn derive_execution_plan(
    conn: &Connection,
    board_id: &str,
    task_id: &str,
) -> Result<TaskExecutionPlanRecord> {
    let step_count: i64 = scalar(
        conn,
        "SELECT COUNT(*) FROM task_steps WHERE board_id=?1 AND parent_task_id=?2",
        params![board_id, task_id],
        |row| row.get(0),
    )?;
    if step_count > 0 {
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

fn insert_step_event(
    conn: &Connection,
    board_id: &str,
    parent_task_id: &str,
    kind: &str,
    actor: &str,
    record: &TaskStepRecord,
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
            "step_id": record.id,
            "linked_task_id": record.linked_task.as_ref().map(|task| task.id.as_str()),
            "position": record.position,
            "required": record.required,
            "status": record.status.as_str(),
        })
        .to_string(),
        now,
    )
}

fn normalize_step_title(title: String) -> Result<String> {
    let title = title.trim().to_owned();
    if title.is_empty() {
        return Err(KanbanError::InvalidInput("step title is required".into()));
    }
    Ok(title)
}

fn bool_to_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
}
