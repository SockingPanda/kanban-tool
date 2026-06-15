use crate::connect_file;

use super::{
    LabelAtomRecord, LabelSemanticsRecord, UpsertLabelSemantics, board_id, derived_status_by_name,
    storage, vector_storage, with_immediate_tx,
};

use std::path::Path;

use kanban_core::{Clock, KanbanError, Result, SystemClock};
use kanban_indexer::{DERIVED_STORE_SCHEMA_VERSION, LANCEDB_LABEL_ATOMS_STORE};
use kanban_labels::{LabelAtomKind, LabelAtomPolarity, LabelDefinition};
use kanban_vector::{
    LabelAtomHit, LabelAtomQuery, LabelAtomVector, VectorStore, VectorStoreStatus,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

pub fn upsert_label_semantics(
    path: impl AsRef<Path>,
    board: &str,
    input: UpsertLabelSemantics,
) -> Result<LabelSemanticsRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let label = resolve_label(&conn, &board_id, &input.label_ref)?;
        let description = normalize_optional_text(input.description);
        let applies_when = normalize_text_list(input.applies_when);
        let excludes_when = normalize_text_list(input.excludes_when);
        let positive_examples = normalize_text_list(input.positive_examples);
        let negative_examples = normalize_text_list(input.negative_examples);

        conn.execute(
            "INSERT INTO label_semantics(label_id, board_id, description, applies_when, excludes_when, positive_examples, negative_examples, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8) \
             ON CONFLICT(label_id) DO UPDATE SET description=excluded.description, applies_when=excluded.applies_when, excludes_when=excluded.excludes_when, positive_examples=excluded.positive_examples, negative_examples=excluded.negative_examples, updated_at=excluded.updated_at",
            params![
                label.id,
                label.board_id,
                description,
                json_array(&applies_when)?,
                json_array(&excludes_when)?,
                json_array(&positive_examples)?,
                json_array(&negative_examples)?,
                now
            ],
        )
        .map_err(storage)?;

        let definition = LabelDefinition {
            id: label.id.clone(),
            name: label.name.clone(),
            description,
            applies_when,
            positive_examples,
            excludes_when,
            negative_examples,
        };
        rebuild_atoms_for_label(&conn, &definition, &label.board_id, now)?;
        mark_label_atom_store_dirty(&conn, now)?;
        get_label_semantics_conn(&conn, &label.board_id, &label.id)
    })
}

pub fn get_label_semantics(
    path: impl AsRef<Path>,
    board: &str,
    label_ref: &str,
) -> Result<LabelSemanticsRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let label = resolve_label(&conn, &board_id, label_ref)?;
    get_label_semantics_conn(&conn, &board_id, &label.id)
}

pub fn list_label_semantics(
    path: impl AsRef<Path>,
    board: &str,
) -> Result<Vec<LabelSemanticsRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let mut stmt = conn
        .prepare(
            "SELECT label_id FROM label_semantics WHERE board_id=?1 ORDER BY updated_at DESC, label_id ASC",
        )
        .map_err(storage)?;
    let ids = stmt
        .query_map([&board_id], |row| row.get::<_, String>(0))
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    ids.into_iter()
        .map(|label_id| get_label_semantics_conn(&conn, &board_id, &label_id))
        .collect()
}

pub fn delete_label_semantics(path: impl AsRef<Path>, board: &str, label_ref: &str) -> Result<()> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let label = resolve_label(&conn, &board_id, label_ref)?;
        conn.execute(
            "DELETE FROM label_semantics WHERE board_id=?1 AND label_id=?2",
            params![board_id, label.id],
        )
        .map_err(storage)?;
        conn.execute(
            "DELETE FROM label_atoms WHERE board_id=?1 AND label_id=?2",
            params![board_id, label.id],
        )
        .map_err(storage)?;
        mark_label_atom_store_dirty(&conn, now)?;
        Ok(())
    })
}

