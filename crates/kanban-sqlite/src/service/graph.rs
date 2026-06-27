#[cfg(feature = "graph-oxigraph")]
use rusqlite::params;
use rusqlite::{Connection, OptionalExtension, Row};

use super::{
    DerivedStoreStatusRecord, MAX_TASK_LIST_LIMIT, TaskRecord, derived_store_status_from_row,
    storage, validate_page_bounds, vector_store_status,
};
#[cfg(feature = "graph-oxigraph")]
use super::{
    IndexOutboxRecord, board_id, current_last_event_id, mark_derived_store_failure,
    mark_derived_store_success, outbox_from_row, search_lag,
};
#[cfg(feature = "vector-lancedb")]
use super::{
    push_context_diagnostic, push_degraded_marker, vector_storage, vector_store_path,
    vector_store_status_with_conn,
};

use std::path::Path;
#[cfg(feature = "graph-oxigraph")]
use std::path::PathBuf;

use kanban_context::{ContextDiagnostic, ContextItem};

#[cfg(feature = "graph-oxigraph")]
use kanban_core::{Clock, SystemClock};
use kanban_core::{KanbanError, Result};

use kanban_entity::{EntityUri, Predicate, Provenance, Relation};

#[cfg(feature = "graph-oxigraph")]
use kanban_graph::GraphStoreStatus;
use kanban_graph::RelationGraph;
#[cfg(feature = "graph-oxigraph")]
use kanban_graph_oxigraph::OxigraphStore;
#[cfg(feature = "graph-oxigraph")]
use kanban_indexer::OXIGRAPH_RELATIONS_STORE;
use kanban_vector::{ChunkVectorStore, VectorStoreStatus};
use kanban_vector::{VectorHit, VectorQuery};
#[cfg(feature = "vector-lancedb")]
use kanban_vector_lancedb::{LanceDbConfig, LanceDbStore};

#[cfg(any(feature = "graph-oxigraph", feature = "vector-lancedb"))]
use crate::connect_file;

#[cfg(feature = "graph-oxigraph")]
pub fn graph_neighbors(
    path: impl AsRef<Path>,
    entity_uri: &EntityUri,
    predicate: Option<Predicate>,
    limit: usize,
) -> Result<Vec<Relation>> {
    validate_page_bounds(limit, MAX_TASK_LIST_LIMIT, 0)?;
    let graph = OxigraphStore::open(graph_store_path(path.as_ref())).map_err(graph_storage)?;
    graph
        .neighbors(entity_uri, predicate, limit)
        .map_err(graph_storage)
}

#[cfg(not(feature = "graph-oxigraph"))]
pub fn graph_neighbors(
    _path: impl AsRef<Path>,
    _entity_uri: &EntityUri,
    _predicate: Option<Predicate>,
    limit: usize,
) -> Result<Vec<Relation>> {
    validate_page_bounds(limit, MAX_TASK_LIST_LIMIT, 0)?;
    Ok(Vec::new())
}

#[cfg(feature = "graph-oxigraph")]
pub(crate) fn context_graph_items(
    path: &Path,
    subject: &EntityUri,
    limit: usize,
) -> Result<Vec<ContextItem>> {
    let relations = graph_neighbors(path, subject, None, limit)?;
    let conn = connect_file(path)?;
    Ok(relations
        .into_iter()
        .map(|relation| {
            let title = entity_title(&conn, relation.object_uri.as_str())
                .ok()
                .flatten();
            ContextItem {
                entity_uri: relation.object_uri,
                source: "graph".to_owned(),
                provenance: vec![format!("graph:{}", relation.predicate)],
                score: None,
                title,
                snippet: Some(relation.predicate.to_string()),
            }
        })
        .collect())
}

#[cfg(not(feature = "graph-oxigraph"))]
pub(crate) fn context_graph_items(
    _path: &Path,
    _subject: &EntityUri,
    _limit: usize,
) -> Result<Vec<ContextItem>> {
    Ok(Vec::new())
}

