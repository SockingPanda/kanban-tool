use kanban_core::{Clock, KanbanError, Result, SystemClock};
use kanban_indexer::{DERIVED_STORE_SCHEMA_VERSION, LANCEDB_LABEL_ATOMS_STORE};
use kanban_vector::{LabelAtomVector, LabelAtomVectorStore, VectorStoreBackend, VectorStoreStatus};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::{board_id, connect_file, derived_status_by_name, storage};

pub fn label_atom_vectors_for_board(
    conn: &Connection,
    board_id: &str,
    embedding_model: &str,
) -> Result<Vec<LabelAtomVector>> {
    let mut stmt = conn
        .prepare(
            "SELECT a.id,a.label_id,l.name,a.board_id,a.polarity,a.kind,a.text,a.ordinal,a.content_hash,a.created_at,a.updated_at \
             FROM label_atoms a JOIN labels l ON l.id=a.label_id AND l.board_id=a.board_id \
             WHERE a.board_id=?1 ORDER BY l.name ASC, a.ordinal ASC, a.id ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map([board_id], |row| {
            label_atom_vector_from_row(row, embedding_model)
        })
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

pub fn label_atom_index_status_with_conn(
    conn: &Connection,
    board_id: &str,
    store: &(impl VectorStoreBackend + ?Sized),
) -> Result<VectorStoreStatus> {
    label_atom_index_status_from_base(conn, board_id, store.status())
}

pub fn label_atom_index_status_from_base(
    conn: &Connection,
    board_id: &str,
    mut status: VectorStoreStatus,
) -> Result<VectorStoreStatus> {
    let state = derived_status_by_name(conn, LANCEDB_LABEL_ATOMS_STORE)?;
    let board = label_atom_index_board_status(conn, board_id)?;
    status.dirty = Some(state.dirty);
    status.board_dirty = Some(board.dirty);
    status.generation = board.last_rebuild_at;
    if !status.enabled {
        push_status_diagnostic(&mut status.diagnostics, "label_atom_index_disabled");
    }
    if state.dirty || board.dirty {
        push_status_diagnostic(&mut status.diagnostics, "label_atom_index_dirty");
    }
    if state.last_error.is_some() || board.last_error.is_some() {
        push_status_diagnostic(&mut status.diagnostics, "label_atom_index_error");
    }
    status.message = format!(
        "{}; dirty={} last_error={}; board_dirty={} board_last_rebuild_at={} board_last_error={}",
        status.message,
        state.dirty,
        state.last_error.as_deref().unwrap_or("none"),
        board.dirty,
        board
            .last_rebuild_at
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_owned()),
        board.last_error.as_deref().unwrap_or("none")
    );
    Ok(status)
}

pub fn rebuild_lancedb_label_atoms_with_store(
    path: impl AsRef<std::path::Path>,
    board: &str,
    store: &impl LabelAtomVectorStore,
) -> Result<VectorStoreStatus> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    rebuild_lancedb_label_atoms_with_conn(&conn, &board_id, store)
}

pub fn sync_lancedb_label_atoms_with_store(
    path: impl AsRef<std::path::Path>,
    board: &str,
    store: &impl LabelAtomVectorStore,
) -> Result<VectorStoreStatus> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let board = label_atom_index_board_status(&conn, &board_id)?;
    let state = derived_status_by_name(&conn, LANCEDB_LABEL_ATOMS_STORE)?;
    if !board.dirty && !state.dirty {
        return label_atom_index_status_with_conn(&conn, &board_id, store);
    }
    rebuild_lancedb_label_atoms_with_conn(&conn, &board_id, store)
}

pub fn mark_label_atom_store_dirty(conn: &Connection, board_id: &str, now: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO derived_store_state(store_name, schema_version, last_event_id, dirty, last_rebuild_at, last_sync_at, last_error, updated_at) \
         VALUES (?1, ?2, 0, 1, NULL, NULL, NULL, ?3) \
         ON CONFLICT(store_name) DO UPDATE SET dirty=1, updated_at=excluded.updated_at",
        params![LANCEDB_LABEL_ATOMS_STORE, DERIVED_STORE_SCHEMA_VERSION, now],
    )
    .map_err(storage)?;
    conn.execute(
        "INSERT INTO label_atom_index_boards(store_name, board_id, dirty, last_rebuild_at, last_error, updated_at) \
         VALUES (?1, ?2, 1, NULL, NULL, ?3) \
         ON CONFLICT(store_name, board_id) DO UPDATE SET dirty=1, last_error=NULL, updated_at=excluded.updated_at",
        params![LANCEDB_LABEL_ATOMS_STORE, board_id, now],
    )
    .map_err(storage)?;
    Ok(())
}

