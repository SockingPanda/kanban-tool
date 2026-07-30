use std::{path::PathBuf, process};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use kanban_derived_io::{
    board_id, connect_file, current_last_event_id, derived_status_by_name,
    ensure_legacy_projection_control, has_pending_graph_outbox_for_board,
    rebuild_oxigraph_with_store, sync_oxigraph_with_store,
};
use kanban_entity::{EntityUri, Predicate, Provenance, Relation};
use kanban_graph::{GraphStoreStatus, RelationGraph};
use kanban_graph_oxigraph::{
    OxigraphStore, graph_helper_error_response, graph_helper_handshake_response,
    graph_helper_neighbors_response, graph_helper_query_response, graph_helper_status_response,
};
use kanban_helper_protocol::HelperEnvelope;
use kanban_indexer::OXIGRAPH_RELATIONS_STORE;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(name = "kanban-graph-oxigraph")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Handshake,
    Status(StoreArgs),
    Rebuild(StoreArgs),
    Sync(StoreArgs),
    Neighbors(NeighborsArgs),
    Query(QueryArgs),
}

#[derive(Debug, Parser)]
struct StoreArgs {
    #[arg(long)]
    db: PathBuf,
    #[arg(long)]
    board: String,
}

#[derive(Debug, Parser)]
struct NeighborsArgs {
    #[command(flatten)]
    store: StoreArgs,
    #[arg(long)]
    entity_uri: String,
    #[arg(long)]
    predicate: Option<String>,
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Debug, Parser)]
struct QueryArgs {
    #[command(flatten)]
    store: StoreArgs,
    #[arg(long)]
    sparql: String,
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

fn main() {
    if let Err(error) = run() {
        let payload = graph_helper_error_response(error.to_string());
        if let Ok(envelope) = HelperEnvelope::new(payload).and_then(|envelope| envelope.to_json()) {
            println!("{envelope}");
        } else {
            eprintln!("{error}");
        }
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Handshake => {
            print_payload(graph_helper_handshake_response(env!("CARGO_PKG_VERSION")))
        }
        Command::Status(args) => print_payload(graph_helper_status_response(graph_status(&args)?)),
        Command::Rebuild(args) => {
            require_legacy_control(&args)?;
            let graph = graph_store(&args)?.graph;
            print_payload(graph_helper_status_response(rebuild_oxigraph_with_store(
                &args.db,
                &args.board,
                &graph,
            )?))
        }
        Command::Sync(args) => {
            require_legacy_control(&args)?;
            let graph = graph_store(&args)?.graph;
            print_payload(graph_helper_status_response(sync_oxigraph_with_store(
                &args.db,
                &args.board,
                &graph,
            )?))
        }
        Command::Neighbors(args) => {
            let resolved = graph_store(&args.store)?;
            let uri = EntityUri::new(args.entity_uri)?;
            if resolved.active.is_some() {
                require_entity_board(&args.store, uri.as_str())?;
            }
            let predicate = args.predicate.as_deref().map(parse_predicate).transpose()?;
            print_payload(graph_helper_neighbors_response(
                resolved.graph.neighbors(&uri, predicate, args.limit)?,
            )?)
        }
        Command::Query(args) => {
            let resolved = graph_store(&args.store)?;
            if resolved.active.is_some() {
                anyhow::bail!(
                    "unrestricted SPARQL is unavailable under board-scoped projection v2"
                );
            }
            print_payload(graph_helper_query_response(
                resolved.graph.query(&args.sparql, args.limit)?,
            ))
        }
    }
}

fn graph_status(args: &StoreArgs) -> Result<GraphStoreStatus> {
    let conn = connect_file(&args.db)?;
    let board_id = board_id(&conn, &args.board)?;
    let resolved = graph_store(args)?;
    if let Some(active) = resolved.active {
        let mut status = resolved.graph.status();
        status.message = format!(
            "{}; control_plane=v2 database_instance_id={} generation={} fence_epoch={} snapshot_cursor={}",
            status.message,
            active.database_instance_id,
            active.generation,
            active.fence_epoch,
            active.snapshot_cursor
        );
        return Ok(status);
    }
    let state = derived_status_by_name(&conn, OXIGRAPH_RELATIONS_STORE)?;
    let current_last_event_id = current_last_event_id(&conn, &board_id)?;
    let board_dirty = has_pending_graph_outbox_for_board(&conn, &board_id, current_last_event_id)?;
    let mut status = resolved.graph.status();
    status.message = format!(
        "{}; dirty={} last_event_id={} board_dirty={} last_error={}",
        status.message,
        state.dirty,
        state.last_event_id,
        board_dirty,
        state.last_error.as_deref().unwrap_or("none")
    );
    Ok(status)
}

struct ResolvedGraph {
    graph: OxigraphStore,
    active: Option<ActiveProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveProjection {
    database_instance_id: String,
    protocol_version: i64,
    schema_version: i64,
    generation: String,
    fingerprint: String,
    fence_epoch: i64,
    snapshot_cursor: i64,
    provider: String,
    provider_fingerprint: String,
    canonical_item_count: i64,
    canonical_digest: String,
    delivery_item_count: i64,
    delivery_digest: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PhysicalEvidence {
    manifest: PhysicalManifest,
    fingerprint: String,
    content_fingerprint: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PhysicalManifest {
    store_name: String,
    database_instance_id: String,
    protocol_version: i64,
    schema_version: i64,
    generation: String,
    fence_epoch: i64,
    snapshot_cursor: i64,
    provider: String,
    provider_fingerprint: String,
    canonical_item_count: i64,
    canonical_digest: String,
    delivery_item_count: i64,
    delivery_digest: String,
    fingerprint: Option<String>,
}

fn graph_store(args: &StoreArgs) -> Result<ResolvedGraph> {
    let conn = connect_file(&args.db)?;
    let Some(active) = active_projection(&conn)? else {
        return Ok(ResolvedGraph {
            graph: OxigraphStore::open(kanban_local::graph_store_path(args.db.clone()))?,
            active: None,
        });
    };
    let generations = kanban_local::checked_projection_store_generations_path(
        &args.db,
        &active.database_instance_id,
        OXIGRAPH_RELATIONS_STORE,
    )?;
    validate_active_generation(&generations, &active)?;
    let generation_path =
        kanban_local::projection_generation_path(&generations, &active.generation)?;
    validate_canonical_content(&conn, &generation_path)?;
    Ok(ResolvedGraph {
        graph: OxigraphStore::open(generation_path)?,
        active: Some(active),
    })
}

fn active_projection(conn: &rusqlite::Connection) -> Result<Option<ActiveProjection>> {
    let has_v2_tables = conn.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM sqlite_master
           WHERE type='table' AND name='projection_store_state'
         ) AND EXISTS(
           SELECT 1 FROM sqlite_master
           WHERE type='table' AND name='projection_deliveries'
         )",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    if !has_v2_tables {
        return Ok(None);
    }
    let state = conn
        .query_row(
            "SELECT control_plane,database_instance_id,protocol_version,schema_version,
                    active_generation,active_fingerprint,active_fence_epoch,
                    active_snapshot_cursor,active_provider,active_provider_fingerprint,
                    active_canonical_count,active_canonical_digest,active_delivery_count,
                    active_delivery_digest,building_generation,last_error,
                    (SELECT COUNT(*) FROM projection_deliveries d
                     WHERE d.store_name=s.store_name
                       AND d.status IN ('pending','running','failed','legacy_done'))
             FROM projection_store_state s WHERE store_name=?1",
            [OXIGRAPH_RELATIONS_STORE],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<i64>>(10)?,
                    row.get::<_, Option<String>>(11)?,
                    row.get::<_, Option<i64>>(12)?,
                    row.get::<_, Option<String>>(13)?,
                    row.get::<_, Option<String>>(14)?,
                    row.get::<_, Option<String>>(15)?,
                    row.get::<_, i64>(16)?,
                ))
            },
        )
        .optional()?;
    let Some((
        control_plane,
        database_instance_id,
        protocol_version,
        schema_version,
        generation,
        fingerprint,
        fence_epoch,
        snapshot_cursor,
        provider,
        provider_fingerprint,
        canonical_item_count,
        canonical_digest,
        delivery_item_count,
        delivery_digest,
        building_generation,
        last_error,
        unfinished,
    )) = state
    else {
        return Ok(None);
    };
    if control_plane != "v2" {
        return Ok(None);
    }
    if building_generation.is_some() || last_error.is_some() || unfinished != 0 {
        anyhow::bail!("Oxigraph projection v2 is not readable: rebuilding, failed, or lagging");
    }
    let required = || anyhow::anyhow!("Oxigraph projection v2 active evidence is incomplete");
    let active = ActiveProjection {
        database_instance_id,
        protocol_version,
        schema_version,
        generation: generation.ok_or_else(required)?,
        fingerprint: fingerprint.ok_or_else(required)?,
        fence_epoch: fence_epoch.ok_or_else(required)?,
        snapshot_cursor: snapshot_cursor.ok_or_else(required)?,
        provider: provider.ok_or_else(required)?,
        provider_fingerprint: provider_fingerprint.ok_or_else(required)?,
        canonical_item_count: canonical_item_count.ok_or_else(required)?,
        canonical_digest: canonical_digest.ok_or_else(required)?,
        delivery_item_count: delivery_item_count.ok_or_else(required)?,
        delivery_digest: delivery_digest.ok_or_else(required)?,
    };
    if active.protocol_version != 2
        || active.provider != "oxigraph"
        || active.provider_fingerprint != "oxigraph-relations-v2"
    {
        anyhow::bail!("Oxigraph projection v2 provider evidence is incompatible");
    }
    Ok(Some(active))
}

