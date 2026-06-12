use rusqlite::{Connection, params};

use super::storage;

use kanban_core::{KanbanError, Result};

use kanban_indexer::{
    DERIVED_STORE_SEEDS, DerivedStoreUpdate, OUTBOX_FANOUT_TARGETS, OutboxTarget,
    derived_store_for_name,
};

pub(crate) fn upsert_board_entity(conn: &Connection, board_id: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         SELECT 'kb://board/' || id, 'board', 'boards', id, id, NULL, name, description, NULL, created_at, updated_at, archived_at FROM boards WHERE id=?1 \
         ON CONFLICT(uri) DO UPDATE SET kind=excluded.kind, source_table=excluded.source_table, source_id=excluded.source_id, board_id=excluded.board_id, task_id=excluded.task_id, title=excluded.title, summary=excluded.summary, content_hash=excluded.content_hash, updated_at=excluded.updated_at, archived_at=excluded.archived_at",
        [board_id],
    )
    .map_err(storage)?;
    Ok(())
}

pub(crate) fn upsert_event_entity(
    conn: &Connection,
    event_id: &str,
    board_id: &str,
    task_id: Option<&str>,
    kind: &str,
    payload: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         VALUES (?1, 'event', 'task_events', ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?7, NULL) \
         ON CONFLICT(uri) DO UPDATE SET kind=excluded.kind, source_table=excluded.source_table, source_id=excluded.source_id, board_id=excluded.board_id, task_id=excluded.task_id, title=excluded.title, summary=excluded.summary, content_hash=excluded.content_hash, updated_at=excluded.updated_at, archived_at=excluded.archived_at",
        params![
            format!("kb://event/{event_id}"),
            event_id,
            board_id,
            task_id,
            kind,
            payload,
            now
        ],
    )
    .map_err(storage)?;
    Ok(())
}

pub(crate) fn upsert_task_entity(conn: &Connection, task_id: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         SELECT 'kb://task/' || id, 'task', 'tasks', id, board_id, id, title, description, NULL, created_at, updated_at, archived_at FROM tasks WHERE id=?1 \
         ON CONFLICT(uri) DO UPDATE SET kind=excluded.kind, source_table=excluded.source_table, source_id=excluded.source_id, board_id=excluded.board_id, task_id=excluded.task_id, title=excluded.title, summary=excluded.summary, content_hash=excluded.content_hash, updated_at=excluded.updated_at, archived_at=excluded.archived_at",
        [task_id],
    )
    .map_err(storage)?;
    upsert_task_board_relation(conn, task_id)?;
    Ok(())
}

pub(crate) fn upsert_run_entity(conn: &Connection, run_id: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         SELECT 'kb://run/' || id, 'run', 'task_runs', id, board_id, task_id, id, COALESCE(summary, error), NULL, started_at, COALESCE(finished_at, last_heartbeat_at, started_at), NULL FROM task_runs WHERE id=?1 \
         ON CONFLICT(uri) DO UPDATE SET kind=excluded.kind, source_table=excluded.source_table, source_id=excluded.source_id, board_id=excluded.board_id, task_id=excluded.task_id, title=excluded.title, summary=excluded.summary, content_hash=excluded.content_hash, updated_at=excluded.updated_at, archived_at=excluded.archived_at",
        [run_id],
    )
    .map_err(storage)?;
    Ok(())
}

pub(crate) fn upsert_task_board_relation(conn: &Connection, task_id: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO entity_relations(subject_uri, predicate, object_uri, graph_uri, authoritative_store, source_table, source_id, source_event_id, metadata_json, created_at, updated_at) \
         SELECT 'kb://task/' || id, 'belongs_to_board', 'kb://board/' || board_id, 'kb://graph/indexed', 'sqlite', 'tasks', id, NULL, '{}', created_at, updated_at \
         FROM tasks WHERE id=?1",
        [task_id],
    )
    .map_err(storage)?;
    Ok(())
}

