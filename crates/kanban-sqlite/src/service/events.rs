use crate::db::connect_file;

use super::{
    EventListOptions, EventRecord, SqlFilter, all_values, board_id_any, enqueue_index_outbox,
    exec_named, resolve_task_any, scalar, storage, upsert_board_entity, upsert_event_entity,
    upsert_run_entity, upsert_task_entity,
};

use std::path::Path;

use kanban_core::{Result, new_event_id};

use rusqlite::{Connection, Row, named_params, params, types::Value};

pub fn list_events(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: Option<&str>,
) -> Result<Vec<EventRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id_any(&conn, board)?;
    let task_id = task_ref
        .map(|r| resolve_task_any(&conn, &board_id, r).map(|t| t.id))
        .transpose()?;
    let mut filter = SqlFilter::new();
    filter.and("board_id=?", board_id)?;
    if let Some(task_id) = task_id {
        filter.and("task_id=?", task_id)?;
    }
    let sql = format!(
        "SELECT id,event_id,task_id,run_id,kind,actor,payload_json,created_at FROM task_events {} ORDER BY id ASC",
        filter.where_sql()
    );
    all_values(&conn, &sql, filter.params(), event_from_row)
}

pub fn list_events_after(
    path: impl AsRef<Path>,
    board: &str,
    options: EventListOptions,
) -> Result<Vec<EventRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id_any(&conn, board)?;
    let task_id = options
        .task_ref
        .as_deref()
        .map(|r| resolve_task_any(&conn, &board_id, r).map(|t| t.id))
        .transpose()?;
    let mut filter = SqlFilter::new();
    filter.and("board_id=?", board_id)?;
    filter.and("id>?", options.after)?;
    if let Some(task_id) = task_id {
        filter.and("task_id=?", task_id)?;
    }
    let mut params = filter.params().to_vec();
    params.push(Value::Integer(options.limit as i64));
    let sql = format!(
        "SELECT id,event_id,task_id,run_id,kind,actor,payload_json,created_at FROM task_events {} ORDER BY id ASC LIMIT ?",
        filter.where_sql()
    );
    all_values(&conn, &sql, &params, event_from_row)
}

pub(crate) fn current_last_event_id(conn: &Connection, board_id: &str) -> Result<Option<i64>> {
    scalar(
        conn,
        "SELECT MAX(id) FROM task_events WHERE board_id=?1",
        params![board_id],
        |row| row.get(0),
    )
}

pub(crate) fn search_lag(
    current_last_event_id: Option<i64>,
    indexed_last_event_id: Option<i64>,
) -> i64 {
    match (current_last_event_id, indexed_last_event_id) {
        (Some(current), Some(indexed)) => current.abs_diff(indexed).try_into().unwrap_or(i64::MAX),
        (Some(current), None) => current,
        _ => 0,
    }
}

pub(crate) fn event_from_row(row: &Row<'_>) -> rusqlite::Result<EventRecord> {
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn insert_event(
    conn: &Connection,
    board_id: &str,
    task_id: Option<&str>,
    run_id: Option<&str>,
    kind: &str,
    actor: &str,
    payload: &str,
    now: i64,
) -> Result<()> {
    let event_id = new_event_id();
    exec_named(
        conn,
        "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, :run_id, :kind, :actor, :payload_json, :created_at)",
        named_params! {
            ":event_id": event_id,
            ":board_id": board_id,
            ":task_id": task_id,
            ":run_id": run_id,
            ":kind": kind,
            ":actor": actor,
            ":payload_json": payload,
            ":created_at": now,
        },
    )?;
    let source_event_id = conn.last_insert_rowid();
    if let Some(task_id) = task_id {
        touch_running_task_lease_from_activity_event(conn, board_id, task_id, kind, now)?;
    }
    upsert_board_entity(conn, board_id)?;
    upsert_event_entity(conn, &event_id, board_id, task_id, kind, payload, now)?;
    if let Some(task_id) = task_id {
        upsert_task_entity(conn, task_id)?;
    }
    if let Some(run_id) = run_id {
        upsert_run_entity(conn, run_id)?;
    }
    let entity_uri = task_id
        .map(|task_id| format!("kb://task/{task_id}"))
        .or_else(|| run_id.map(|run_id| format!("kb://run/{run_id}")))
        .unwrap_or_else(|| format!("kb://board/{board_id}"));
    enqueue_index_outbox(conn, source_event_id, &entity_uri, "upsert", now)?;
    Ok(())
}

fn touch_running_task_lease_from_activity_event(
    conn: &Connection,
    board_id: &str,
    task_id: &str,
    kind: &str,
    now: i64,
) -> Result<()> {
    if kind == "task.heartbeat" {
        return Ok(());
    }
    let changed = exec_named(
        conn,
        "UPDATE tasks
         SET claim_expires_at=:now + MAX(1, claim_expires_at - COALESCE(last_heartbeat_at, :now)),
             last_heartbeat_at=:now,
             updated_at=:now,
             lock_version=lock_version+1
         WHERE board_id=:board_id
           AND id=:task_id
           AND status='running'
           AND claim_token IS NOT NULL
           AND claim_expires_at IS NOT NULL
           AND current_run_id IS NOT NULL
           AND EXISTS (
               SELECT 1 FROM task_runs r
               WHERE r.id=tasks.current_run_id
                 AND r.board_id=tasks.board_id
                 AND r.task_id=tasks.id
                 AND r.status='running'
                 AND r.claim_token=tasks.claim_token
           )",
        named_params! {
            ":now": now,
            ":board_id": board_id,
            ":task_id": task_id,
        },
    )?;
    if changed == 0 {
        return Ok(());
    }
    conn.execute(
        "UPDATE task_runs
         SET claim_expires_at=(SELECT claim_expires_at FROM tasks WHERE board_id=?1 AND id=?2),
             last_heartbeat_at=?3
         WHERE id=(SELECT current_run_id FROM tasks WHERE board_id=?1 AND id=?2)
           AND board_id=?1
           AND task_id=?2
           AND status='running'
           AND claim_token=(SELECT claim_token FROM tasks WHERE board_id=?1 AND id=?2)",
        params![board_id, task_id, now],
    )
    .map_err(storage)?;
    Ok(())
}