#[cfg(feature = "vector-lancedb")]
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
    let owned_store;
    let store = match store {
        Some(store) => store,
        None => {
            owned_store = LanceDbStore::connect(LanceDbConfig::degraded(vector_store_path(path)))
                .map_err(vector_storage)?;
            &owned_store
        }
    };
    let hits = store
        .query(&VectorQuery {
            text: task_context_text(task),
            limit,
        })
        .map_err(vector_storage)?;
    vector_hits_to_context_items(path, hits)
}

#[cfg(not(feature = "vector-lancedb"))]
pub(crate) fn context_vector_items(
    _path: &Path,
    _task: &TaskRecord,
    _status: &VectorStoreStatus,
    _limit: usize,
    _store: Option<&dyn ChunkVectorStore>,
) -> Result<Vec<ContextItem>> {
    Ok(Vec::new())
}

#[cfg(feature = "vector-lancedb")]
pub(crate) fn context_vector_status(
    path: &Path,
    conn: &Connection,
    board_id: &str,
    board: &str,
    store: Option<&dyn ChunkVectorStore>,
    degraded: &mut Vec<String>,
    diagnostics: &mut Vec<ContextDiagnostic>,
) -> VectorStoreStatus {
    let status = match store {
        Some(store) => vector_store_status_with_conn(conn, board_id, store),
        None => vector_store_status(path, board),
    };
    match status {
        Ok(status) => status,
        Err(error) => {
            push_degraded_marker(degraded, "vector_error");
            push_context_diagnostic(diagnostics, "vector", "vector_error", &error);
            VectorStoreStatus::new("lancedb", true, error.to_string())
        }
    }
}

#[cfg(not(feature = "vector-lancedb"))]
pub(crate) fn context_vector_status(
    path: &Path,
    _conn: &Connection,
    _board_id: &str,
    board: &str,
    _store: Option<&dyn ChunkVectorStore>,
    _degraded: &mut Vec<String>,
    _diagnostics: &mut Vec<ContextDiagnostic>,
) -> VectorStoreStatus {
    vector_store_status(path, board).expect("disabled vector status is infallible")
}

#[cfg(feature = "vector-lancedb")]
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

#[cfg(any(feature = "graph-oxigraph", feature = "vector-lancedb"))]
fn entity_title(conn: &Connection, uri: &str) -> Result<Option<String>> {
    conn.query_row("SELECT title FROM entities WHERE uri=?1", [uri], |row| {
        row.get(0)
    })
    .optional()
    .map_err(storage)
}

#[cfg(feature = "vector-lancedb")]
fn task_context_text(task: &TaskRecord) -> String {
    match task.description.as_deref().map(str::trim) {
        Some(description) if !description.is_empty() => {
            format!("{}\n\n{}", task.title.trim(), description)
        }
        _ => task.title.trim().to_owned(),
    }
}

#[cfg(feature = "graph-oxigraph")]
pub fn graph_store_status(path: impl AsRef<Path>, board: &str) -> Result<GraphStoreStatus> {
    let path_ref = path.as_ref();
    let conn = connect_file(path_ref)?;
    let board_id = board_id(&conn, board)?;
    let graph = OxigraphStore::open(graph_store_path(path_ref)).map_err(graph_storage)?;
    let current_last_event_id = current_last_event_id(&conn, &board_id)?;
    let state = derived_status_by_name(&conn, OXIGRAPH_RELATIONS_STORE)?;
    let board_has_pending =
        has_pending_graph_outbox_for_board(&conn, &board_id, current_last_event_id)?;
    let mut status = graph.status();
    let lag = if board_has_pending {
        search_lag(current_last_event_id, Some(state.last_event_id))
    } else {
        0
    };
    status.message = format!(
        "{}; dirty={} last_event_id={} lag={}",
        status.message, state.dirty, state.last_event_id, lag
    );
    Ok(status)
}

