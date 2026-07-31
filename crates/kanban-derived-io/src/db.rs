use std::{
    fs,
    path::{Path, PathBuf},
};

use kanban_core::{KanbanError, Result};
use kanban_local::DerivedStoreWriteGuard;
use kanban_local::sqlite_connection::{
    DatabaseLifecycleSharedConnectionOpenError, open_database_with_shared_lifecycle,
};
use rusqlite::{Connection, OptionalExtension};

pub fn connect_file(
    path: impl AsRef<Path>,
) -> Result<kanban_local::sqlite_connection::DatabaseLifecycleSharedConnection> {
    let path = path.as_ref();
    let lock_path = maintenance_lock_path(path);
    if maintenance_lock_blocks(&lock_path)? {
        return Err(KanbanError::InvalidInput(format!(
            "database is locked for maintenance: {}",
            path.display()
        )));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(storage)?;
    }
    let conn = open_database_with_shared_lifecycle(path, |guarded_path| {
        let lock_path = maintenance_lock_path(guarded_path);
        if maintenance_lock_blocks(&lock_path).map_err(io_storage)? {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                format!(
                    "database is locked for maintenance: {}",
                    guarded_path.display()
                ),
            ));
        }
        Ok(())
    })
    .map_err(lifecycle_connection_error)?;
    default_pragmas(&conn)?;
    Ok(conn)
}

fn lifecycle_connection_error(error: DatabaseLifecycleSharedConnectionOpenError) -> KanbanError {
    match error {
        DatabaseLifecycleSharedConnectionOpenError::BeforeOpen(error)
            if error.kind() == std::io::ErrorKind::WouldBlock =>
        {
            KanbanError::InvalidInput(error.to_string())
        }
        DatabaseLifecycleSharedConnectionOpenError::BeforeOpen(error) => storage(error),
        DatabaseLifecycleSharedConnectionOpenError::Lifecycle(error)
            if error.kind() == std::io::ErrorKind::WouldBlock =>
        {
            KanbanError::Conflict(error.to_string())
        }
        DatabaseLifecycleSharedConnectionOpenError::Lifecycle(error) => storage(error),
        DatabaseLifecycleSharedConnectionOpenError::SQLite(error) => storage(error),
    }
}

fn io_storage(error: KanbanError) -> std::io::Error {
    std::io::Error::other(error.to_string())
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

pub fn maintenance_lock_path(path: &Path) -> PathBuf {
    kanban_local::database_maintenance_lock_path(path)
}

pub fn maintenance_lock_blocks(lock_path: &Path) -> Result<bool> {
    if !lock_path.exists() {
        return Ok(false);
    }
    if lock_is_stale(lock_path) {
        fs::remove_file(lock_path).map_err(storage)?;
        return Ok(false);
    }
    Ok(true)
}

pub fn acquire_derived_store_write_guard(
    path: &Path,
    store_name: &str,
) -> Result<DerivedStoreWriteGuard> {
    DerivedStoreWriteGuard::acquire(path, store_name).map_err(|error| {
        if error.kind() == std::io::ErrorKind::WouldBlock {
            KanbanError::Conflict(error.to_string())
        } else {
            storage(error)
        }
    })
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

fn lock_is_stale(lock_path: &Path) -> bool {
    let Ok(content) = fs::read_to_string(lock_path) else {
        return false;
    };
    let Some(pid) = content
        .lines()
        .find_map(|line| line.strip_prefix("pid="))
        .and_then(|pid| pid.trim().parse::<u32>().ok())
    else {
        return false;
    };
    !process_is_alive(pid)
}

#[cfg(target_os = "linux")]
fn process_is_alive(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

#[cfg(all(unix, not(target_os = "linux")))]
fn process_is_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|status| status.success())
        .unwrap_or(true)
}

#[cfg(windows)]
fn process_is_alive(pid: u32) -> bool {
    let filter = format!("PID eq {pid}");
    std::process::Command::new("tasklist")
        .args(["/FI", &filter])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
        .unwrap_or(true)
}

#[cfg(not(any(unix, windows)))]
fn process_is_alive(_pid: u32) -> bool {
    true
}