fn validate_physical_evidence(path: &std::path::Path, active: &ActiveProjection) -> Result<()> {
    let bytes = std::fs::read(path.join("kb-projection-meta.json"))
        .context("Oxigraph projection v2 metadata is unavailable")?;
    let physical: PhysicalEvidence =
        serde_json::from_slice(&bytes).context("Oxigraph projection v2 metadata is invalid")?;
    let manifest = physical.manifest;
    let content_fingerprint = physical_content_fingerprint(&path.join("relations.json"))?;
    let matches = manifest.store_name == OXIGRAPH_RELATIONS_STORE
        && manifest.database_instance_id == active.database_instance_id
        && manifest.protocol_version == active.protocol_version
        && manifest.schema_version == active.schema_version
        && manifest.generation == active.generation
        && manifest.fence_epoch == active.fence_epoch
        && manifest.snapshot_cursor == active.snapshot_cursor
        && manifest.provider == active.provider
        && manifest.provider_fingerprint == active.provider_fingerprint
        && manifest.canonical_item_count == active.canonical_item_count
        && manifest.canonical_digest == active.canonical_digest
        && manifest.delivery_item_count == active.delivery_item_count
        && manifest.delivery_digest == active.delivery_digest
        && physical.fingerprint == active.fingerprint
        && physical.content_fingerprint == content_fingerprint
        && manifest.fingerprint.as_deref() == Some(active.fingerprint.as_str());
    if !matches {
        anyhow::bail!("Oxigraph projection v2 physical evidence does not match SQLite");
    }
    Ok(())
}

