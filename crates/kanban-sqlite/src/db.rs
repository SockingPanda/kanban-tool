use std::path::{Path, PathBuf};

use kanban_core::{KanbanError, Result};
use rusqlite::Connection;

pub fn connect_file(path: &Path) -> Result<Connection> {
    let lock_path = maintenance_lock_path(path);
    if lock_path.exists() {
        return Err(KanbanError::InvalidInput(format!(
            "database is locked for maintenance: {}",
            path.display()
        )));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| KanbanError::Storage(err.to_string()))?;
    }
    let conn = Connection::open(path).map_err(|err| KanbanError::Storage(err.to_string()))?;
    default_pragmas(&conn)?;
    Ok(conn)
}

pub fn connect(path: impl AsRef<Path>) -> Result<Connection> {
    connect_file(path.as_ref())
}

pub fn default_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 120000;
PRAGMA synchronous = NORMAL;
PRAGMA temp_store = MEMORY;",
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))
}

pub fn maintenance_lock_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.maintenance.lock", path.display()))
}
