use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use kanban_core::{KanbanError, Result};
use kanban_entity::{EntityUri, Predicate, Provenance, Relation};
use kanban_graph_oxigraph::OxigraphStore;
use kanban_indexer::OXIGRAPH_RELATIONS_STORE;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{
    ProjectionArtifactEvidence, ProjectionArtifactManifest, ProjectionBatch,
    ProjectionBatchReceipt, ProjectionPublishReceipt, ProjectionSnapshot, ProjectionStoreBackend,
    ProjectionStoreDescriptor, storage,
};

pub(crate) const OXIGRAPH_PROJECTION_PROVIDER: &str = "oxigraph";
pub(crate) const OXIGRAPH_PROJECTION_PROVIDER_FINGERPRINT: &str = "oxigraph-relations-v2";
const GENERATIONS_DIR: &str = "generations";
const METADATA_FILE: &str = "kb-projection-meta.json";
const PUBLISHED_MARKER: &str = "published";

#[derive(Debug, Clone)]
pub(crate) struct OxigraphProjectionStore {
    db_path: PathBuf,
    root: PathBuf,
}

impl OxigraphProjectionStore {
    pub(crate) fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        let root = kanban_local::projection_store_root_path(&db_path, OXIGRAPH_RELATIONS_STORE)
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
        Ok(Self { db_path, root })
    }

    fn generations_root(&self) -> PathBuf {
        self.root.join(GENERATIONS_DIR)
    }

    pub(crate) fn generation_path(&self, generation: &str) -> PathBuf {
        self.generations_root().join(generation)
    }

    fn published_marker(&self, generation: &str) -> PathBuf {
        self.generation_path(generation).join(PUBLISHED_MARKER)
    }

    fn inspect_published(&self) -> Result<Vec<ProjectionArtifactEvidence>> {
        let root = self.generations_root();
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut published = Vec::new();
        for entry in fs::read_dir(&root).map_err(io_storage)? {
            let entry = entry.map_err(io_storage)?;
            if !entry.file_type().map_err(io_storage)?.is_dir()
                || !entry.path().join(PUBLISHED_MARKER).is_file()
            {
                continue;
            }
            let generation = entry.file_name().to_string_lossy().into_owned();
            if let Ok(Some(evidence)) = self.inspect_generation(&generation) {
                published.push(evidence);
            }
        }
        published.sort_by(|left, right| {
            left.manifest
                .fence_epoch
                .cmp(&right.manifest.fence_epoch)
                .then_with(|| left.manifest.generation.cmp(&right.manifest.generation))
        });
        Ok(published)
    }
}

impl ProjectionStoreBackend for OxigraphProjectionStore {
    fn descriptor(&self) -> Result<ProjectionStoreDescriptor> {
        Ok(ProjectionStoreDescriptor {
            store_name: OXIGRAPH_RELATIONS_STORE.to_owned(),
            provider: OXIGRAPH_PROJECTION_PROVIDER.to_owned(),
            provider_fingerprint: OXIGRAPH_PROJECTION_PROVIDER_FINGERPRINT.to_owned(),
        })
    }

    fn prepare_snapshot(
        &self,
        snapshot: &ProjectionSnapshot,
    ) -> Result<ProjectionArtifactEvidence> {
        if snapshot.manifest.store_name != OXIGRAPH_RELATIONS_STORE {
            return Err(KanbanError::Conflict(
                "Oxigraph projection received a different store manifest".to_owned(),
            ));
        }
        let mut relations = Vec::with_capacity(snapshot.records.len());
        for record in &snapshot.records {
            let payload: RelationPayload =
                serde_json::from_str(&record.payload_json).map_err(json_storage)?;
            let relation = payload.into_relation()?;
            let identity = relation_identity(&relation);
            if record.identity != identity {
                return Err(KanbanError::Conflict(
                    "Oxigraph projection snapshot record identity mismatch".to_owned(),
                ));
            }
            relations.push(relation);
        }
        let fingerprint = snapshot_fingerprint(snapshot);
        let mut manifest = snapshot.manifest.clone();
        manifest.fingerprint = Some(fingerprint.clone());
        let evidence = ProjectionArtifactEvidence {
            manifest,
            fingerprint,
        };
        let path = self.generation_path(&evidence.manifest.generation);
        if path.exists() {
            fs::remove_dir_all(&path).map_err(io_storage)?;
        }
        fs::create_dir_all(&path).map_err(io_storage)?;
        OxigraphStore::replace(&path, &relations).map_err(graph_storage)?;
        write_physical_metadata(&path, &evidence)?;
        sync_directory(&path)?;
        sync_directory(&self.generations_root())?;
        Ok(evidence)
    }