pub fn mark_label_atom_store_success(conn: &Connection, board_id: &str, now: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO label_atom_index_boards(store_name, board_id, dirty, last_rebuild_at, last_error, updated_at) \
         VALUES (?1, ?2, 0, ?3, NULL, ?3) \
         ON CONFLICT(store_name, board_id) DO UPDATE SET dirty=0, last_rebuild_at=excluded.last_rebuild_at, last_error=NULL, updated_at=excluded.updated_at",
        params![LANCEDB_LABEL_ATOMS_STORE, board_id, now],
    )
    .map_err(storage)?;
    let dirty = has_dirty_label_atom_boards(conn)?;
    conn.execute(
        "INSERT INTO derived_store_state(store_name, schema_version, last_event_id, dirty, last_rebuild_at, last_sync_at, last_error, updated_at) \
         VALUES (?1, ?2, 0, ?3, ?4, NULL, NULL, ?5) \
         ON CONFLICT(store_name) DO UPDATE SET dirty=excluded.dirty, last_rebuild_at=COALESCE(excluded.last_rebuild_at, derived_store_state.last_rebuild_at), last_error=CASE WHEN excluded.dirty=0 THEN NULL ELSE derived_store_state.last_error END, updated_at=excluded.updated_at",
        params![
            LANCEDB_LABEL_ATOMS_STORE,
            DERIVED_STORE_SCHEMA_VERSION,
            i64::from(dirty),
            now,
            now
        ],
    )
    .map_err(storage)?;
    Ok(())
}

pub fn mark_label_atom_store_failure(
    conn: &Connection,
    board_id: &str,
    error: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO derived_store_state(store_name, schema_version, last_event_id, dirty, last_rebuild_at, last_sync_at, last_error, updated_at) \
         VALUES (?1, ?2, 0, 1, NULL, NULL, ?3, ?4) \
         ON CONFLICT(store_name) DO UPDATE SET dirty=1, last_error=excluded.last_error, updated_at=excluded.updated_at",
        params![
            LANCEDB_LABEL_ATOMS_STORE,
            DERIVED_STORE_SCHEMA_VERSION,
            error,
            now
        ],
    )
    .map_err(storage)?;
    conn.execute(
        "INSERT INTO label_atom_index_boards(store_name, board_id, dirty, last_rebuild_at, last_error, updated_at) \
         VALUES (?1, ?2, 1, NULL, ?3, ?4) \
         ON CONFLICT(store_name, board_id) DO UPDATE SET dirty=1, last_error=excluded.last_error, updated_at=excluded.updated_at",
        params![LANCEDB_LABEL_ATOMS_STORE, board_id, error, now],
    )
    .map_err(storage)?;
    Ok(())
}

fn rebuild_lancedb_label_atoms_with_conn(
    conn: &Connection,
    board_id: &str,
    store: &impl LabelAtomVectorStore,
) -> Result<VectorStoreStatus> {
    let atoms = label_atom_vectors_for_board(conn, board_id, store.embedding_model())?;
    match store
        .delete_label_atoms_for_board(board_id)
        .and_then(|()| store.upsert_label_atoms(&atoms))
    {
        Ok(()) => {
            let now = SystemClock.now_ms();
            mark_label_atom_store_success(conn, board_id, now)?;
            let mut status = label_atom_index_status_with_conn(conn, board_id, store)?;
            status.message = format!("{}; rebuilt {} label atom(s)", status.message, atoms.len());
            Ok(status)
        }
        Err(error) => {
            mark_label_atom_store_failure(
                conn,
                board_id,
                &error.to_string(),
                SystemClock.now_ms(),
            )?;
            Err(vector_storage(error))
        }
    }
}

fn label_atom_vector_from_row(
    row: &Row<'_>,
    embedding_model: &str,
) -> rusqlite::Result<LabelAtomVector> {
    Ok(LabelAtomVector {
        atom_id: row.get(0)?,
        label_id: row.get(1)?,
        label_name: row.get(2)?,
        board_id: row.get(3)?,
        polarity: row.get(4)?,
        kind: row.get(5)?,
        text: row.get(6)?,
        ordinal: row.get(7)?,
        content_hash: row.get(8)?,
        embedding_model: embedding_model.to_owned(),
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn has_dirty_label_atom_boards(conn: &Connection) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM label_atom_index_boards WHERE store_name=?1 AND dirty=1)",
        [LANCEDB_LABEL_ATOMS_STORE],
        |row| row.get(0),
    )
    .map_err(storage)
}

struct LabelAtomIndexBoardStatus {
    dirty: bool,
    last_rebuild_at: Option<i64>,
    last_error: Option<String>,
}

fn label_atom_index_board_status(
    conn: &Connection,
    board_id: &str,
) -> Result<LabelAtomIndexBoardStatus> {
    conn.query_row(
        "SELECT dirty,last_rebuild_at,last_error \
         FROM label_atom_index_boards WHERE store_name=?1 AND board_id=?2",
        params![LANCEDB_LABEL_ATOMS_STORE, board_id],
        |row| {
            Ok(LabelAtomIndexBoardStatus {
                dirty: row.get::<_, bool>(0)?,
                last_rebuild_at: row.get(1)?,
                last_error: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(storage)
    .map(|status| {
        status.unwrap_or(LabelAtomIndexBoardStatus {
            dirty: false,
            last_rebuild_at: None,
            last_error: None,
        })
    })
}

fn push_status_diagnostic(diagnostics: &mut Vec<String>, code: &str) {
    if !diagnostics.iter().any(|diagnostic| diagnostic == code) {
        diagnostics.push(code.to_owned());
    }
}

fn vector_storage(error: impl std::fmt::Display) -> KanbanError {
    KanbanError::Storage(error.to_string())
}
