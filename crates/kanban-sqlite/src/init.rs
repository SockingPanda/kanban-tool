use std::path::{Path, PathBuf};

use kanban_core::{Clock, KanbanError, Result, SystemClock, new_board_id};
use rusqlite::{Connection, OptionalExtension, params};

use serde::{Deserialize, Serialize};

use crate::connect_file;

const INITIAL_MIGRATION: &str = include_str!("../../../migrations/001_initial.sql");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitResult {
    pub db_path: PathBuf,
    pub board_id: String,
    pub board_slug: String,
}

pub fn init_database(path: impl AsRef<Path>, actor: &str) -> Result<InitResult> {
    let path = path.as_ref();
    let conn = connect_file(path)?;
    conn.execute_batch(INITIAL_MIGRATION)
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    ensure_default_board(&conn, actor, SystemClock.now_ms())?;
    let board_id = default_board_id(&conn)?;
    ensure_default_columns(&conn, &board_id, SystemClock.now_ms())?;
    Ok(InitResult {
        db_path: path.to_path_buf(),
        board_id,
        board_slug: "default".to_owned(),
    })
}

fn ensure_default_board(conn: &Connection, actor: &str, now_ms: i64) -> Result<()> {
    let existing: Option<String> = conn
        .query_row("SELECT id FROM boards WHERE slug = 'default'", [], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    if existing.is_some() {
        return Ok(());
    }

    let board_id = new_board_id();
    conn.execute(
        "INSERT INTO boards(id, slug, name, description, created_at, updated_at, archived_at) VALUES (?1, 'default', 'Default', NULL, ?2, ?2, NULL)",
        params![board_id, now_ms],
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    conn.execute(
        "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, ?2, NULL, NULL, 'board.created', ?3, '{}', ?4)",
        params![kanban_core::new_event_id(), board_id, actor, now_ms],
    )
    .map_err(|err| KanbanError::Storage(err.to_string()))?;
    Ok(())
}

fn default_board_id(conn: &Connection) -> Result<String> {
    conn.query_row("SELECT id FROM boards WHERE slug = 'default'", [], |row| {
        row.get(0)
    })
    .map_err(|err| KanbanError::Storage(err.to_string()))
}

fn ensure_default_columns(conn: &Connection, board_id: &str, now_ms: i64) -> Result<()> {
    let defaults = [
        ("triage", "Triage", 10, 0),
        ("todo", "Todo", 20, 0),
        ("scheduled", "Scheduled", 30, 0),
        ("ready", "Ready", 40, 0),
        ("running", "Running", 50, 0),
        ("blocked", "Blocked", 60, 0),
        ("review", "Review", 70, 0),
        ("done", "Done", 80, 0),
        ("archived", "Archived", 90, 1),
    ];
    for (status, title, position, hidden) in defaults {
        let id = format!("col_{}_{}", board_id.trim_start_matches("b_"), status);
        conn.execute(
            "INSERT OR IGNORE INTO board_columns(id, board_id, status, title, position, hidden, wip_limit, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?7)",
            params![id, board_id, status, title, position, hidden, now_ms],
        )
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    }
    Ok(())
}