    fn apply_batch(&self, batch: &ProjectionBatch) -> Result<ProjectionBatchReceipt> {
        let path = self.generation_path(&batch.target_generation);
        let evidence = self
            .inspect_generation(&batch.target_generation)?
            .ok_or_else(|| {
                KanbanError::Conflict(format!(
                    "Oxigraph target generation {} does not exist",
                    batch.target_generation
                ))
            })?;
        let conn = crate::db::connect_file(&self.db_path)?;
        let subjects = affected_subjects(&conn, batch)?;
        let graph = OxigraphStore::open(&path).map_err(graph_storage)?;
        let mut entity_uris = Vec::with_capacity(subjects.len());
        let mut relations = Vec::new();
        for (board_id, subject_uri) in subjects {
            let entity_uri = EntityUri::new(subject_uri.clone())
                .map_err(|error| KanbanError::Conflict(error.to_string()))?;
            entity_uris.push(entity_uri);
            relations.extend(relations_for_subject(&conn, &board_id, &subject_uri)?);
        }
        graph
            .replace_entities(&entity_uris, &relations)
            .map_err(graph_storage)?;
        write_physical_metadata(&path, &evidence)?;
        sync_directory(&path)?;
        Ok(ProjectionBatchReceipt {
            store_name: batch.store_name.clone(),
            database_instance_id: batch.database_instance_id.clone(),
            protocol_version: batch.protocol_version,
            schema_version: batch.schema_version,
            provider: batch.provider.clone(),
            provider_fingerprint: batch.provider_fingerprint.clone(),
            target_generation: batch.target_generation.clone(),
            lease_token: batch.lease_token.clone(),
            fence_epoch: batch.fence_epoch,
            claim_token: batch.claim_token.clone(),
            applied_item_count: batch.items.len(),
        })
    }

