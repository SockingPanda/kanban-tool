use kanban_core::{Clock, KanbanError, Result, SystemClock};
use kanban_entity::{EntityUri, Predicate, Provenance, Relation};
use kanban_graph::{GraphStoreStatus, RelationGraph};
use kanban_indexer::{OXIGRAPH_RELATIONS_STORE, OutboxTarget};
use rusqlite::{Connection, Row, params};

use crate::{
    IndexOutboxRecord, board_id, connect_file, current_last_event_id, derived_status_by_name,
    mark_derived_store_failure, mark_derived_store_success, outbox_from_row, storage,
};

pub fn graph_relation_snapshot_for_board(
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

pub fn graph_entity_uris_for_board(conn: &Connection, board_id: &str) -> Result<Vec<EntityUri>> {
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

pub fn graph_relations_for_entity(
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

pub fn pending_graph_outbox_for_board(
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

pub fn has_pending_graph_outbox_for_board(
    conn: &Connection,
    board_id: &str,
    last_event_id: Option<i64>,
) -> Result<bool> {
    crate::has_pending_outbox_for_target(conn, OutboxTarget::Oxigraph, board_id, last_event_id)
}

pub fn rebuild_oxigraph_with_store(
    path: impl AsRef<std::path::Path>,
    board: &str,
    graph: &impl RelationGraph,
) -> Result<GraphStoreStatus> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let last_event_id = current_last_event_id(&conn, &board_id)?;
    let relations = graph_relation_snapshot_for_board(&conn, &board_id)?;
    let entity_uris = graph_entity_uris_for_board(&conn, &board_id)?;
    match graph
        .replace_entities(&entity_uris, &relations)
        .map_err(graph_storage)
    {
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

pub fn sync_oxigraph_with_store(
    path: impl AsRef<std::path::Path>,
    board: &str,
    graph: &impl RelationGraph,
) -> Result<GraphStoreStatus> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let last_event_id = current_last_event_id(&conn, &board_id)?;
    let state = derived_status_by_name(&conn, OXIGRAPH_RELATIONS_STORE)?;
    if !has_pending_graph_outbox_for_board(&conn, &board_id, last_event_id)? {
        return Ok(graph.status());
    }
    let jobs = pending_graph_outbox_for_board(&conn, &board_id, last_event_id)?;
    let result = if state.last_event_id == 0 || jobs.iter().any(|job| job.action == "rebuild") {
        let relations = graph_relation_snapshot_for_board(&conn, &board_id)?;
        let entity_uris = graph_entity_uris_for_board(&conn, &board_id)?;
        graph
            .replace_entities(&entity_uris, &relations)
            .map_err(graph_storage)
    } else {
        let mut affected = jobs
            .iter()
            .map(|job| job.entity_uri.clone())
            .collect::<Vec<_>>();
        affected.sort();
        affected.dedup();
        let mut result = Ok(());
        for uri in affected {
            if result.is_err() {
                break;
            }
            result = (|| -> Result<()> {
                let entity_uri = EntityUri::new(uri).map_err(graph_storage)?;
                let relations = graph_relations_for_entity(&conn, &board_id, entity_uri.as_str())?;
                if relations.is_empty() {
                    graph.delete(&entity_uri).map_err(graph_storage)?;
                } else {
                    graph.upsert(&relations).map_err(graph_storage)?;
                }
                Ok(())
            })();
        }
        result
    };
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

trait RelationGraphExt {
    fn replace_entities(
        &self,
        entity_uris: &[EntityUri],
        relations: &[Relation],
    ) -> std::result::Result<(), kanban_graph::GraphError>;
}

impl<T: RelationGraph + ?Sized> RelationGraphExt for T {
    fn replace_entities(
        &self,
        entity_uris: &[EntityUri],
        relations: &[Relation],
    ) -> std::result::Result<(), kanban_graph::GraphError> {
        for entity_uri in entity_uris {
            self.delete(entity_uri)?;
        }
        self.upsert(relations)
    }
}

fn sql_from_display(error: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(KanbanError::Storage(error.to_string())))
}

fn graph_storage(error: impl std::fmt::Display) -> KanbanError {
    KanbanError::Storage(error.to_string())
}