fn validate_active_generation(
    generations: &std::path::Path,
    active: &ActiveProjection,
) -> Result<()> {
    let active_path = kanban_local::projection_generation_path(generations, &active.generation)?;
    let active_marker = active_path.join("published");
    match std::fs::symlink_metadata(&active_marker) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => {
            anyhow::bail!("Oxigraph projection v2 published marker is not a regular file");
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            anyhow::bail!("Oxigraph projection v2 active generation is not published");
        }
        Err(error) => {
            return Err(error).context("Oxigraph projection v2 published marker is unavailable");
        }
    }
    validate_published_marker(
        &active_marker,
        &active.database_instance_id,
        &active.generation,
        active.fence_epoch,
    )?;
    validate_physical_evidence(&active_path, active)?;

    let mut highest = None;
    for entry in std::fs::read_dir(&generations)
        .context("Oxigraph projection v2 generations are unavailable")?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let marker = entry.path().join("published");
        match std::fs::symlink_metadata(&marker) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "published Oxigraph marker is unavailable: {}",
                        marker.display()
                    )
                });
            }
        }
        let Ok(physical) = read_physical_evidence(&entry.path()) else {
            continue;
        };
        let manifest = physical.manifest;
        if manifest.store_name != OXIGRAPH_RELATIONS_STORE
            || manifest.provider != "oxigraph"
            || manifest.provider_fingerprint != "oxigraph-relations-v2"
            || manifest.fingerprint.as_deref() != Some(physical.fingerprint.as_str())
            || manifest.generation != entry.file_name().to_string_lossy()
            || physical.fingerprint.trim().is_empty()
        {
            continue;
        }
        if validate_published_marker(
            &marker,
            &manifest.database_instance_id,
            &manifest.generation,
            manifest.fence_epoch,
        )
        .is_err()
        {
            continue;
        }
        let candidate = (manifest.fence_epoch, manifest.generation);
        if highest.as_ref().is_none_or(|current| candidate > *current) {
            highest = Some(candidate);
        }
    }
    if highest.as_ref() != Some(&(active.fence_epoch, active.generation.clone())) {
        anyhow::bail!(
            "Oxigraph projection v2 SQLite active is not the physically published active generation"
        );
    }
    Ok(())
}