    fn publish_generation(
        &self,
        expected_active: Option<&ProjectionArtifactEvidence>,
        prepared: &ProjectionArtifactEvidence,
    ) -> Result<ProjectionPublishReceipt> {
        if self.inspect_active()?.as_ref() != expected_active {
            return Err(KanbanError::Conflict(
                "Oxigraph active generation changed before publish".to_owned(),
            ));
        }
        let stored = self
            .inspect_generation(&prepared.manifest.generation)?
            .ok_or_else(|| {
                KanbanError::Conflict("prepared Oxigraph generation is missing".to_owned())
            })?;
        if stored != *prepared {
            return Err(KanbanError::Conflict(
                "prepared Oxigraph generation readback mismatch".to_owned(),
            ));
        }
        let marker = self.published_marker(&prepared.manifest.generation);
        if !marker.exists() {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&marker)
                .map_err(io_storage)?;
            writeln!(
                file,
                "database_instance_id={}\ngeneration={}\nfence_epoch={}",
                prepared.manifest.database_instance_id,
                prepared.manifest.generation,
                prepared.manifest.fence_epoch
            )
            .map_err(io_storage)?;
            file.sync_all().map_err(io_storage)?;
            sync_directory(marker.parent().expect("published marker has parent"))?;
            sync_directory(&self.generations_root())?;
        }
        let active = self.inspect_active()?.ok_or_else(|| {
            KanbanError::Storage("published Oxigraph generation is not discoverable".to_owned())
        })?;
        if active != *prepared {
            return Err(KanbanError::Conflict(
                "a newer Oxigraph generation won the publish fence".to_owned(),
            ));
        }
        Ok(ProjectionPublishReceipt {
            active,
            retained_previous: expected_active.cloned(),
        })
    }

    fn inspect_active(&self) -> Result<Option<ProjectionArtifactEvidence>> {
        Ok(self.inspect_published()?.pop())
    }

    fn inspect_generation(&self, generation: &str) -> Result<Option<ProjectionArtifactEvidence>> {
        let path = self.generation_path(generation);
        if !path.exists() {
            return Ok(None);
        }
        let physical: OxigraphPhysicalMetadata =
            serde_json::from_slice(&fs::read(path.join(METADATA_FILE)).map_err(io_storage)?)
                .map_err(json_storage)?;
        validate_content_fingerprint(&path, &physical.content_fingerprint)?;
        let evidence = physical.evidence();
        validate_evidence(&evidence, generation)?;
        OxigraphStore::open(&path).map_err(graph_storage)?;
        Ok(Some(evidence))
    }

    fn validate_active_contents(&self, active: &ProjectionArtifactEvidence) -> Result<()> {
        let path = self.generation_path(&active.manifest.generation);
        let physical: OxigraphPhysicalMetadata =
            serde_json::from_slice(&fs::read(path.join(METADATA_FILE)).map_err(io_storage)?)
                .map_err(json_storage)?;
        let conn = crate::db::connect_file(&self.db_path)?;
        let canonical = canonical_content_fingerprint(&conn)?;
        if physical.content_fingerprint != canonical {
            return Err(KanbanError::Conflict(
                "Oxigraph projection content does not match canonical SQLite relations".to_owned(),
            ));
        }
        Ok(())
    }

    fn quarantine_generation(&self, generation: &str) -> Result<()> {
        let marker = self.published_marker(generation);
        if !marker.is_file() {
            return Err(KanbanError::Conflict(format!(
                "Oxigraph generation {generation} is not published"
            )));
        }
        fs::remove_file(&marker).map_err(io_storage)?;
        sync_directory(marker.parent().expect("published marker has parent"))?;
        sync_directory(&self.generations_root())?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OxigraphPhysicalMetadata {
    manifest: ProjectionArtifactManifest,
    fingerprint: String,
    content_fingerprint: String,
}

impl OxigraphPhysicalMetadata {
    fn evidence(&self) -> ProjectionArtifactEvidence {
        ProjectionArtifactEvidence {
            manifest: self.manifest.clone(),
            fingerprint: self.fingerprint.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RelationPayload {
    subject_uri: String,
    predicate: String,
    object_uri: String,
    graph_uri: String,
    authoritative_store: String,
    source_table: Option<String>,
    source_id: Option<String>,
    source_event_id: Option<i64>,
    metadata_json: String,
    created_at: i64,
    updated_at: i64,
}

enum SourceEventTarget {
    Legacy,
    Taskless,
    Task(String),
}

impl RelationPayload {
    fn into_relation(self) -> Result<Relation> {
        Ok(Relation {
            subject_uri: entity_uri(self.subject_uri)?,
            predicate: predicate(&self.predicate)?,
            object_uri: entity_uri(self.object_uri)?,
            graph_uri: entity_uri(self.graph_uri)?,
            provenance: Provenance {
                source_table: self.source_table,
                source_id: self.source_id,
                source_event_id: self.source_event_id,
                authoritative_store: self.authoritative_store,
            },
            metadata_json: self.metadata_json,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

fn affected_subjects(
    conn: &Connection,
    batch: &ProjectionBatch,
) -> Result<BTreeSet<(String, String)>> {
    let mut subjects = BTreeSet::new();
    for item in &batch.items {
        let event_target = match item.source_event_id {
            None => SourceEventTarget::Legacy,
            Some(event_id) => {
                let event_task = conn
                    .query_row(
                        "SELECT COALESCE(e.task_id,r.task_id)
                     FROM task_events e
                     LEFT JOIN task_runs r ON r.board_id=e.board_id AND r.id=e.run_id
                     WHERE e.id=?1 AND e.board_id=?2",
                        params![event_id, item.board_id],
                        |row| row.get::<_, Option<String>>(0),
                    )
                    .optional()
                    .map_err(storage)?;
                match event_task {
                    Some(Some(task_id)) => SourceEventTarget::Task(task_id),
                    Some(None) => SourceEventTarget::Taskless,
                    None => {
                        return Err(KanbanError::Conflict(format!(
                            "Oxigraph delivery {} source event is missing or belongs to another board",
                            item.id
                        )));
                    }
                }
            }
        };
        if item.entity_uri == format!("kb://board/{}", item.board_id) {
            if matches!(event_target, SourceEventTarget::Taskless) {
                continue;
            }
            return Err(KanbanError::Conflict(format!(
                "Oxigraph delivery {} cannot be mapped to a board-scoped entity",
                item.id
            )));
        }
        let entity_board = conn
            .query_row(
                "SELECT board_id FROM entities WHERE uri=?1",
                [&item.entity_uri],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(storage)?
            .flatten();
        match entity_board {
            Some(board_id) if board_id == item.board_id => match &event_target {
                SourceEventTarget::Legacy => {}
                SourceEventTarget::Task(task_id)
                    if item.entity_uri == format!("kb://task/{task_id}") => {}
                _ => {
                    return Err(KanbanError::Conflict(format!(
                        "Oxigraph delivery {} cannot be mapped to its source event entity",
                        item.id
                    )));
                }
            },
            Some(board_id) => {
                return Err(KanbanError::Conflict(format!(
                    "Oxigraph delivery {} entity belongs to board {board_id}, not {}",
                    item.id, item.board_id
                )));
            }
            None => {
                let valid_deletion = item.action == "delete"
                    && matches!(
                        &event_target,
                        SourceEventTarget::Task(task_id)
                            if item.entity_uri == format!("kb://task/{task_id}")
                    );
                if !valid_deletion {
                    return Err(KanbanError::Conflict(format!(
                        "Oxigraph delivery {} cannot be mapped to a board-scoped entity",
                        item.id
                    )));
                }
            }
        }
        entity_uri(item.entity_uri.clone())?;
        subjects.insert((item.board_id.clone(), item.entity_uri.clone()));
    }
    Ok(subjects)
}

fn relations_for_subject(
    conn: &Connection,
    board_id: &str,
    subject_uri: &str,
) -> Result<Vec<Relation>> {
    let cross_board: Option<String> = conn
        .query_row(
            "SELECT object.board_id
             FROM entity_relations r
             JOIN entities subject ON subject.uri=r.subject_uri
             JOIN entities object ON object.uri=r.object_uri
             WHERE r.subject_uri=?1 AND subject.board_id=?2
               AND object.board_id IS NOT NULL AND object.board_id!=?2
             LIMIT 1",
            params![subject_uri, board_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage)?;
    if let Some(object_board) = cross_board {
        return Err(KanbanError::Conflict(format!(
            "Oxigraph subject {subject_uri} has a cross-board relation to {object_board}"
        )));
    }
    let mut statement = conn
        .prepare(
            "SELECT r.subject_uri,r.predicate,r.object_uri,r.graph_uri,r.authoritative_store,
                    r.source_table,r.source_id,r.source_event_id,r.metadata_json,r.created_at,
                    r.updated_at
             FROM entity_relations r
             JOIN entities subject ON subject.uri=r.subject_uri
             WHERE subject.board_id=?1 AND r.subject_uri=?2
             ORDER BY r.predicate,r.object_uri,r.graph_uri",
        )
        .map_err(storage)?;
    let rows = statement
        .query_map(params![board_id, subject_uri], |row| {
            Ok(RelationPayload {
                subject_uri: row.get(0)?,
                predicate: row.get(1)?,
                object_uri: row.get(2)?,
                graph_uri: row.get(3)?,
                authoritative_store: row.get(4)?,
                source_table: row.get(5)?,
                source_id: row.get(6)?,
                source_event_id: row.get(7)?,
                metadata_json: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })
        .map_err(storage)?;
    rows.map(|row| {
        row.map_err(storage)
            .and_then(RelationPayload::into_relation)
    })
    .collect()
}

fn validate_evidence(evidence: &ProjectionArtifactEvidence, generation: &str) -> Result<()> {
    let manifest = &evidence.manifest;
    if manifest.store_name != OXIGRAPH_RELATIONS_STORE
        || manifest.provider != OXIGRAPH_PROJECTION_PROVIDER
        || manifest.provider_fingerprint != OXIGRAPH_PROJECTION_PROVIDER_FINGERPRINT
        || manifest.generation != generation
        || manifest.fingerprint.as_deref() != Some(evidence.fingerprint.as_str())
        || evidence.fingerprint.trim().is_empty()
    {
        return Err(KanbanError::Conflict(
            "Oxigraph projection metadata is inconsistent".to_owned(),
        ));
    }
    Ok(())
}

fn relation_identity(relation: &Relation) -> String {
    format!(
        "{}|{}|{}|{}",
        relation.subject_uri.as_str(),
        relation.predicate.as_str(),
        relation.object_uri.as_str(),
        relation.graph_uri.as_str()
    )
}

fn snapshot_fingerprint(snapshot: &ProjectionSnapshot) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    hash_bytes(&mut hash, snapshot.manifest.database_instance_id.as_bytes());
    hash_bytes(&mut hash, snapshot.manifest.generation.as_bytes());
    hash_bytes(&mut hash, &snapshot.manifest.fence_epoch.to_le_bytes());
    hash_bytes(&mut hash, &snapshot.manifest.snapshot_cursor.to_le_bytes());
    hash_bytes(&mut hash, snapshot.manifest.canonical_digest.as_bytes());
    hash_bytes(&mut hash, snapshot.manifest.delivery_digest.as_bytes());
    for record in &snapshot.records {
        hash_bytes(&mut hash, record.board_id.as_bytes());
        hash_bytes(&mut hash, record.identity.as_bytes());
        hash_bytes(&mut hash, record.content_hash.as_bytes());
    }
    format!("fnv64:{hash:016x}")
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in (bytes.len() as u64).to_le_bytes().iter().chain(bytes) {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

fn predicate(value: &str) -> Result<Predicate> {
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
        _ => Err(KanbanError::Conflict(format!(
            "unknown Oxigraph relation predicate {value}"
        ))),
    }
}

fn entity_uri(value: String) -> Result<EntityUri> {
    EntityUri::new(value).map_err(|error| KanbanError::Conflict(error.to_string()))
}

fn write_json_atomic(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let temp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value).map_err(json_storage)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temp)
        .map_err(io_storage)?;
    file.write_all(&bytes).map_err(io_storage)?;
    file.sync_all().map_err(io_storage)?;
    fs::rename(&temp, path).map_err(io_storage)?;
    sync_directory(path.parent().expect("metadata has generation directory"))
}

fn write_physical_metadata(path: &Path, evidence: &ProjectionArtifactEvidence) -> Result<()> {
    let content_fingerprint = physical_content_fingerprint(&path.join("relations.json"))?;
    write_json_atomic(
        &path.join(METADATA_FILE),
        &OxigraphPhysicalMetadata {
            manifest: evidence.manifest.clone(),
            fingerprint: evidence.fingerprint.clone(),
            content_fingerprint,
        },
    )
}

fn validate_content_fingerprint(path: &Path, expected: &str) -> Result<()> {
    let actual = physical_content_fingerprint(&path.join("relations.json"))?;
    if actual != expected {
        return Err(KanbanError::Conflict(
            "Oxigraph projection content fingerprint mismatch".to_owned(),
        ));
    }
    Ok(())
}

fn physical_content_fingerprint(path: &Path) -> Result<String> {
    let relations: Vec<Relation> =
        serde_json::from_slice(&fs::read(path).map_err(io_storage)?).map_err(json_storage)?;
    relations_fingerprint(relations)
}

fn canonical_content_fingerprint(conn: &Connection) -> Result<String> {
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
        .optional()
        .map_err(storage)?;
    if let Some((subject, object, subject_board, object_board)) = cross_board {
        return Err(KanbanError::Conflict(format!(
            "projection content contains cross-board relation {subject} ({subject_board}) -> {object} ({object_board})"
        )));
    }
    let mut statement = conn
        .prepare(
            "SELECT r.subject_uri,r.predicate,r.object_uri,r.graph_uri,r.authoritative_store,
                    r.source_table,r.source_id,r.source_event_id,r.metadata_json,r.created_at,
                    r.updated_at
             FROM entity_relations r
             LEFT JOIN entities subject ON subject.uri=r.subject_uri
             LEFT JOIN entities object ON object.uri=r.object_uri
             WHERE COALESCE(subject.board_id,object.board_id) IS NOT NULL
             ORDER BY r.subject_uri,r.predicate,r.object_uri,r.graph_uri",
        )
        .map_err(storage)?;
    let rows = statement
        .query_map([], |row| {
            Ok(RelationPayload {
                subject_uri: row.get(0)?,
                predicate: row.get(1)?,
                object_uri: row.get(2)?,
                graph_uri: row.get(3)?,
                authoritative_store: row.get(4)?,
                source_table: row.get(5)?,
                source_id: row.get(6)?,
                source_event_id: row.get(7)?,
                metadata_json: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })
        .map_err(storage)?;
    let relations = rows
        .map(|row| {
            row.map_err(storage)
                .and_then(RelationPayload::into_relation)
        })
        .collect::<Result<Vec<_>>>()?;
    relations_fingerprint(relations)
}

fn relations_fingerprint(mut relations: Vec<Relation>) -> Result<String> {
    relations.sort_by_key(relation_sort_key);
    let bytes = serde_json::to_vec(&relations).map_err(json_storage)?;
    let mut hash = 0xcbf29ce484222325_u64;
    hash_bytes(&mut hash, &bytes);
    Ok(format!("fnv64:{hash:016x}"))
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

fn sync_directory(path: &Path) -> Result<()> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(io_storage)
}

fn io_storage(error: std::io::Error) -> KanbanError {
    KanbanError::Storage(error.to_string())
}

fn json_storage(error: serde_json::Error) -> KanbanError {
    KanbanError::Storage(error.to_string())
}

fn graph_storage(error: impl std::fmt::Display) -> KanbanError {
    KanbanError::Storage(error.to_string())
}