pub(crate) fn upsert_dependency_relation(
    conn: &Connection,
    parent_task_id: &str,
    child_task_id: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO entity_relations(subject_uri, predicate, object_uri, graph_uri, authoritative_store, source_table, source_id, source_event_id, metadata_json, created_at, updated_at) \
         SELECT 'kb://task/' || child_task_id, 'depends_on', 'kb://task/' || parent_task_id, 'kb://graph/indexed', 'sqlite', 'task_dependencies', parent_task_id || '->' || child_task_id, NULL, '{}', created_at, ?3 \
         FROM task_dependencies WHERE parent_task_id=?1 AND child_task_id=?2",
        params![parent_task_id, child_task_id, now],
    )
    .map_err(storage)?;
    Ok(())
}

pub(crate) fn delete_dependency_relation(
    conn: &Connection,
    parent_task_id: &str,
    child_task_id: &str,
) -> Result<()> {
    conn.execute(
        "DELETE FROM entity_relations \
         WHERE subject_uri=?1 AND predicate='depends_on' AND object_uri=?2 AND source_table='task_dependencies'",
        params![
            format!("kb://task/{child_task_id}"),
            format!("kb://task/{parent_task_id}")
        ],
    )
    .map_err(storage)?;
    Ok(())
}

pub(crate) fn enqueue_index_outbox(
    conn: &Connection,
    source_event_id: i64,
    entity_uri: &str,
    action: &str,
    now: i64,
) -> Result<()> {
    for target in OUTBOX_FANOUT_TARGETS {
        conn.execute(
            "INSERT INTO index_outbox(source_event_id, target, entity_uri, action, payload_json, status, attempts, last_error, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, '{}', 'pending', 0, NULL, ?5, ?5)",
            params![source_event_id, target.as_str(), entity_uri, action, now],
        )
        .map_err(storage)?;
    }
    for seed in DERIVED_STORE_SEEDS {
        mark_derived_store_dirty(conn, seed.store_name, now)?;
    }
    Ok(())
}

pub(crate) fn mark_derived_store_dirty(
    conn: &Connection,
    store_name: &str,
    now: i64,
) -> Result<()> {
    let seed = derived_store_for_name(store_name)
        .ok_or_else(|| KanbanError::Storage(format!("unknown derived store: {store_name}")))?;
    let update = DerivedStoreUpdate::dirty(seed, now);
    conn.execute(
        "INSERT INTO derived_store_state(store_name, schema_version, last_event_id, dirty, last_rebuild_at, last_sync_at, last_error, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT(store_name) DO UPDATE SET dirty=1, updated_at=excluded.updated_at",
        params![
            store_name,
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
    Ok(())
}

pub(crate) fn mark_derived_store_success(
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

pub(crate) fn mark_derived_store_failure(
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

pub(crate) fn store_target(store_name: &str) -> Result<OutboxTarget> {
    DERIVED_STORE_SEEDS
        .iter()
        .find(|seed| seed.store_name == store_name)
        .map(|seed| seed.target)
        .ok_or_else(|| KanbanError::Storage(format!("unknown derived store: {store_name}")))
}

pub(crate) fn complete_outbox_for_store(
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
           AND EXISTS ( \
               SELECT 1 FROM task_events e \
               WHERE e.id=index_outbox.source_event_id AND e.board_id=?4 \
           )",
        params![
            now,
            target.as_str(),
            last_event_id.unwrap_or(i64::MAX),
            board_id
        ],
    )
    .map_err(storage)?;
    Ok(())
}

pub(crate) fn has_unfinished_outbox_for_store(
    conn: &Connection,
    target: OutboxTarget,
) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS( \
             SELECT 1 FROM index_outbox \
             WHERE target IN (?1, 'all') \
               AND status IN ('pending', 'running', 'failed') \
         )",
        [target.as_str()],
        |row| row.get::<_, i64>(0),
    )
    .map(|exists| exists != 0)
    .map_err(storage)
}

pub(crate) fn fail_outbox_for_store(
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
           AND EXISTS ( \
               SELECT 1 FROM task_events e \
               WHERE e.id=index_outbox.source_event_id AND e.board_id=?4 \
           )",
        params![error, now, target.as_str(), board_id],
    )
    .map_err(storage)?;
    Ok(())
}

pub(crate) fn json_valid(conn: &Connection, json: &str) -> Result<bool> {
    conn.query_row("SELECT json_valid(?1)", [json], |r| r.get::<_, i64>(0))
        .map(|v| v == 1)
        .map_err(storage)
}