pub fn list_label_atoms(path: impl AsRef<Path>, board: &str) -> Result<Vec<LabelAtomRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    label_atoms_for_board(&conn, &board_id)
}

pub fn label_atom_index_status(path: impl AsRef<Path>, board: &str) -> Result<VectorStoreStatus> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    label_atom_index_status_from_base(
        &conn,
        &board_id,
        VectorStoreStatus {
            backend: "disabled".to_owned(),
            enabled: false,
            message: "Label atom vector store is disabled or has no provider".to_owned(),
        },
    )
}

pub fn label_atom_index_status_with(
    path: impl AsRef<Path>,
    board: &str,
    store: &(impl VectorStore + ?Sized),
) -> Result<VectorStoreStatus> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    label_atom_index_status_from_base(&conn, &board_id, store.status())
}

pub fn rebuild_label_atom_index_with(
    path: impl AsRef<Path>,
    board: &str,
    store: &impl VectorStore,
) -> Result<VectorStoreStatus> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let atoms = label_atom_vectors_for_board(&conn, &board_id, store.chunk_embedding_model())?;
    match store
        .delete_label_atoms_for_board(&board_id)
        .and_then(|()| store.upsert_label_atoms(&atoms))
    {
        Ok(()) => {
            let now = SystemClock.now_ms();
            mark_label_atom_store_success(&conn, true, now)?;
            let derived = derived_status_by_name(&conn, LANCEDB_LABEL_ATOMS_STORE)?;
            let mut status = store.status();
            status.message = format!(
                "{}; rebuilt {} label atom(s); dirty={} last_error={}",
                status.message,
                atoms.len(),
                derived.dirty,
                derived.last_error.as_deref().unwrap_or("none")
            );
            Ok(status)
        }
        Err(error) => {
            mark_label_atom_store_failure(&conn, &error.to_string(), SystemClock.now_ms())?;
            Err(vector_storage(error))
        }
    }
}

pub fn query_label_atom_index_with(
    path: impl AsRef<Path>,
    board: &str,
    store: &impl VectorStore,
    mut query: LabelAtomQuery,
) -> Result<Vec<LabelAtomHit>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    query.board_id = Some(board_id);
    store.query_label_atoms(&query).map_err(vector_storage)
}

fn get_label_semantics_conn(
    conn: &Connection,
    board_id: &str,
    label_id: &str,
) -> Result<LabelSemanticsRecord> {
    let mut record = conn
        .query_row(
            "SELECT s.label_id,s.board_id,l.name,s.description,s.applies_when,s.excludes_when,s.positive_examples,s.negative_examples,s.created_at,s.updated_at \
             FROM label_semantics s JOIN labels l ON l.id=s.label_id AND l.board_id=s.board_id \
             WHERE s.board_id=?1 AND s.label_id=?2",
            params![board_id, label_id],
            label_semantics_from_row,
        )
        .optional()
        .map_err(storage)?
        .ok_or_else(|| KanbanError::NotFound(format!("label semantics {label_id}")))?;
    record.atoms = label_atoms_for_label(conn, board_id, label_id)?;
    Ok(record)
}

fn label_semantics_from_row(row: &Row<'_>) -> rusqlite::Result<LabelSemanticsRecord> {
    Ok(LabelSemanticsRecord {
        label_id: row.get(0)?,
        board_id: row.get(1)?,
        label_name: row.get(2)?,
        description: row.get(3)?,
        applies_when: json_vec(row.get::<_, String>(4)?)
            .map_err(|err| rusqlite::Error::InvalidParameterName(err.to_string()))?,
        excludes_when: json_vec(row.get::<_, String>(5)?)
            .map_err(|err| rusqlite::Error::InvalidParameterName(err.to_string()))?,
        positive_examples: json_vec(row.get::<_, String>(6)?)
            .map_err(|err| rusqlite::Error::InvalidParameterName(err.to_string()))?,
        negative_examples: json_vec(row.get::<_, String>(7)?)
            .map_err(|err| rusqlite::Error::InvalidParameterName(err.to_string()))?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        atoms: Vec::new(),
    })
}