fn published_marker_contents(
    database_instance_id: &str,
    generation: &str,
    fence_epoch: i64,
) -> Vec<u8> {
    format!(
        "database_instance_id={database_instance_id}\ngeneration={generation}\nfence_epoch={fence_epoch}\n"
    )
    .into_bytes()
}

fn validate_published_marker(
    path: &std::path::Path,
    database_instance_id: &str,
    generation: &str,
    fence_epoch: i64,
) -> Result<()> {
    let actual = std::fs::read(path).with_context(|| {
        format!(
            "Oxigraph projection v2 published marker is unavailable: {}",
            path.display()
        )
    })?;
    if actual != published_marker_contents(database_instance_id, generation, fence_epoch) {
        anyhow::bail!("Oxigraph projection v2 published marker does not match generation evidence");
    }
    Ok(())
}

fn read_physical_evidence(path: &std::path::Path) -> Result<PhysicalEvidence> {
    let bytes = std::fs::read(path.join("kb-projection-meta.json"))
        .context("Oxigraph projection v2 metadata is unavailable")?;
    let physical: PhysicalEvidence =
        serde_json::from_slice(&bytes).context("Oxigraph projection v2 metadata is invalid")?;
    let actual = physical_content_fingerprint(&path.join("relations.json"))?;
    if physical.content_fingerprint != actual {
        anyhow::bail!("Oxigraph projection v2 content fingerprint mismatch");
    }
    Ok(physical)
}

fn validate_canonical_content(
    conn: &rusqlite::Connection,
    generation_path: &std::path::Path,
) -> Result<()> {
    let physical = read_physical_evidence(generation_path)?;
    let canonical = canonical_content_fingerprint(conn)?;
    if physical.content_fingerprint != canonical {
        anyhow::bail!("Oxigraph projection v2 content does not match canonical SQLite relations");
    }
    Ok(())
}

fn physical_content_fingerprint(path: &std::path::Path) -> Result<String> {
    let relations: Vec<Relation> = serde_json::from_slice(
        &std::fs::read(path).context("Oxigraph projection v2 relations are unavailable")?,
    )
    .context("Oxigraph projection v2 relations are invalid")?;
    relations_fingerprint(relations)
}

fn canonical_content_fingerprint(conn: &rusqlite::Connection) -> Result<String> {
    let cross_board: Option<(String, String, String, String)> = conn
        .query_row(
            "SELECT r.subject_uri,r.object_uri,subject.board_id,object.board_id
             FROM entity_relations r
             JOIN entities subject ON subject.uri=r.subject_uri
             JOIN entities object ON object.uri=r.object_uri
             WHERE subject.board_id IS NOT NULL
               AND object.board_id IS NOT NULL
               AND subject.board_id!=object.board_id
             ORDER BY r.subject_uri,r.predicate,r.object_uri,r.graph_uri
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    if let Some((subject, object, subject_board, object_board)) = cross_board {
        anyhow::bail!(
            "projection content contains cross-board relation {subject} ({subject_board}) -> {object} ({object_board})"
        );
    }
    let mut statement = conn.prepare(
        "SELECT r.subject_uri,r.predicate,r.object_uri,r.graph_uri,r.authoritative_store,
                r.source_table,r.source_id,r.source_event_id,r.metadata_json,r.created_at,
                r.updated_at
         FROM entity_relations r
         LEFT JOIN entities subject ON subject.uri=r.subject_uri
         LEFT JOIN entities object ON object.uri=r.object_uri
         WHERE COALESCE(subject.board_id,object.board_id) IS NOT NULL
         ORDER BY r.subject_uri,r.predicate,r.object_uri,r.graph_uri",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<i64>>(7)?,
            row.get::<_, String>(8)?,
            row.get::<_, i64>(9)?,
            row.get::<_, i64>(10)?,
        ))
    })?;
    let mut relations = Vec::new();
    for row in rows {
        let (
            subject,
            predicate,
            object,
            graph,
            authoritative_store,
            source_table,
            source_id,
            source_event_id,
            metadata_json,
            created_at,
            updated_at,
        ) = row?;
        relations.push(Relation {
            subject_uri: EntityUri::new(subject)?,
            predicate: parse_predicate(&predicate)?,
            object_uri: EntityUri::new(object)?,
            graph_uri: EntityUri::new(graph)?,
            provenance: Provenance {
                source_table,
                source_id,
                source_event_id,
                authoritative_store,
            },
            metadata_json,
            created_at,
            updated_at,
        });
    }
    relations_fingerprint(relations)
}

