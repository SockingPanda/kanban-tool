use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use kanban_core::{KanbanError, Result};
use kanban_entity::{EntityUri, Predicate, Provenance, Relation};
use kanban_graph_oxigraph::OxigraphStore;
use kanban_indexer::OXIGRAPH_RELATIONS_STORE;
use kanban_local::{
    durable_create_dir_all, durable_create_new_file, durable_publish_directory,
    durable_quarantine_entry, durable_remove_directory, durable_replace_file_contents,
    durable_sync_directory,
};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OxigraphPreparePhase {
    RelationsPublished,
    MetadataPublished,
}

#[derive(Debug, Clone)]
pub(crate) struct OxigraphProjectionStore {
    db_path: PathBuf,
    database_instance_id: String,
    root: PathBuf,
}

impl OxigraphProjectionStore {
    pub(crate) fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        let conn = super::maintenance::connect_existing_database(&db_path)?;
        let database_instance_id = conn
            .query_row(
                "SELECT database_instance_id
                 FROM projection_database WHERE singleton=1",
                [],
                |row| row.get::<_, String>(0),
            )
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
        drop(conn);
        Self::new_bound(db_path, database_instance_id)
    }

    fn new_bound(db_path: PathBuf, database_instance_id: String) -> Result<Self> {
        let generations = kanban_local::checked_projection_store_generations_path(
            &db_path,
            &database_instance_id,
            OXIGRAPH_RELATIONS_STORE,
        )
        .map_err(io_storage)?;
        let root = generations.parent().ok_or_else(|| {
            KanbanError::Storage("Oxigraph generations path has no store parent".to_owned())
        })?;
        Ok(Self {
            db_path,
            database_instance_id,
            root: root.to_path_buf(),
        })
    }

    fn validate_managed_ancestors(&self, create_missing: bool) -> Result<()> {
        let actual = if create_missing {
            kanban_local::ensure_projection_store_generations_path(
                &self.db_path,
                &self.database_instance_id,
                OXIGRAPH_RELATIONS_STORE,
            )
        } else {
            kanban_local::checked_projection_store_generations_path(
                &self.db_path,
                &self.database_instance_id,
                OXIGRAPH_RELATIONS_STORE,
            )
        }
        .map_err(io_storage)?;
        if actual != self.generations_root() {
            return Err(KanbanError::Storage(
                "Oxigraph database namespace changed after backend construction".to_owned(),
            ));
        }
        Ok(())
    }

    fn generations_root(&self) -> PathBuf {
        self.root.join(GENERATIONS_DIR)
    }

    pub(crate) fn generation_path(&self, generation: &str) -> PathBuf {
        self.generations_root().join(generation)
    }

    fn checked_generation_path(&self, generation: &str) -> Result<PathBuf> {
        kanban_local::projection_generation_path(&self.generations_root(), generation)
            .map_err(io_storage)
    }

    fn staged_generation_path(&self, generation: &str) -> PathBuf {
        self.generations_root()
            .join(format!(".{generation}.staged"))
    }

    fn checked_staged_generation_path(&self, generation: &str) -> Result<PathBuf> {
        self.checked_generation_path(generation)?;
        Ok(self.staged_generation_path(generation))
    }

    fn published_marker(&self, generation: &str) -> PathBuf {
        self.generation_path(generation).join(PUBLISHED_MARKER)
    }

    fn inspect_published(&self) -> Result<Vec<ProjectionArtifactEvidence>> {
        self.validate_managed_ancestors(false)?;
        let root = self.generations_root();
        let root_metadata = match fs::symlink_metadata(&root) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(io_storage(error)),
        };
        if !root_metadata.is_dir() {
            return Err(KanbanError::Storage(format!(
                "Oxigraph generations root is not a directory: {}",
                root.display()
            )));
        }
        let mut published = Vec::new();
        for entry in fs::read_dir(&root).map_err(io_storage)? {
            let entry = entry.map_err(io_storage)?;
            if !entry.file_type().map_err(io_storage)?.is_dir() {
                continue;
            }
            let marker = entry.path().join(PUBLISHED_MARKER);
            match fs::symlink_metadata(&marker) {
                Ok(metadata) if metadata.is_file() => {}
                Ok(_) => continue,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(io_storage(error)),
            }
            let generation = entry.file_name().to_string_lossy().into_owned();
            let evidence = match self.inspect_generation(&generation) {
                Ok(Some(evidence)) => evidence,
                Ok(None) | Err(_) => continue,
            };
            if validate_published_marker(&marker, &evidence).is_err() {
                continue;
            }
            published.push(evidence);
        }
        published.sort_by(|left, right| {
            left.manifest
                .fence_epoch
                .cmp(&right.manifest.fence_epoch)
                .then_with(|| left.manifest.generation.cmp(&right.manifest.generation))
        });
        Ok(published)
    }

    fn prepare_snapshot_with_failpoint(
        &self,
        snapshot: &ProjectionSnapshot,
        mut failpoint: impl FnMut(OxigraphPreparePhase) -> Result<()>,
    ) -> Result<ProjectionArtifactEvidence> {
        self.validate_managed_ancestors(true)?;
        if snapshot.manifest.store_name != OXIGRAPH_RELATIONS_STORE
            || snapshot.manifest.database_instance_id != self.database_instance_id
            || snapshot.manifest.corpus.is_some()
        {
            return Err(KanbanError::Conflict(
                "Oxigraph projection received a different store or database manifest".to_owned(),
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
        let generation = &evidence.manifest.generation;
        let path = self.checked_generation_path(generation)?;
        let staged = self.checked_staged_generation_path(generation)?;
        self.abort_generation(generation)?;
        durable_create_dir_all(&self.generations_root()).map_err(io_storage)?;
        match fs::symlink_metadata(&staged) {
            Ok(metadata) if metadata.is_dir() => {
                durable_remove_directory(&staged).map_err(io_storage)?;
            }
            Ok(_) => {
                durable_quarantine_entry(&staged).map_err(io_storage)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_storage(error)),
        }
        fs::create_dir(&staged).map_err(io_storage)?;
        OxigraphStore::replace(&staged, &relations).map_err(graph_storage)?;
        failpoint(OxigraphPreparePhase::RelationsPublished)?;
        write_physical_metadata(&staged, &evidence)?;
        failpoint(OxigraphPreparePhase::MetadataPublished)?;
        durable_publish_directory(&staged, &path).map_err(io_storage)?;
        Ok(evidence)
    }
}

impl ProjectionStoreBackend for OxigraphProjectionStore {
    fn descriptor(&self) -> Result<ProjectionStoreDescriptor> {
        Ok(ProjectionStoreDescriptor {
            store_name: OXIGRAPH_RELATIONS_STORE.to_owned(),
            provider: OXIGRAPH_PROJECTION_PROVIDER.to_owned(),
            provider_fingerprint: OXIGRAPH_PROJECTION_PROVIDER_FINGERPRINT.to_owned(),
            corpus: None,
        })
    }

    fn prepare_snapshot(
        &self,
        snapshot: &ProjectionSnapshot,
    ) -> Result<ProjectionArtifactEvidence> {
        self.prepare_snapshot_with_failpoint(snapshot, |_| Ok(()))
    }

    fn apply_batch(&self, batch: &ProjectionBatch) -> Result<ProjectionBatchReceipt> {
        self.validate_managed_ancestors(false)?;
        if batch.database_instance_id != self.database_instance_id {
            return Err(KanbanError::Conflict(
                "Oxigraph batch belongs to another database".to_owned(),
            ));
        }
        if batch.corpus.is_some() {
            return Err(KanbanError::Conflict(
                "Oxigraph batch has an unexpected corpus binding".to_owned(),
            ));
        }
        let path = self.checked_generation_path(&batch.target_generation)?;
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
        durable_sync_directory(&path).map_err(io_storage)?;
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
        self.validate_managed_ancestors(false)?;
        if prepared.manifest.database_instance_id != self.database_instance_id
            || expected_active.is_some_and(|expected| {
                expected.manifest.database_instance_id != self.database_instance_id
            })
        {
            return Err(KanbanError::Conflict(
                "Oxigraph publish evidence belongs to another database".to_owned(),
            ));
        }
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
        self.repair_generation_publication(prepared)?;
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
        self.validate_managed_ancestors(false)?;
        let path = self.checked_generation_path(generation)?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(io_storage(error)),
        };
        if !metadata.is_dir() {
            return Err(KanbanError::Storage(format!(
                "Oxigraph generation path is not a directory: {}",
                path.display()
            )));
        }
        let physical = read_physical_metadata(&path)?;
        validate_content_fingerprint(&path, &physical.content_fingerprint)?;
        let evidence = physical.evidence();
        validate_evidence(&evidence, generation)?;
        if evidence.manifest.database_instance_id != self.database_instance_id {
            return Err(KanbanError::Conflict(
                "Oxigraph generation belongs to another database".to_owned(),
            ));
        }
        OxigraphStore::open(&path).map_err(graph_storage)?;
        Ok(Some(evidence))
    }

    fn validate_active_contents(&self, active: &ProjectionArtifactEvidence) -> Result<()> {
        self.validate_managed_ancestors(false)?;
        if active.manifest.database_instance_id != self.database_instance_id {
            return Err(KanbanError::Conflict(
                "Oxigraph active evidence belongs to another database".to_owned(),
            ));
        }
        let path = self.checked_generation_path(&active.manifest.generation)?;
        let physical = read_physical_metadata(&path)?;
        let conn = crate::db::connect_file(&self.db_path)?;
        let canonical = canonical_content_fingerprint(&conn)?;
        if physical.content_fingerprint != canonical {
            return Err(KanbanError::Conflict(
                "Oxigraph projection content does not match canonical SQLite relations".to_owned(),
            ));
        }
        Ok(())
    }

    fn validate_generation_publication(&self, expected: &ProjectionArtifactEvidence) -> Result<()> {
        let generation = &expected.manifest.generation;
        let stored = self.inspect_generation(generation)?.ok_or_else(|| {
            KanbanError::Storage(format!("Oxigraph generation {generation} is missing"))
        })?;
        if stored != *expected {
            return Err(KanbanError::Storage(format!(
                "Oxigraph generation {generation} evidence mismatch"
            )));
        }
        let marker = self.published_marker(generation);
        let metadata = fs::symlink_metadata(&marker).map_err(io_storage)?;
        if !metadata.is_file() {
            return Err(KanbanError::Storage(format!(
                "Oxigraph published marker is not a regular file: {}",
                marker.display()
            )));
        }
        validate_published_marker(&marker, expected)
    }

    fn repair_generation_publication(&self, expected: &ProjectionArtifactEvidence) -> Result<()> {
        let generation = &expected.manifest.generation;
        let stored = self.inspect_generation(generation)?.ok_or_else(|| {
            KanbanError::Storage(format!(
                "Oxigraph generation {generation} is missing during marker repair"
            ))
        })?;
        if stored != *expected {
            return Err(KanbanError::Conflict(format!(
                "Oxigraph generation {generation} evidence mismatch during marker repair"
            )));
        }
        let marker = self.published_marker(generation);
        match fs::symlink_metadata(&marker) {
            Ok(metadata)
                if metadata.is_file() && validate_published_marker(&marker, expected).is_ok() =>
            {
                return Ok(());
            }
            Ok(_) => {
                durable_quarantine_entry(&marker).map_err(io_storage)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_storage(error)),
        }
        durable_create_new_file(&marker, &published_marker_contents(expected))
            .map_err(io_storage)?;
        validate_published_marker(&marker, expected)
    }

    fn quarantine_generation(&self, generation: &str) -> Result<()> {
        self.validate_managed_ancestors(false)?;
        let generation_path = self.checked_generation_path(generation)?;
        match fs::symlink_metadata(&generation_path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(io_storage(error)),
        }
        durable_quarantine_entry(&generation_path)
            .map(|_| ())
            .map_err(io_storage)
    }

    fn abort_generation(&self, generation: &str) -> Result<()> {
        self.validate_managed_ancestors(false)?;
        let path = self.checked_generation_path(generation)?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(io_storage(error)),
        };
        if !metadata.is_dir() {
            durable_quarantine_entry(&path).map_err(io_storage)?;
            return Ok(());
        }
        match fs::symlink_metadata(self.published_marker(generation)) {
            Ok(_) => {
                return Err(KanbanError::Conflict(format!(
                    "cannot abort published Oxigraph generation {generation}"
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_storage(error)),
        }
        durable_remove_directory(&path).map_err(io_storage)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
        || manifest.corpus.is_some()
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
    let bytes = serde_json::to_vec_pretty(value).map_err(json_storage)?;
    durable_replace_file_contents(path, &bytes).map_err(io_storage)
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

fn read_physical_metadata(path: &Path) -> Result<OxigraphPhysicalMetadata> {
    let metadata_path = path.join(METADATA_FILE);
    let bytes = fs::read(&metadata_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            KanbanError::Storage(format!(
                "Oxigraph physical metadata is missing: {}",
                metadata_path.display()
            ))
        } else {
            io_storage(error)
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        KanbanError::Storage(format!(
            "Oxigraph physical metadata is corrupt at {}: {error}",
            metadata_path.display()
        ))
    })
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
    let bytes = fs::read(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            KanbanError::Storage(format!(
                "Oxigraph projection relations are missing: {}",
                path.display()
            ))
        } else {
            io_storage(error)
        }
    })?;
    let relations: Vec<Relation> = serde_json::from_slice(&bytes).map_err(|error| {
        KanbanError::Storage(format!(
            "Oxigraph projection relations are corrupt at {}: {error}",
            path.display()
        ))
    })?;
    relations_fingerprint(relations)
}

fn published_marker_contents(evidence: &ProjectionArtifactEvidence) -> Vec<u8> {
    format!(
        "database_instance_id={}\ngeneration={}\nfence_epoch={}\n",
        evidence.manifest.database_instance_id,
        evidence.manifest.generation,
        evidence.manifest.fence_epoch
    )
    .into_bytes()
}

fn validate_published_marker(path: &Path, evidence: &ProjectionArtifactEvidence) -> Result<()> {
    let actual = fs::read(path).map_err(io_storage)?;
    if actual != published_marker_contents(evidence) {
        return Err(KanbanError::Storage(format!(
            "Oxigraph published marker does not match generation evidence: {}",
            path.display()
        )));
    }
    Ok(())
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

fn io_storage(error: std::io::Error) -> KanbanError {
    KanbanError::Storage(error.to_string())
}

fn json_storage(error: serde_json::Error) -> KanbanError {
    KanbanError::Storage(error.to_string())
}

fn graph_storage(error: impl std::fmt::Display) -> KanbanError {
    KanbanError::Storage(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::{ProjectionSnapshotRecord, ProjectionStoreBackend};

    fn store(temp: &tempfile::TempDir) -> OxigraphProjectionStore {
        OxigraphProjectionStore::new_bound(temp.path().join("kanban.db"), "db_test".to_owned())
            .unwrap()
    }

    fn snapshot(generation: &str) -> ProjectionSnapshot {
        ProjectionSnapshot {
            manifest: ProjectionArtifactManifest {
                store_name: OXIGRAPH_RELATIONS_STORE.to_owned(),
                database_instance_id: "db_test".to_owned(),
                protocol_version: 2,
                schema_version: 1,
                generation: generation.to_owned(),
                fence_epoch: 7,
                snapshot_cursor: 11,
                provider: OXIGRAPH_PROJECTION_PROVIDER.to_owned(),
                provider_fingerprint: OXIGRAPH_PROJECTION_PROVIDER_FINGERPRINT.to_owned(),
                corpus: None,
                canonical_item_count: 1,
                canonical_digest: "fnv64:canonical".to_owned(),
                delivery_item_count: 1,
                delivery_digest: "fnv64:delivery".to_owned(),
                fingerprint: None,
            },
            records: vec![ProjectionSnapshotRecord {
                board_id: "b_test".to_owned(),
                identity: "kb://task/t_child|depends_on|kb://task/t_parent|kb://graph/relations"
                    .to_owned(),
                payload_json: serde_json::json!({
                    "subject_uri": "kb://task/t_child",
                    "predicate": "depends_on",
                    "object_uri": "kb://task/t_parent",
                    "graph_uri": "kb://graph/relations",
                    "authoritative_store": "sqlite",
                    "source_table": "task_dependencies",
                    "source_id": "t_parent->t_child",
                    "source_event_id": 11,
                    "metadata_json": "{}",
                    "created_at": 10,
                    "updated_at": 11
                })
                .to_string(),
                content_hash: "fnv64:record".to_owned(),
            }],
        }
    }

    #[test]
    fn non_lance_projection_rejects_corpus_binding() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(&temp);
        let mut snapshot = snapshot("gen_unexpected_corpus");
        snapshot.manifest.corpus = Some(crate::service::ProjectionCorpusMetadata {
            corpus_schema: "task-chunks-v2".to_owned(),
            corpus_fingerprint: "corpus:unexpected".to_owned(),
            embedding_model: "unexpected".to_owned(),
            embedding_dimensions: 3,
        });

        let error = store.prepare_snapshot(&snapshot).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("different store or database manifest")
        );
        assert!(!store.generation_path("gen_unexpected_corpus").exists());
    }

    #[test]
    fn prepare_failpoint_after_relations_publish_is_detected_and_abortable() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(&temp);
        let error = store
            .prepare_snapshot_with_failpoint(&snapshot("gen_partial"), |phase| {
                if phase == OxigraphPreparePhase::RelationsPublished {
                    return Err(KanbanError::Storage(
                        "injected crash after relations publish".to_owned(),
                    ));
                }
                Ok(())
            })
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("injected crash after relations publish")
        );

        let generation = store.generation_path("gen_partial");
        let staged = store.staged_generation_path("gen_partial");
        assert!(!generation.exists());
        assert!(staged.join("relations.json").is_file());
        assert!(!staged.join(METADATA_FILE).exists());
        assert!(store.inspect_generation("gen_partial").unwrap().is_none());

        let rebuilt = store
            .prepare_snapshot(&snapshot("gen_partial"))
            .expect("retry rebuilds corrupt unpublished generation");
        assert!(!staged.exists());
        assert_eq!(
            store.inspect_generation("gen_partial").unwrap(),
            Some(rebuilt)
        );
        store.abort_generation("gen_partial").unwrap();
        assert!(!generation.exists());
        assert!(store.inspect_generation("gen_partial").unwrap().is_none());
    }

    #[test]
    fn quarantine_moves_the_whole_generation_and_preserves_evidence() {
        for published in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let store = store(&temp);
            let generation = if published {
                "gen_published_quarantine"
            } else {
                "gen_unpublished_quarantine"
            };
            let evidence = store.prepare_snapshot(&snapshot(generation)).unwrap();
            if published {
                store.publish_generation(None, &evidence).unwrap();
            }
            let generation_path = store.generation_path(generation);
            fs::write(generation_path.join("recovery-evidence"), b"preserve-me").unwrap();

            store.quarantine_generation(generation).unwrap();

            assert!(fs::symlink_metadata(&generation_path).is_err());
            assert_eq!(store.inspect_generation(generation).unwrap(), None);
            assert_eq!(store.inspect_active().unwrap(), None);
            let prefix = format!(".{generation}.quarantine.");
            let quarantined = fs::read_dir(store.generations_root())
                .unwrap()
                .flatten()
                .find(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
                .expect("quarantined generation sibling")
                .path();
            assert_eq!(
                fs::read(quarantined.join("recovery-evidence")).unwrap(),
                b"preserve-me"
            );
            assert_eq!(
                quarantined.join(PUBLISHED_MARKER).is_file(),
                published,
                "whole-directory quarantine must preserve publication evidence"
            );
        }
    }

    #[test]
    fn inspect_generation_fails_closed_for_corrupt_metadata_or_missing_relations() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(&temp);

        store.prepare_snapshot(&snapshot("gen_corrupt")).unwrap();
        let corrupt = store.generation_path("gen_corrupt");
        fs::write(corrupt.join(METADATA_FILE), b"{not-json").unwrap();
        assert!(store.inspect_generation("gen_corrupt").is_err());

        store.prepare_snapshot(&snapshot("gen_missing")).unwrap();
        let missing = store.generation_path("gen_missing");
        fs::remove_file(missing.join("relations.json")).unwrap();
        let error = store.inspect_generation("gen_missing").unwrap_err();
        assert!(
            error.to_string().contains("relations are missing"),
            "{error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn incremental_apply_does_not_follow_fixed_temp_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let store = store(&temp);
        let evidence = store
            .prepare_snapshot(&snapshot("gen_incremental"))
            .unwrap();
        Connection::open(&store.db_path).unwrap();
        let generation = store.generation_path("gen_incremental");
        let external_relations = temp.path().join("external-relations");
        let external_metadata = temp.path().join("external-metadata");
        fs::write(&external_relations, b"relations-sentinel").unwrap();
        fs::write(&external_metadata, b"metadata-sentinel").unwrap();
        let fixed_relations_temp = generation.join("relations.json.tmp");
        let fixed_metadata_temp = generation.join("kb-projection-meta.json.tmp");
        symlink(&external_relations, &fixed_relations_temp).unwrap();
        symlink(&external_metadata, &fixed_metadata_temp).unwrap();

        let batch = ProjectionBatch {
            store_name: OXIGRAPH_RELATIONS_STORE.to_owned(),
            database_instance_id: "db_test".to_owned(),
            protocol_version: evidence.manifest.protocol_version,
            schema_version: evidence.manifest.schema_version,
            provider: OXIGRAPH_PROJECTION_PROVIDER.to_owned(),
            provider_fingerprint: OXIGRAPH_PROJECTION_PROVIDER_FINGERPRINT.to_owned(),
            corpus: None,
            owner: "owner".to_owned(),
            lease_token: "please".to_owned(),
            fence_epoch: evidence.manifest.fence_epoch,
            target_generation: "gen_incremental".to_owned(),
            claim_token: "pclaim".to_owned(),
            claim_expires_at: i64::MAX,
            items: Vec::new(),
        };
        let receipt = store.apply_batch(&batch).unwrap();

        assert_eq!(receipt.applied_item_count, 0);
        assert_eq!(
            fs::read(&external_relations).unwrap(),
            b"relations-sentinel"
        );
        assert_eq!(fs::read(&external_metadata).unwrap(), b"metadata-sentinel");
        assert!(
            fs::symlink_metadata(&fixed_relations_temp)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(
            fs::symlink_metadata(&fixed_metadata_temp)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            store.inspect_generation("gen_incremental").unwrap(),
            Some(evidence)
        );
    }

    #[test]
    fn corrupt_or_non_file_marker_is_ignored_and_repairable_without_deleting_generation() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(&temp);
        let evidence = store.prepare_snapshot(&snapshot("gen_active")).unwrap();
        store.publish_generation(None, &evidence).unwrap();
        fs::write(store.published_marker("gen_active"), b"corrupt").unwrap();

        assert_eq!(store.inspect_active().unwrap(), None);
        let error = store
            .validate_generation_publication(&evidence)
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("published marker does not match generation evidence"),
            "{error}"
        );
        store.repair_generation_publication(&evidence).unwrap();
        assert_eq!(store.inspect_active().unwrap(), Some(evidence.clone()));
        assert!(store.generation_path("gen_active").is_dir());

        fs::remove_file(store.published_marker("gen_active")).unwrap();
        fs::create_dir(store.published_marker("gen_active")).unwrap();
        assert_eq!(store.inspect_active().unwrap(), None);
        store.repair_generation_publication(&evidence).unwrap();
        assert_eq!(store.inspect_active().unwrap(), Some(evidence));
        assert!(store.published_marker("gen_active").is_file());
    }

    #[test]
    fn publish_fails_closed_when_prepared_generation_directory_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(&temp);
        let evidence = store
            .prepare_snapshot(&snapshot("gen_missing_dir"))
            .unwrap();
        fs::remove_dir_all(store.generation_path("gen_missing_dir")).unwrap();

        let error = store.publish_generation(None, &evidence).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("prepared Oxigraph generation is missing"),
            "{error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn quarantine_does_not_follow_a_generation_symlink_outside_the_store() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let store = store(&temp);
        fs::create_dir_all(store.generations_root()).unwrap();
        let external = temp.path().join("external-generation");
        fs::create_dir(&external).unwrap();
        let sentinel = external.join(PUBLISHED_MARKER);
        fs::write(&sentinel, b"outside").unwrap();
        symlink(&external, store.generation_path("gen_symlink")).unwrap();

        store.quarantine_generation("gen_symlink").unwrap();

        assert_eq!(fs::read(&sentinel).unwrap(), b"outside");
        assert!(
            fs::symlink_metadata(store.generation_path("gen_symlink")).is_err(),
            "the authoritative symlink entry must be moved aside"
        );

        fs::write(store.generation_path("gen_file"), b"not-a-directory").unwrap();
        store.quarantine_generation("gen_file").unwrap();
        assert!(fs::symlink_metadata(store.generation_path("gen_file")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn prepare_recovers_non_directory_generation_entries_without_following_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let store = store(&temp);
        fs::create_dir_all(store.generations_root()).unwrap();
        fs::write(
            store.generation_path("gen_file"),
            b"not-a-generation-directory",
        )
        .unwrap();
        let file_evidence = store.prepare_snapshot(&snapshot("gen_file")).unwrap();
        assert_eq!(
            store.inspect_generation("gen_file").unwrap(),
            Some(file_evidence)
        );

        let external = temp.path().join("external-generation");
        fs::create_dir(&external).unwrap();
        let sentinel = external.join("sentinel");
        fs::write(&sentinel, b"outside").unwrap();
        symlink(&external, store.generation_path("gen_symlink")).unwrap();
        let symlink_evidence = store.prepare_snapshot(&snapshot("gen_symlink")).unwrap();
        assert_eq!(fs::read(&sentinel).unwrap(), b"outside");
        assert_eq!(
            store.inspect_generation("gen_symlink").unwrap(),
            Some(symlink_evidence)
        );

        symlink(&external, store.staged_generation_path("gen_staged")).unwrap();
        let staged_evidence = store.prepare_snapshot(&snapshot("gen_staged")).unwrap();
        assert_eq!(fs::read(&sentinel).unwrap(), b"outside");
        assert_eq!(
            store.inspect_generation("gen_staged").unwrap(),
            Some(staged_evidence)
        );
    }

    #[cfg(unix)]
    #[test]
    fn prepare_rejects_generations_root_symlink_without_touching_external_generation() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let store = store(&temp);
        fs::create_dir_all(&store.root).unwrap();
        let traversal_sentinel = temp.path().join("traversal-sentinel");
        fs::write(&traversal_sentinel, b"must-stay").unwrap();
        let error = store
            .abort_generation("../../traversal-sentinel")
            .unwrap_err();
        assert!(
            error.to_string().contains("projection generation id"),
            "{error}"
        );
        assert_eq!(fs::read(&traversal_sentinel).unwrap(), b"must-stay");

        let external = temp.path().join("external-generations");
        let external_generation = external.join("gen_external");
        fs::create_dir_all(&external_generation).unwrap();
        let sentinel = external_generation.join("sentinel");
        fs::write(&sentinel, b"outside").unwrap();
        symlink(&external, store.generations_root()).unwrap();

        let error = store
            .prepare_snapshot(&snapshot("gen_external"))
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("managed projection path component is not a directory"),
            "{error}"
        );
        assert_eq!(fs::read(&sentinel).unwrap(), b"outside");
    }

    #[test]
    fn physical_generations_are_isolated_by_database_instance_id() {
        let temp = tempfile::tempdir().unwrap();
        let db_a = OxigraphProjectionStore::new_bound(temp.path().join("a.db"), "db_a".to_owned())
            .unwrap();
        let db_b = OxigraphProjectionStore::new_bound(temp.path().join("b.db"), "db_b".to_owned())
            .unwrap();
        assert_ne!(db_a.root, db_b.root);

        let mut snapshot_a = snapshot("gen_shared");
        snapshot_a.manifest.database_instance_id = "db_a".to_owned();
        let evidence_a = db_a.prepare_snapshot(&snapshot_a).unwrap();
        db_a.publish_generation(None, &evidence_a).unwrap();
        assert_eq!(db_a.inspect_active().unwrap(), Some(evidence_a));
        assert_eq!(db_b.inspect_active().unwrap(), None);

        let legacy_root = temp
            .path()
            .join("index")
            .join("v2")
            .join(OXIGRAPH_RELATIONS_STORE);
        fs::create_dir_all(&legacy_root).unwrap();
        let sentinel = legacy_root.join("legacy-sentinel");
        fs::write(&sentinel, b"unscoped-v2-evidence").unwrap();
        let mut snapshot_b = snapshot("gen_shared");
        snapshot_b.manifest.database_instance_id = "db_b".to_owned();
        db_b.prepare_snapshot(&snapshot_b).unwrap();
        assert_eq!(fs::read(&sentinel).unwrap(), b"unscoped-v2-evidence");
    }
}
