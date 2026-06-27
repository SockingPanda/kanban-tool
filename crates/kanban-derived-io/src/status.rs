use kanban_core::{KanbanError, Result};
use kanban_indexer::{
    DERIVED_STORE_SEEDS, DerivedStoreUpdate, OutboxTarget, derived_store_for_name,
};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use crate::storage;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexOutboxRecord {
    pub id: i64,
    pub source_event_id: Option<i64>,
    pub target: String,
    pub entity_uri: String,
    pub action: String,
    pub payload_json: String,
    pub status: String,
    pub attempts: i64,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedStoreStatusRecord {
    pub store_name: String,
    pub schema_version: i64,
    pub last_event_id: i64,
    pub dirty: bool,
    pub last_rebuild_at: Option<i64>,
    pub last_sync_at: Option<i64>,
    pub last_error: Option<String>,
    pub updated_at: i64,
}

pub fn derived_status_by_name(
    conn: &Connection,
    store_name: &str,
) -> Result<DerivedStoreStatusRecord> {
    conn.query_row(
        "SELECT store_name,schema_version,last_event_id,dirty,last_rebuild_at,last_sync_at,last_error,updated_at \
         FROM derived_store_state WHERE store_name=?1",
        [store_name],
        derived_store_status_from_row,
    )
    .optional()
    .map_err(storage)?
    .ok_or_else(|| KanbanError::Storage(format!("missing derived store state: {store_name}")))
}

pub fn mark_derived_store_success(
    conn: &Connection,
    store_name: &str,
    board_id: &str,
    last_event_id: Option<i64>,
    rebuilt: bool,
    now: i64,
) -> Result<()> {
    let target = store_target(store_name)?;
    let seed = derived_store_for_name(store_name)
        .ok_or_else(|| KanbanError::Storage(format!("unknown derived store: {store_name}")))?;
    complete_outbox_for_store(conn, target, board_id, last_event_id, now)?;
    let dirty = has_unfinished_outbox_for_store(conn, target)?;
    let update = DerivedStoreUpdate::success(seed, last_event_id, rebuilt, now);
    conn.execute(
        "INSERT INTO derived_store_state(store_name, schema_version, last_event_id, dirty, last_rebuild_at, last_sync_at, last_error, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT(store_name) DO UPDATE SET last_event_id=MAX(derived_store_state.last_event_id, excluded.last_event_id), dirty=excluded.dirty, last_rebuild_at=COALESCE(excluded.last_rebuild_at, derived_store_state.last_rebuild_at), last_sync_at=COALESCE(excluded.last_sync_at, derived_store_state.last_sync_at), last_error=NULL, updated_at=excluded.updated_at",
        params![
            update.store_name,
            update.schema_version,
            update.last_event_id,
            i64::from(dirty),
            update.last_rebuild_at,
            update.last_sync_at,
            update.last_error,
            update.updated_at
        ],
    )
    .map_err(storage)?;
    Ok(())
}

pub fn mark_derived_store_failure(
    conn: &Connection,
    store_name: &str,
    board_id: &str,
    error: &str,
    now: i64,
) -> Result<()> {
    let target = store_target(store_name)?;
    let seed = derived_store_for_name(store_name)
        .ok_or_else(|| KanbanError::Storage(format!("unknown derived store: {store_name}")))?;
    let update = DerivedStoreUpdate::failure(seed, error, now);
    conn.execute(
        "INSERT INTO derived_store_state(store_name, schema_version, last_event_id, dirty, last_rebuild_at, last_sync_at, last_error, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT(store_name) DO UPDATE SET dirty=1, last_error=excluded.last_error, updated_at=excluded.updated_at",
        params![
            update.store_name,
            update.schema_version,
            update.last_event_id,
            i64::from(update.dirty),
            update.last_rebuild_at,
            update.last_sync_at,
            update.last_error,
            update.updated_at
        ],
    )
    .map_err(storage)?;
    fail_outbox_for_store(conn, target, board_id, error, now)?;
    Ok(())
}

pub fn has_pending_outbox_for_target(
    conn: &Connection,
    target: OutboxTarget,
    board_id: &str,
    last_event_id: Option<i64>,
) -> Result<bool> {
    let target = target.as_str();
    conn.query_row(
        "SELECT EXISTS( \
             SELECT 1 FROM index_outbox o \
             JOIN task_events e ON e.id=o.source_event_id \
             WHERE o.target IN (?1, 'all') \
               AND o.status IN ('pending', 'running', 'failed') \
               AND e.board_id=?2 \
               AND e.id <= ?3 \
         )",
        params![target, board_id, last_event_id.unwrap_or(i64::MAX)],
        |row| row.get::<_, bool>(0),
    )
    .map_err(storage)
}

pub(crate) fn outbox_from_row(row: &Row<'_>) -> rusqlite::Result<IndexOutboxRecord> {
    Ok(IndexOutboxRecord {
        id: row.get(0)?,
        source_event_id: row.get(1)?,
        target: row.get(2)?,
        entity_uri: row.get(3)?,
        action: row.get(4)?,
        payload_json: row.get(5)?,
        status: row.get(6)?,
        attempts: row.get(7)?,
        last_error: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn derived_store_status_from_row(row: &Row<'_>) -> rusqlite::Result<DerivedStoreStatusRecord> {
    let dirty: i64 = row.get(3)?;
    Ok(DerivedStoreStatusRecord {
        store_name: row.get(0)?,
        schema_version: row.get(1)?,
        last_event_id: row.get(2)?,
        dirty: dirty != 0,
        last_rebuild_at: row.get(4)?,
        last_sync_at: row.get(5)?,
        last_error: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn store_target(store_name: &str) -> Result<OutboxTarget> {
    DERIVED_STORE_SEEDS
        .iter()
        .find(|seed| seed.store_name == store_name)
        .map(|seed| seed.target)
        .ok_or_else(|| KanbanError::Storage(format!("unknown derived store: {store_name}")))
}

fn complete_outbox_for_store(
    conn: &Connection,
    target: OutboxTarget,
    board_id: &str,
    last_event_id: Option<i64>,
    now: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE index_outbox \
         SET status='done', last_error=NULL, updated_at=?1 \
         WHERE target IN (?2, 'all') \
           AND status IN ('pending', 'running', 'failed') \
           AND source_event_id <= ?3 \
           AND EXISTS (SELECT 1 FROM task_events e WHERE e.id=index_outbox.source_event_id AND e.board_id=?4)",
        params![now, target.as_str(), last_event_id.unwrap_or(i64::MAX), board_id],
    )
    .map_err(storage)?;
    Ok(())
}

fn has_unfinished_outbox_for_store(conn: &Connection, target: OutboxTarget) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS( \
             SELECT 1 FROM index_outbox \
             WHERE target IN (?1, 'all') AND status IN ('pending', 'running', 'failed') \
         )",
        [target.as_str()],
        |row| row.get::<_, i64>(0),
    )
    .map(|exists| exists != 0)
    .map_err(storage)
}

fn fail_outbox_for_store(
    conn: &Connection,
    target: OutboxTarget,
    board_id: &str,
    error: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE index_outbox \
         SET status='failed', attempts=attempts + 1, last_error=?1, updated_at=?2 \
         WHERE target IN (?3, 'all') \
           AND status IN ('pending', 'running') \
           AND EXISTS (SELECT 1 FROM task_events e WHERE e.id=index_outbox.source_event_id AND e.board_id=?4)",
        params![error, now, target.as_str(), board_id],
    )
    .map_err(storage)?;
    Ok(())
}