fn relations_fingerprint(mut relations: Vec<Relation>) -> Result<String> {
    relations.sort_by_key(relation_sort_key);
    Ok(fnv_fingerprint(&serde_json::to_vec(&relations)?))
}

fn relation_sort_key(relation: &Relation) -> String {
    format!(
        "{}\u{0}{}\u{0}{}\u{0}{}\u{0}{}\u{0}{:?}\u{0}{:?}\u{0}{:?}\u{0}{}\u{0}{}\u{0}{}",
        relation.subject_uri.as_str(),
        relation.predicate.as_str(),
        relation.object_uri.as_str(),
        relation.graph_uri.as_str(),
        relation.provenance.authoritative_store,
        relation.provenance.source_table,
        relation.provenance.source_id,
        relation.provenance.source_event_id,
        relation.metadata_json,
        relation.created_at,
        relation.updated_at
    )
}

fn fnv_fingerprint(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in (bytes.len() as u64).to_le_bytes().iter().chain(bytes) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv64:{hash:016x}")
}

fn require_legacy_control(args: &StoreArgs) -> Result<()> {
    let conn = connect_file(&args.db)?;
    ensure_legacy_projection_control(&conn, OXIGRAPH_RELATIONS_STORE)?;
    Ok(())
}

fn require_entity_board(args: &StoreArgs, entity_uri: &str) -> Result<()> {
    let conn = connect_file(&args.db)?;
    let resolved_board_id = board_id(&conn, &args.board)?;
    let entity_board = conn
        .query_row(
            "SELECT board_id FROM entities WHERE uri=?1",
            params![entity_uri],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();
    if entity_board.as_deref() != Some(resolved_board_id.as_str()) {
        anyhow::bail!("graph entity is not scoped to the resolved board");
    }
    Ok(())
}

fn parse_predicate(value: &str) -> Result<Predicate> {
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
        _ => anyhow::bail!("unknown predicate: {value}"),
    }
}

