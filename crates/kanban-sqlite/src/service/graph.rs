use rusqlite::{Connection, OptionalExtension, Row};

use super::{
    DerivedStoreStatusRecord, MAX_TASK_LIST_LIMIT, TaskRecord, derived_store_status_from_row,
    storage, validate_page_bounds,
};
use super::{
    board_id, current_last_event_id, has_pending_outbox_for_target, push_context_diagnostic,
    push_degraded_marker, search_lag, vector_storage, vector_store_status_with_conn,
};

use std::path::Path;

use kanban_context::{ContextDiagnostic, ContextItem};

use kanban_core::{KanbanError, Result};

use kanban_entity::{EntityUri, Predicate, Provenance, Relation};

use crate::connect_file;
use kanban_graph::{GraphStoreStatus, RelationGraph};
use kanban_indexer::{OXIGRAPH_RELATIONS_STORE, OutboxTarget};
use kanban_vector::{ChunkVectorStore, VectorStoreBackend, VectorStoreStatus};
use kanban_vector::{VectorHit, VectorQuery};

pub fn graph_neighbors(
    _path: impl AsRef<Path>,
    _entity_uri: &EntityUri,
    _predicate: Option<Predicate>,
    limit: usize,
) -> Result<Vec<Relation>> {
    validate_page_bounds(limit, MAX_TASK_LIST_LIMIT, 0)?;
    Ok(Vec::new())
}

pub(crate) fn context_graph_items(
    _path: &Path,
    _subject: &EntityUri,
    _limit: usize,
) -> Result<Vec<ContextItem>> {
    Ok(Vec::new())
}

pub(crate) fn context_vector_items(
    path: &Path,
    task: &TaskRecord,
    status: &VectorStoreStatus,
    limit: usize,
    store: Option<&dyn ChunkVectorStore>,
) -> Result<Vec<ContextItem>> {
    if !status.enabled || limit == 0 {
        return Ok(Vec::new());
    }
    let Some(store) = store else {
        return Ok(Vec::new());
    };
    let hits = store
        .query(&VectorQuery {
            text: task_context_text(task),
            limit,
        })
        .map_err(vector_storage)?;
    vector_hits_to_context_items(path, hits)
}

pub(crate) fn context_vector_status(
    _path: &Path,
    conn: &Connection,
    board_id: &str,
    _board: &str,
    store: Option<&dyn ChunkVectorStore>,
    degraded: &mut Vec<String>,
    diagnostics: &mut Vec<ContextDiagnostic>,
) -> VectorStoreStatus {
    let status = match store {
        Some(store) => vector_store_status_with_conn(conn, board_id, store),
        None => Ok(kanban_vector::DisabledVectorStore.status()),
    };
    match status {
        Ok(status) => status,
        Err(error) => {
            push_degraded_marker(degraded, "vector_error");
            push_context_diagnostic(diagnostics, "vector", "vector_error", &error);
            VectorStoreStatus::new("helper-missing", false, error.to_string())
        }
    }
}

fn vector_hits_to_context_items(path: &Path, hits: Vec<VectorHit>) -> Result<Vec<ContextItem>> {
    let conn = connect_file(path)?;
    Ok(hits
        .into_iter()
        .map(|hit| {
            let title = entity_title(&conn, hit.chunk.entity_uri.as_str())
                .ok()
                .flatten();
            ContextItem {
                entity_uri: hit.chunk.entity_uri,
                source: "vector".to_owned(),
                provenance: vec!["vector:lancedb".to_owned()],
                score: Some(f64::from(hit.score)),
                title: title.or(hit.summary),
                snippet: hit.text,
            }
        })
        .collect())
}

fn entity_title(conn: &Connection, uri: &str) -> Result<Option<String>> {
    conn.query_row("SELECT title FROM entities WHERE uri=?1", [uri], |row| {
        row.get(0)
    })
    .optional()
    .map_err(storage)
}

fn task_context_text(task: &TaskRecord) -> String {
    match task.description.as_deref().map(str::trim) {
        Some(description) if !description.is_empty() => {
            format!("{}\n\n{}", task.title.trim(), description)
        }
        _ => task.title.trim().to_owned(),
    }
}

