use crate::connect_file;

use super::{
    EventListOptions, EventRecord, SqlFilter, all_values, board_id_any, enqueue_index_outbox,
    exec_named, resolve_task_any, scalar, upsert_board_entity, upsert_event_entity,
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