#[cfg(not(feature = "graph-oxigraph"))]
pub fn graph_store_status(
    _path: impl AsRef<Path>,
    _board: &str,
) -> Result<kanban_graph::GraphStoreStatus> {
    let graph = kanban_graph::DisabledGraphStore;
    Ok(kanban_graph::RelationGraph::status(&graph))
}

#[cfg(feature = "graph-oxigraph")]
pub fn rebuild_graph_store(path: impl AsRef<Path>, board: &str) -> Result<GraphStoreStatus> {
    let path_ref = path.as_ref();
    let conn = connect_file(path_ref)?;
    let board_id = board_id(&conn, board)?;
    let last_event_id = current_last_event_id(&conn, &board_id)?;
    let relations = graph_relation_snapshot_for_board(&conn, &board_id)?;
    let entity_uris = graph_entity_uris_for_board(&conn, &board_id)?;
    let result = (|| -> Result<()> {
        let graph = OxigraphStore::open(graph_store_path(path_ref)).map_err(graph_storage)?;
        graph
            .replace_entities(&entity_uris, &relations)
            .map_err(graph_storage)?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            let now = SystemClock.now_ms();
            mark_derived_store_success(
                &conn,
                OXIGRAPH_RELATIONS_STORE,
                &board_id,
                last_event_id,
                true,
                now,
            )?;
            Ok(GraphStoreStatus {
                backend: "oxigraph".to_owned(),
                enabled: true,
                message: format!(
                    "Rebuilt Oxigraph relation store ({} relation(s))",
                    relations.len()
                ),
            })
        }
        Err(error) => {
            mark_derived_store_failure(
                &conn,
                OXIGRAPH_RELATIONS_STORE,
                &board_id,
                &error.to_string(),
                SystemClock.now_ms(),
            )?;
            Err(KanbanError::Storage(error.to_string()))
        }
    }
}

#[cfg(not(feature = "graph-oxigraph"))]
pub fn rebuild_graph_store(
    path: impl AsRef<Path>,
    board: &str,
) -> Result<kanban_graph::GraphStoreStatus> {
    graph_store_status(path, board)
}