fn print_payload(payload: impl Serialize) -> Result<()> {
    println!("{}", HelperEnvelope::new(payload)?.to_json()?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projection_connection() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory SQLite");
        conn.execute_batch(
            "CREATE TABLE projection_store_state(
                 store_name TEXT PRIMARY KEY,control_plane TEXT,database_instance_id TEXT,
                 protocol_version INTEGER,schema_version INTEGER,active_generation TEXT,
                 active_fingerprint TEXT,active_fence_epoch INTEGER,
                 active_snapshot_cursor INTEGER,active_provider TEXT,
                 active_provider_fingerprint TEXT,active_canonical_count INTEGER,
                 active_canonical_digest TEXT,active_delivery_count INTEGER,
                 active_delivery_digest TEXT,building_generation TEXT,last_error TEXT
             );
             CREATE TABLE projection_deliveries(store_name TEXT,status TEXT);
             INSERT INTO projection_store_state VALUES(
                 'oxigraph_relations','v2','db_test',2,1,'gen_test','fp_test',7,11,
                 'oxigraph','oxigraph-relations-v2',3,'canonical',4,'delivery',NULL,NULL
             );",
        )
        .expect("projection fixture");
        conn
    }

    #[test]
    fn active_projection_rejects_any_unfinished_delivery() {
        let conn = projection_connection();
        assert_eq!(
            active_projection(&conn)
                .expect("complete evidence")
                .expect("active generation")
                .generation,
            "gen_test"
        );
        conn.execute(
            "INSERT INTO projection_deliveries VALUES('oxigraph_relations','pending')",
            [],
        )
        .expect("pending delivery");
        let error = active_projection(&conn).expect_err("lag must fail closed");
        assert!(error.to_string().contains("not readable"));
    }

    #[test]
    fn active_projection_preserves_pre_v26_legacy_fallback() {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory SQLite");
        assert_eq!(active_projection(&conn).expect("legacy fallback"), None);
    }

    #[test]
    fn canonical_fingerprint_includes_board_global_relations() {
        for direction in ["board_to_global", "global_to_board"] {
            let conn = rusqlite::Connection::open_in_memory().expect("in-memory SQLite");
            conn.execute_batch(
                "CREATE TABLE entities(uri TEXT PRIMARY KEY,board_id TEXT);
                 CREATE TABLE entity_relations(
                   subject_uri TEXT,predicate TEXT,object_uri TEXT,graph_uri TEXT,
                   authoritative_store TEXT,source_table TEXT,source_id TEXT,
                   source_event_id INTEGER,metadata_json TEXT,created_at INTEGER,updated_at INTEGER
                 );
                 INSERT INTO entities VALUES('kb://fixture/scoped','b_default');
                 INSERT INTO entities VALUES('kb://fixture/global',NULL);",
            )
            .expect("relation fixture");
            let (subject, object) = if direction == "board_to_global" {
                ("kb://fixture/scoped", "kb://fixture/global")
            } else {
                ("kb://fixture/global", "kb://fixture/scoped")
            };
            conn.execute(
                "INSERT INTO entity_relations VALUES(
                   ?1,'related_to',?2,'kb://graph/indexed','sqlite',
                   NULL,NULL,NULL,'{}',1,2
                 )",
                params![subject, object],
            )
            .expect("board-global relation");

            let expected = relations_fingerprint(vec![Relation {
                subject_uri: EntityUri::new(subject).expect("subject URI"),
                predicate: Predicate::RelatedTo,
                object_uri: EntityUri::new(object).expect("object URI"),
                graph_uri: EntityUri::new("kb://graph/indexed").expect("graph URI"),
                provenance: Provenance {
                    source_table: None,
                    source_id: None,
                    source_event_id: None,
                    authoritative_store: "sqlite".to_owned(),
                },
                metadata_json: "{}".to_owned(),
                created_at: 1,
                updated_at: 2,
            }])
            .expect("expected fingerprint");
            assert_eq!(
                canonical_content_fingerprint(&conn).expect("canonical fingerprint"),
                expected,
                "{direction}"
            );
        }
    }

    #[test]
    fn physical_evidence_requires_every_sqlite_field_to_match() {
        let conn = projection_connection();
        let active = active_projection(&conn)
            .expect("complete evidence")
            .expect("active generation");
        let temp = tempfile::tempdir().expect("temporary generation");
        let payload = serde_json::json!({
            "manifest": {
                "store_name": "oxigraph_relations",
                "database_instance_id": "db_test",
                "protocol_version": 2,
                "schema_version": 1,
                "generation": "gen_test",
                "fence_epoch": 7,
                "snapshot_cursor": 11,
                "provider": "oxigraph",
                "provider_fingerprint": "oxigraph-relations-v2",
                "canonical_item_count": 3,
                "canonical_digest": "canonical",
                "delivery_item_count": 4,
                "delivery_digest": "delivery",
                "fingerprint": "fp_test"
            },
            "fingerprint": "fp_test",
            "content_fingerprint": fnv_fingerprint(b"[]")
        });
        std::fs::write(temp.path().join("relations.json"), b"[]").expect("relations file");
        std::fs::write(
            temp.path().join("kb-projection-meta.json"),
            serde_json::to_vec(&payload).expect("metadata JSON"),
        )
        .expect("metadata file");
        validate_physical_evidence(temp.path(), &active).expect("exact evidence");

        let mut mismatched = payload;
        mismatched["manifest"]["delivery_digest"] = serde_json::json!("forged");
        std::fs::write(
            temp.path().join("kb-projection-meta.json"),
            serde_json::to_vec(&mismatched).expect("metadata JSON"),
        )
        .expect("metadata file");
        let error =
            validate_physical_evidence(temp.path(), &active).expect_err("mismatch must fail");
        assert!(error.to_string().contains("does not match SQLite"));

        std::fs::write(temp.path().join("relations.json"), b"[{}]").expect("tampered relations");
        let error =
            validate_physical_evidence(temp.path(), &active).expect_err("content must be bound");
        assert!(error.to_string().contains("relations are invalid"));
    }

    #[test]
    fn active_generation_requires_marker_and_highest_published_fence() {
        let conn = projection_connection();
        let active = active_projection(&conn)
            .expect("complete evidence")
            .expect("active generation");
        let temp = tempfile::tempdir().expect("temporary projection root");
        let generations = temp.path().join("generations");
        let traversal_sentinel = temp.path().join("traversal-sentinel");
        std::fs::write(&traversal_sentinel, b"must-stay").expect("sentinel");
        let mut traversal = active.clone();
        traversal.generation = "../../traversal-sentinel".to_owned();
        let error = validate_active_generation(&generations, &traversal)
            .expect_err("generation traversal must fail closed");
        assert!(
            error.to_string().contains("projection generation id"),
            "{error}"
        );
        assert_eq!(
            std::fs::read(&traversal_sentinel).expect("sentinel remains"),
            b"must-stay"
        );
        let generation = generations.join("gen_test");
        std::fs::create_dir_all(&generation).expect("generation directory");
        let payload = serde_json::json!({
            "manifest": {
                "store_name": "oxigraph_relations",
                "database_instance_id": "db_test",
                "protocol_version": 2,
                "schema_version": 1,
                "generation": "gen_test",
                "fence_epoch": 7,
                "snapshot_cursor": 11,
                "provider": "oxigraph",
                "provider_fingerprint": "oxigraph-relations-v2",
                "canonical_item_count": 3,
                "canonical_digest": "canonical",
                "delivery_item_count": 4,
                "delivery_digest": "delivery",
                "fingerprint": "fp_test"
            },
            "fingerprint": "fp_test",
            "content_fingerprint": fnv_fingerprint(b"[]")
        });
        std::fs::write(generation.join("relations.json"), b"[]").expect("relations");
        std::fs::write(
            generation.join("kb-projection-meta.json"),
            serde_json::to_vec(&payload).expect("metadata JSON"),
        )
        .expect("metadata");

        let error = validate_active_generation(&generations, &active)
            .expect_err("unpublished generation must fail closed");
        assert!(error.to_string().contains("published"));

        std::fs::write(generation.join("published"), b"published").expect("corrupt marker");
        let error = validate_active_generation(&generations, &active)
            .expect_err("corrupt marker must fail closed");
        assert!(error.to_string().contains("marker does not match"));
        std::fs::write(
            generation.join("published"),
            published_marker_contents("db_test", "gen_test", 7),
        )
        .expect("marker");
        validate_active_generation(&generations, &active).expect("published active");

        let corrupt_previous = generations.join("gen_previous");
        std::fs::create_dir_all(&corrupt_previous).expect("previous generation directory");
        std::fs::write(
            corrupt_previous.join("kb-projection-meta.json"),
            b"{not-json",
        )
        .expect("corrupt previous metadata");
        std::fs::write(corrupt_previous.join("published"), b"corrupt")
            .expect("corrupt previous marker");
        validate_active_generation(&generations, &active)
            .expect("corrupt retained previous must not hide the exact active generation");

        let newer = generations.join("gen_newer");
        std::fs::create_dir_all(&newer).expect("newer generation directory");
        let mut newer_payload = payload;
        newer_payload["manifest"]["generation"] = serde_json::json!("gen_newer");
        newer_payload["manifest"]["fence_epoch"] = serde_json::json!(8);
        std::fs::write(newer.join("relations.json"), b"[]").expect("newer relations");
        std::fs::write(
            newer.join("kb-projection-meta.json"),
            serde_json::to_vec(&newer_payload).expect("newer metadata JSON"),
        )
        .expect("newer metadata");
        std::fs::write(
            newer.join("published"),
            published_marker_contents("db_test", "gen_newer", 8),
        )
        .expect("newer marker");
        let error = validate_active_generation(&generations, &active)
            .expect_err("newer published generation must win");
        assert!(error.to_string().contains("physically published active"));
    }
}
