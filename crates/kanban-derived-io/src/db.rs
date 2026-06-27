use std::{fs, path::Path};

use kanban_core::{KanbanError, Result};
use rusqlite::{Connection, OptionalExtension};

pub fn connect_file(path: impl AsRef<Path>) -> Result<Connection> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(storage)?;
    }
    let conn = Connection::open(path).map_err(storage)?;
    default_pragmas(&conn)?;
    Ok(conn)
}

pub fn default_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 120000;
         PRAGMA synchronous = NORMAL;
         PRAGMA temp_store = MEMORY;",
    )
    .map_err(storage)
}

pub fn board_id(conn: &Connection, slug_or_id: &str) -> Result<String> {
    let sql = if slug_or_id.starts_with("b_") {
        "SELECT id FROM boards WHERE id=?1 AND archived_at IS NULL"
    } else {
        "SELECT id FROM boards WHERE slug=?1 AND archived_at IS NULL"
    };
    conn.query_row(sql, [slug_or_id], |row| row.get(0))
        .optional()
        .map_err(storage)?
        .ok_or_else(|| KanbanError::NotFound(format!("board {slug_or_id}")))
}

pub fn current_last_event_id(conn: &Connection, board_id: &str) -> Result<Option<i64>> {
    conn.query_row(
        "SELECT MAX(id) FROM task_events WHERE board_id=?1",
        [board_id],
        |row| row.get(0),
    )
    .map_err(storage)
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

pub(crate) fn storage(error: impl std::fmt::Display) -> KanbanError {
    KanbanError::Storage(error.to_string())
}
