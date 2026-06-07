use std::{
    fs,
    path::{Path, PathBuf},
};

use kanban_core::{KanbanError, Result};
use rusqlite::Connection;

pub fn connect_file(path: &Path) -> Result<Connection> {
    let lock_path = maintenance_lock_path(path);
    if maintenance_lock_blocks(&lock_path)? {
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
    let normalized = normalized_database_path(path);
    PathBuf::from(format!("{}.maintenance.lock", normalized.display()))
}

pub fn runtime_lock_path(path: &Path) -> PathBuf {
    let normalized = normalized_database_path(path);
    PathBuf::from(format!("{}.runtime.lock", normalized.display()))
}

pub fn maintenance_lock_blocks(lock_path: &Path) -> Result<bool> {
    lock_file_blocks(lock_path)
}

pub fn runtime_lock_blocks(lock_path: &Path) -> Result<bool> {
    lock_file_blocks(lock_path)
}

fn lock_file_blocks(lock_path: &Path) -> Result<bool> {
    if !lock_path.exists() {
        return Ok(false);
    }
    if lock_is_stale(lock_path) {
        fs::remove_file(lock_path).map_err(|err| KanbanError::Storage(err.to_string()))?;
        return Ok(false);
    }
    Ok(true)
}

fn normalized_database_path(path: &Path) -> PathBuf {
    if path.exists()
        && let Ok(canonical) = fs::canonicalize(path)
    {
        return canonical;
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().unwrap_or_default();
    if let Ok(canonical_parent) = fs::canonicalize(parent) {
        return canonical_parent.join(file_name);
    }
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
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
