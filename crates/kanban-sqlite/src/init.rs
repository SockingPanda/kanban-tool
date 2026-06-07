use std::path::{Path, PathBuf};

use kanban_core::{Clock, KanbanError, Result, SystemClock, new_board_id};
use rusqlite::{Connection, OptionalExtension, params};

use serde::{Deserialize, Serialize};

use crate::connect_file;

const INITIAL_MIGRATION: &str = include_str!("../../../migrations/001_initial.sql");
const INITIAL_MIGRATION_VERSION: i64 = 1;
const INITIAL_MIGRATION_NAME: &str = "001_initial";

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
    validate_initial_migration(&conn)?;
    ensure_default_board(&conn, actor, SystemClock.now_ms())?;
    let board_id = default_board_id(&conn)?;
    ensure_default_columns(&conn, &board_id, SystemClock.now_ms())?;
    Ok(InitResult {
        db_path: path.to_path_buf(),
        board_id,
        board_slug: "default".to_owned(),
    })
}

fn validate_initial_migration(conn: &Connection) -> Result<()> {
    ensure_schema_migrations_shape(conn)?;
    let checksum = migration_checksum(INITIAL_MIGRATION);
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT name, checksum FROM schema_migrations WHERE version = ?1",
            [INITIAL_MIGRATION_VERSION],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    match row {
        Some((name, _stored)) if name != INITIAL_MIGRATION_NAME => {
            return Err(KanbanError::Storage(format!(
                "migration name mismatch for version {INITIAL_MIGRATION_VERSION}: expected {INITIAL_MIGRATION_NAME}, found {name}"
            )));
        }
        Some((_name, stored)) if stored.is_empty() => {
            conn.execute(
                "UPDATE schema_migrations SET checksum=?1 WHERE version=?2",
                params![checksum, INITIAL_MIGRATION_VERSION],
            )
            .map_err(|err| KanbanError::Storage(err.to_string()))?;
        }
        Some((_name, stored)) if stored != checksum => {
            return Err(KanbanError::Storage(format!(
                "migration checksum mismatch for {INITIAL_MIGRATION_NAME}: expected {checksum}, found {stored}"
            )));
        }
        Some((_name, _stored)) => {}
        None => {
            conn.execute(
                "INSERT INTO schema_migrations(version, name, checksum, applied_at) VALUES (?1, ?2, ?3, ?4)",
                params![
                    INITIAL_MIGRATION_VERSION,
                    INITIAL_MIGRATION_NAME,
                    checksum,
                    SystemClock.now_ms()
                ],
            )
            .map_err(|err| KanbanError::Storage(err.to_string()))?;
        }
    }
    validate_schema_shape(conn)?;
    conn.pragma_update(None, "user_version", INITIAL_MIGRATION_VERSION)
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    Ok(())
}

fn ensure_schema_migrations_shape(conn: &Connection) -> Result<()> {
    if !table_has_column(conn, "schema_migrations", "checksum")? {
        conn.execute(
            "ALTER TABLE schema_migrations ADD COLUMN checksum TEXT NOT NULL DEFAULT ''",
            [],
        )
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    }
    Ok(())
}

fn validate_schema_shape(conn: &Connection) -> Result<()> {
    let required = [
        (
            "boards",
            &["id", "slug", "name", "created_at", "updated_at"][..],
        ),
        (
            "tasks",
            &[
                "id",
                "board_id",
                "seq",
                "title",
                "description",
                "status",
                "claim_token",
                "claim_expires_at",
                "current_run_id",
                "lock_version",
            ][..],
        ),
        (
            "task_dependencies",
            &["board_id", "parent_task_id", "child_task_id"][..],
        ),
        (
            "task_runs",
            &["id", "board_id", "task_id", "status", "claim_token"][..],
        ),
        (
            "task_events",
            &[
                "id",
                "event_id",
                "board_id",
                "task_id",
                "kind",
                "payload_json",
            ][..],
        ),
    ];
    for (table, columns) in required {
        for column in columns {
            if !table_has_column(conn, table, column)? {
                return Err(KanbanError::Storage(format!(
                    "schema validation failed: missing column {table}.{column}"
                )));
            }
        }
    }
    Ok(())
}

fn table_has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|err| KanbanError::Storage(err.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    Ok(columns.iter().any(|name| name == column))
}

fn migration_checksum(sql: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in sql.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv64:{hash:016x}")
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