#[cfg(feature = "graph-oxigraph")]
pub fn sync_graph_store(path: impl AsRef<Path>, board: &str) -> Result<GraphStoreStatus> {
    let path_ref = path.as_ref();
    let conn = connect_file(path_ref)?;
    let board_id = board_id(&conn, board)?;
    let last_event_id = current_last_event_id(&conn, &board_id)?;
    let state = derived_status_by_name(&conn, OXIGRAPH_RELATIONS_STORE)?;
    if !has_pending_graph_outbox_for_board(&conn, &board_id, last_event_id)? {
        return graph_store_status(path_ref, board);
    }
    let jobs = pending_graph_outbox_for_board(&conn, &board_id, last_event_id)?;
    let result = (|| -> Result<()> {
        let graph = OxigraphStore::open(graph_store_path(path_ref)).map_err(graph_storage)?;
        if state.last_event_id == 0 || jobs.iter().any(|job| job.action == "rebuild") {
            let relations = graph_relation_snapshot_for_board(&conn, &board_id)?;
            let entity_uris = graph_entity_uris_for_board(&conn, &board_id)?;
            graph
                .replace_entities(&entity_uris, &relations)
                .map_err(graph_storage)?;
        } else {
            let mut affected = jobs
                .iter()
                .map(|job| job.entity_uri.clone())
                .collect::<Vec<_>>();
            affected.sort();
            affected.dedup();
            for uri in affected {
                let entity_uri = EntityUri::new(uri).map_err(graph_storage)?;
                let relations = graph_relations_for_entity(&conn, &board_id, entity_uri.as_str())?;
                if relations.is_empty() {
                    graph.delete(&entity_uri).map_err(graph_storage)?;
                } else {
                    graph.upsert(&relations).map_err(graph_storage)?;
                }
            }
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            let now = SystemClock.now_ms();
            mark_derived_store_success(
                &conn,
                OXIGRAPH_RELATIONS_STORE,
                &board_id,
                last_event_id,
                false,
                now,
            )?;
            Ok(GraphStoreStatus {
                backend: "oxigraph".to_owned(),
                enabled: true,
                message: format!("Synced Oxigraph relation store ({} job(s))", jobs.len()),
            })
        }
        Err(error) => {
            mark_derived_store_failure(
                &conn,
                OXIGRAPH_RELATIONS_STORE,
                &board_id,
                &error.to_string(),
                SystemClock.now_ms(),
            )?;
            Err(KanbanError::Storage(error.to_string()))
        }
    }
}

#[cfg(not(feature = "graph-oxigraph"))]
pub fn sync_graph_store(
    path: impl AsRef<Path>,
    board: &str,
) -> Result<kanban_graph::GraphStoreStatus> {
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

#[cfg(feature = "graph-oxigraph")]
fn graph_entity_uris_for_board(conn: &Connection, board_id: &str) -> Result<Vec<EntityUri>> {
    let mut stmt = conn
        .prepare("SELECT uri FROM entities WHERE board_id=?1 ORDER BY uri ASC")
        .map_err(storage)?;
    let rows = stmt
        .query_map([board_id], |row| {
            EntityUri::new(row.get::<_, String>(0)?).map_err(sql_from_display)
        })
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

#[cfg(feature = "graph-oxigraph")]
fn graph_relations_for_entity(
    conn: &Connection,
    board_id: &str,
    entity_uri: &str,
) -> Result<Vec<Relation>> {
    let mut stmt = conn
        .prepare(
            "SELECT r.subject_uri,r.predicate,r.object_uri,r.graph_uri,r.authoritative_store,r.source_table,r.source_id,r.source_event_id,r.metadata_json,r.created_at,r.updated_at \
             FROM entity_relations r \
             JOIN entities s ON s.uri=r.subject_uri \
             WHERE s.board_id=?1 AND r.subject_uri=?2 \
             ORDER BY r.predicate ASC, r.object_uri ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map(params![board_id, entity_uri], relation_from_row)
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

#[cfg(feature = "graph-oxigraph")]
fn graph_storage(error: impl std::fmt::Display) -> KanbanError {
    KanbanError::Storage(error.to_string())
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

#[cfg(feature = "graph-oxigraph")]
fn pending_graph_outbox_for_board(
    conn: &Connection,
    board_id: &str,
    last_event_id: Option<i64>,
) -> Result<Vec<IndexOutboxRecord>> {
    let mut stmt = conn
        .prepare(
            "SELECT o.id,o.source_event_id,o.target,o.entity_uri,o.action,o.payload_json,o.status,o.attempts,o.last_error,o.created_at,o.updated_at \
             FROM index_outbox o \
             JOIN task_events e ON e.id=o.source_event_id \
             WHERE o.target IN ('oxigraph', 'all') \
               AND o.status IN ('pending', 'running', 'failed') \
               AND e.board_id=?1 \
               AND e.id <= ?2 \
             ORDER BY o.id ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map(
            params![board_id, last_event_id.unwrap_or(i64::MAX)],
            outbox_from_row,
        )
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

#[cfg(feature = "graph-oxigraph")]
fn has_pending_graph_outbox_for_board(
    conn: &Connection,
    board_id: &str,
    last_event_id: Option<i64>,
) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS( \
             SELECT 1 FROM index_outbox o \
             JOIN task_events e ON e.id=o.source_event_id \
             WHERE o.target IN ('oxigraph', 'all') \
               AND o.status IN ('pending', 'running', 'failed') \
               AND e.board_id=?1 \
               AND e.id <= ?2 \
         )",
        params![board_id, last_event_id.unwrap_or(i64::MAX)],
        |row| row.get::<_, bool>(0),
    )
    .map_err(storage)
}

#[cfg(feature = "graph-oxigraph")]
fn graph_store_path(db_path: &Path) -> PathBuf {
    kanban_local::graph_store_path(db_path.to_path_buf())
}
