use kanban_core::{KanbanError, Result};
use rusqlite::Connection;

pub(super) fn with_immediate_tx<T>(conn: &Connection, f: impl FnOnce() -> Result<T>) -> Result<T> {
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

pub(super) fn with_read_tx<T>(conn: &Connection, f: impl FnOnce() -> Result<T>) -> Result<T> {
    conn.execute_batch("BEGIN").map_err(storage)?;
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

pub(super) fn storage(err: rusqlite::Error) -> KanbanError {
    KanbanError::Storage(err.to_string())
}