fn rebuild_atoms_for_label(
    conn: &Connection,
    definition: &LabelDefinition,
    board_id: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "DELETE FROM label_atoms WHERE board_id=?1 AND label_id=?2",
        params![board_id, definition.id],
    )
    .map_err(storage)?;

    for (ordinal, source) in definition.atom_sources().into_iter().enumerate() {
        let polarity = polarity_to_str(source.polarity);
        let kind = kind_to_str(source.kind);
        let content_hash = stable_hash(&format!(
            "{}\n{}\n{}\n{}\n{}",
            definition.id, polarity, kind, ordinal, source.text
        ));
        let id = format!("la_{content_hash}");
        conn.execute(
            "INSERT INTO label_atoms(id, label_id, board_id, polarity, kind, text, ordinal, content_hash, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                id,
                source.label_id,
                board_id,
                polarity,
                kind,
                source.text,
                ordinal as i64,
                content_hash,
                now
            ],
        )
        .map_err(storage)?;
    }
    Ok(())
}

fn label_atoms_for_board(conn: &Connection, board_id: &str) -> Result<Vec<LabelAtomRecord>> {
    let mut stmt = conn
        .prepare(
            "SELECT a.id,a.label_id,a.board_id,l.name,a.polarity,a.kind,a.text,a.ordinal,a.content_hash,a.created_at,a.updated_at \
             FROM label_atoms a JOIN labels l ON l.id=a.label_id AND l.board_id=a.board_id \
             WHERE a.board_id=?1 ORDER BY l.name ASC, a.ordinal ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map([board_id], label_atom_from_row)
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn label_atoms_for_label(
    conn: &Connection,
    board_id: &str,
    label_id: &str,
) -> Result<Vec<LabelAtomRecord>> {
    let mut stmt = conn
        .prepare(
            "SELECT a.id,a.label_id,a.board_id,l.name,a.polarity,a.kind,a.text,a.ordinal,a.content_hash,a.created_at,a.updated_at \
             FROM label_atoms a JOIN labels l ON l.id=a.label_id AND l.board_id=a.board_id \
             WHERE a.board_id=?1 AND a.label_id=?2 ORDER BY a.ordinal ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map(params![board_id, label_id], label_atom_from_row)
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn label_atom_vectors_for_board(
    conn: &Connection,
    board_id: &str,
    embedding_model: &str,
) -> Result<Vec<LabelAtomVector>> {
    Ok(label_atoms_for_board(conn, board_id)?
        .into_iter()
        .map(|atom| LabelAtomVector {
            atom_id: atom.id,
            label_id: atom.label_id,
            label_name: atom.label_name,
            board_id: atom.board_id,
            polarity: atom.polarity,
            kind: atom.kind,
            text: atom.text,
            ordinal: atom.ordinal,
            content_hash: atom.content_hash,
            embedding_model: embedding_model.to_owned(),
            created_at: atom.created_at,
            updated_at: atom.updated_at,
        })
        .collect())
}

fn label_atom_from_row(row: &Row<'_>) -> rusqlite::Result<LabelAtomRecord> {
    Ok(LabelAtomRecord {
        id: row.get(0)?,
        label_id: row.get(1)?,
        board_id: row.get(2)?,
        label_name: row.get(3)?,
        polarity: row.get(4)?,
        kind: row.get(5)?,
        text: row.get(6)?,
        ordinal: row.get(7)?,
        content_hash: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn resolve_label(conn: &Connection, board_id: &str, label_ref: &str) -> Result<ResolvedLabel> {
    let label_ref = label_ref.trim();
    if label_ref.is_empty() {
        return Err(KanbanError::InvalidInput("label ref is required".into()));
    }
    let sql = if label_ref.starts_with("l_") {
        "SELECT id,board_id,name FROM labels WHERE board_id=?1 AND id=?2"
    } else {
        "SELECT id,board_id,name FROM labels WHERE board_id=?1 AND name=?2"
    };
    conn.query_row(sql, params![board_id, label_ref], |row| {
        Ok(ResolvedLabel {
            id: row.get(0)?,
            board_id: row.get(1)?,
            name: row.get(2)?,
        })
    })
    .optional()
    .map_err(storage)?
    .ok_or_else(|| KanbanError::NotFound(format!("label {label_ref}")))
}

struct ResolvedLabel {
    id: String,
    board_id: String,
    name: String,
}

fn label_atom_index_status_from_base(
    conn: &Connection,
    _board_id: &str,
    mut status: VectorStoreStatus,
) -> Result<VectorStoreStatus> {
    let state = derived_status_by_name(conn, LANCEDB_LABEL_ATOMS_STORE)?;
    status.message = format!(
        "{}; dirty={} last_error={}",
        status.message,
        state.dirty,
        state.last_error.as_deref().unwrap_or("none")
    );
    Ok(status)
}

fn mark_label_atom_store_dirty(conn: &Connection, now: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO derived_store_state(store_name, schema_version, last_event_id, dirty, last_rebuild_at, last_sync_at, last_error, updated_at) \
         VALUES (?1, ?2, 0, 1, NULL, NULL, NULL, ?3) \
         ON CONFLICT(store_name) DO UPDATE SET dirty=1, updated_at=excluded.updated_at",
        params![LANCEDB_LABEL_ATOMS_STORE, DERIVED_STORE_SCHEMA_VERSION, now],
    )
    .map_err(storage)?;
    Ok(())
}

fn mark_label_atom_store_success(conn: &Connection, rebuilt: bool, now: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO derived_store_state(store_name, schema_version, last_event_id, dirty, last_rebuild_at, last_sync_at, last_error, updated_at) \
         VALUES (?1, ?2, 0, 0, ?3, ?4, NULL, ?5) \
         ON CONFLICT(store_name) DO UPDATE SET dirty=0, last_rebuild_at=COALESCE(excluded.last_rebuild_at, derived_store_state.last_rebuild_at), last_sync_at=COALESCE(excluded.last_sync_at, derived_store_state.last_sync_at), last_error=NULL, updated_at=excluded.updated_at",
        params![
            LANCEDB_LABEL_ATOMS_STORE,
            DERIVED_STORE_SCHEMA_VERSION,
            rebuilt.then_some(now),
            (!rebuilt).then_some(now),
            now
        ],
    )
    .map_err(storage)?;
    Ok(())
}

fn mark_label_atom_store_failure(conn: &Connection, error: &str, now: i64) -> Result<()> {
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
    Ok(())
}

fn normalize_optional_text(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

fn normalize_text_list(items: Vec<String>) -> Vec<String> {
    items
        .into_iter()
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
        .collect()
}

fn json_array(items: &[String]) -> Result<String> {
    serde_json::to_string(items).map_err(|err| KanbanError::InvalidInput(err.to_string()))
}

fn json_vec(json: String) -> Result<Vec<String>> {
    serde_json::from_str(&json).map_err(|err| KanbanError::Storage(err.to_string()))
}

fn polarity_to_str(polarity: LabelAtomPolarity) -> &'static str {
    match polarity {
        LabelAtomPolarity::Positive => "positive",
        LabelAtomPolarity::Negative => "negative",
    }
}

fn kind_to_str(kind: LabelAtomKind) -> &'static str {
    match kind {
        LabelAtomKind::Name => "name",
        LabelAtomKind::Description => "description",
        LabelAtomKind::AppliesWhen => "applies_when",
        LabelAtomKind::PositiveExample => "positive_example",
        LabelAtomKind::ExcludesWhen => "excludes_when",
        LabelAtomKind::NegativeExample => "negative_example",
    }
}

fn stable_hash(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