pub fn graph_store_status(path: impl AsRef<Path>, board: &str) -> Result<GraphStoreStatus> {
    let path_ref = path.as_ref();
    let conn = connect_file(path_ref)?;
    let board_id = board_id(&conn, board)?;
    let current_last_event_id = current_last_event_id(&conn, &board_id)?;
    let state = derived_status_by_name(&conn, OXIGRAPH_RELATIONS_STORE)?;
    let board_has_pending = has_pending_outbox_for_target(
        &conn,
        OutboxTarget::Oxigraph,
        &board_id,
        current_last_event_id,
    )?;
    let graph = kanban_graph::DisabledGraphStore;
    let mut status = graph.status();
    let lag = if board_has_pending {
        search_lag(current_last_event_id, Some(state.last_event_id))
    } else {
        0
    };
    status.message = format!(
        "{}; dirty={} last_event_id={} lag={} last_error={}",
        status.message,
        state.dirty,
        state.last_event_id,
        lag,
        state.last_error.as_deref().unwrap_or("none")
    );
    Ok(status)
}

pub fn rebuild_graph_store(path: impl AsRef<Path>, board: &str) -> Result<GraphStoreStatus> {
    graph_store_status(path, board)
}

pub fn sync_graph_store(path: impl AsRef<Path>, board: &str) -> Result<GraphStoreStatus> {
    graph_store_status(path, board)
}

fn relation_from_row(row: &Row<'_>) -> rusqlite::Result<Relation> {
    let predicate: String = row.get(1)?;
    Ok(Relation {
        subject_uri: EntityUri::new(row.get::<_, String>(0)?).map_err(sql_from_display)?,
        predicate: predicate_from_str(&predicate).map_err(sql_from_display)?,
        object_uri: EntityUri::new(row.get::<_, String>(2)?).map_err(sql_from_display)?,
        graph_uri: EntityUri::new(row.get::<_, String>(3)?).map_err(sql_from_display)?,
        provenance: Provenance {
            authoritative_store: row.get(4)?,
            source_table: row.get(5)?,
            source_id: row.get(6)?,
            source_event_id: row.get(7)?,
        },
        metadata_json: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

pub(crate) fn graph_relation_snapshot_for_board(
    conn: &Connection,
    board_id: &str,
) -> Result<Vec<Relation>> {
    let mut stmt = conn
        .prepare(
            "SELECT r.subject_uri,r.predicate,r.object_uri,r.graph_uri,r.authoritative_store,r.source_table,r.source_id,r.source_event_id,r.metadata_json,r.created_at,r.updated_at \
             FROM entity_relations r \
             JOIN entities s ON s.uri=r.subject_uri \
             WHERE s.board_id=?1 \
             ORDER BY r.subject_uri ASC, r.predicate ASC, r.object_uri ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map([board_id], relation_from_row)
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn predicate_from_str(value: &str) -> Result<Predicate> {
    match value {
        "belongs_to_board" => Ok(Predicate::BelongsToBoard),
        "belongs_to_task" => Ok(Predicate::BelongsToTask),
        "depends_on" => Ok(Predicate::DependsOn),
        "produced_by" => Ok(Predicate::ProducedBy),
        "generated_by" => Ok(Predicate::GeneratedBy),
        "references_artifact" => Ok(Predicate::ReferencesArtifact),
        "related_to" => Ok(Predicate::RelatedTo),
        "uses_skill" => Ok(Predicate::UsesSkill),
        "uses_context" => Ok(Predicate::UsesContext),
        "derived_from" => Ok(Predicate::DerivedFrom),
        "supersedes" => Ok(Predicate::Supersedes),
        "similar_to" => Ok(Predicate::SimilarTo),
        "requires_review" => Ok(Predicate::RequiresReview),
        "waiting_for_user" => Ok(Predicate::WaitingForUser),
        _ => Err(KanbanError::Storage(format!("unknown predicate: {value}"))),
    }
}

fn sql_from_display(error: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(KanbanError::Storage(error.to_string())))
}

pub(crate) fn derived_status_by_name(
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
