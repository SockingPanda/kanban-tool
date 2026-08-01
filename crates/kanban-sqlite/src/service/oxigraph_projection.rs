use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use kanban_core::{Clock, KanbanError, Result, SystemClock};
use kanban_entity::{EntityUri, Predicate, Provenance, Relation};
use kanban_graph_oxigraph::OxigraphStore;
use kanban_indexer::OXIGRAPH_RELATIONS_STORE;
use kanban_local::{
    durable_create_dir_all, durable_create_new_file, durable_publish_directory,
    durable_quarantine_entry, durable_remove_directory, durable_replace_file_contents,
    durable_sync_directory,
};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use super::{
    ProjectionArtifactEvidence, ProjectionArtifactManifest, ProjectionBatch,
    ProjectionBatchReceipt, ProjectionDestructiveAuthority, ProjectionGenerationBinding,
    ProjectionGenerationRole, ProjectionPublishReceipt, ProjectionSnapshot, ProjectionStoreBackend,
    ProjectionStoreDescriptor, storage, validate_board_rebuild_delivery,
};

pub(crate) const OXIGRAPH_PROJECTION_PROVIDER: &str = "oxigraph";
pub(crate) const OXIGRAPH_PROJECTION_PROVIDER_FINGERPRINT: &str = "oxigraph-relations-v2";
const OXIGRAPH_PROJECTION_HELPER_LOCK: &str = "oxigraph_relations-projection-helper";
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
        let _authority_guard = crate::db::acquire_derived_store_read_guard(
            &self.db_path,
            OXIGRAPH_PROJECTION_HELPER_LOCK,
        )?;
        self.inspect_published_while_helper_locked()
    }

    fn inspect_published_while_helper_locked(&self) -> Result<Vec<ProjectionArtifactEvidence>> {
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
            let evidence = match self.inspect_generation_while_helper_locked(&generation) {
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
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            OXIGRAPH_PROJECTION_HELPER_LOCK,
        )?;
        self.prepare_snapshot_with_failpoint_while_helper_locked(snapshot, &mut failpoint)
    }

    fn prepare_snapshot_with_failpoint_while_helper_locked(
        &self,
        snapshot: &ProjectionSnapshot,
        failpoint: &mut impl FnMut(OxigraphPreparePhase) -> Result<()>,
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
        match fs::symlink_metadata(&path) {
            Ok(metadata) if !metadata.is_dir() => {
                return Err(KanbanError::Conflict(format!(
                    "Oxigraph generation {generation} has a non-directory entry; fenced recovery is required before prepare"
                )));
            }
            Ok(_) => match self.inspect_generation_while_helper_locked(generation) {
                Ok(Some(existing)) if existing == evidence => return Ok(evidence),
                Ok(Some(_)) | Ok(None) | Err(_) => {
                    return Err(KanbanError::Conflict(format!(
                        "Oxigraph generation {generation} is not safely reusable; fenced recovery is required before prepare"
                    )));
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_storage(error)),
        }
        // A failed prepare may leave a partial staged generation behind. Keep
        // it as recovery evidence; this API has no opaque owner/token
        // capability with which to authorize deleting or quarantining it.
        match fs::symlink_metadata(&staged) {
            Ok(_) => {
                return Err(KanbanError::Conflict(format!(
                    "Oxigraph staged generation {generation} is not safely reusable; fenced recovery is required before prepare"
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_storage(error)),
        }
        durable_create_dir_all(&self.generations_root()).map_err(io_storage)?;
        fs::create_dir(&staged).map_err(io_storage)?;
        OxigraphStore::replace(&staged, &relations).map_err(graph_storage)?;
        failpoint(OxigraphPreparePhase::RelationsPublished)?;
        write_physical_metadata(&staged, &evidence)?;
        failpoint(OxigraphPreparePhase::MetadataPublished)?;
        durable_publish_directory(&staged, &path).map_err(io_storage)?;
        Ok(evidence)
    }

    fn apply_batch_while_helper_locked(
        &self,
        batch: &ProjectionBatch,
    ) -> Result<ProjectionBatchReceipt> {
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
            .inspect_generation_while_helper_locked(&batch.target_generation)?
            .ok_or_else(|| {
                KanbanError::Conflict(format!(
                    "Oxigraph target generation {} does not exist",
                    batch.target_generation
                ))
            })?;
        let conn = crate::db::connect_file(&self.db_path)?;
        let rebuild_boards = authorized_board_rebuilds(&conn, batch)?;
        // Validate and map every delivery before mutating the graph.  This
        // keeps a valid board rebuild from partially applying when a later
        // task/run/action in the same claimed batch is malformed.
        let subjects = affected_subjects(&conn, batch)?;
        let legacy_missing_deletions = legacy_missing_task_deletions(&conn, batch)?;
        // Prefetch every canonical board/entity relation snapshot before
        // opening or mutating Oxigraph.  A later board read failure must not
        // leave an earlier board replacement partially applied.
        let board_replacements = rebuild_boards
            .iter()
            .map(|board_id| {
                let entity_uris = board_entity_uris(&conn, board_id)?;
                let mut relations = Vec::new();
                for entity_uri in &entity_uris {
                    relations.extend(relations_for_subject(&conn, board_id, entity_uri.as_str())?);
                }
                Ok((board_id.clone(), entity_uris, relations))
            })
            .collect::<Result<Vec<_>>>()?;
        let subject_replacements = subjects
            .iter()
            .map(|(board_id, subject_uri)| {
                let entity_uri = EntityUri::new(subject_uri.clone())
                    .map_err(|error| KanbanError::Conflict(error.to_string()))?;
                let relations = relations_for_subject(&conn, board_id, subject_uri)?;
                Ok((entity_uri, relations))
            })
            .collect::<Result<Vec<_>>>()?;
        let graph = OxigraphStore::open(&path).map_err(graph_storage)?;
        for (board_id, subject_uri) in &legacy_missing_deletions {
            let board_uri = EntityUri::new(format!("kb://board/{board_id}"))
                .map_err(|error| KanbanError::Conflict(error.to_string()))?;
            let subject_uri = EntityUri::new(subject_uri.clone())
                .map_err(|error| KanbanError::Conflict(error.to_string()))?;
            graph
                .validate_board_scoped_subject(&board_uri, &subject_uri)
                .map_err(graph_storage)?;
        }
        for (board_id, entity_uris, board_relations) in board_replacements {
            let board_uri = EntityUri::new(format!("kb://board/{board_id}"))
                .map_err(|error| KanbanError::Conflict(error.to_string()))?;
            graph
                .replace_board_entities(&board_uri, &entity_uris, &board_relations)
                .map_err(graph_storage)?;
        }
        let mut entity_uris = Vec::with_capacity(subject_replacements.len());
        let mut relations = Vec::new();
        for (entity_uri, subject_relations) in subject_replacements {
            entity_uris.push(entity_uri);
            relations.extend(subject_relations);
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

    fn publish_generation_while_helper_locked(
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
        if self.inspect_published_while_helper_locked()?.last() != expected_active {
            return Err(KanbanError::Conflict(
                "Oxigraph active generation changed before publish".to_owned(),
            ));
        }
        let stored = self
            .inspect_generation_while_helper_locked(&prepared.manifest.generation)?
            .ok_or_else(|| {
                KanbanError::Conflict("prepared Oxigraph generation is missing".to_owned())
            })?;
        if stored != *prepared {
            return Err(KanbanError::Conflict(
                "prepared Oxigraph generation readback mismatch".to_owned(),
            ));
        }
        self.repair_generation_publication_while_helper_locked(prepared)?;
        let active = self
            .inspect_published_while_helper_locked()?
            .pop()
            .ok_or_else(|| {
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

    fn inspect_generation_while_helper_locked(
        &self,
        generation: &str,
    ) -> Result<Option<ProjectionArtifactEvidence>> {
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

    fn validate_generation_publication_while_helper_locked(
        &self,
        expected: &ProjectionArtifactEvidence,
    ) -> Result<()> {
        let generation = &expected.manifest.generation;
        let stored = self
            .inspect_generation_while_helper_locked(generation)?
            .ok_or_else(|| {
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

    fn repair_generation_publication_while_helper_locked(
        &self,
        expected: &ProjectionArtifactEvidence,
    ) -> Result<()> {
        let generation = &expected.manifest.generation;
        let stored = self
            .inspect_generation_while_helper_locked(generation)?
            .ok_or_else(|| {
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OxigraphSqliteGenerationBinding {
    generation: Option<String>,
    fingerprint: Option<String>,
    fence_epoch: Option<i64>,
    snapshot_cursor: Option<i64>,
    provider: Option<String>,
    provider_fingerprint: Option<String>,
    canonical_item_count: Option<i64>,
    canonical_digest: Option<String>,
    delivery_item_count: Option<i64>,
    delivery_digest: Option<String>,
    corpus_schema: Option<String>,
    corpus_fingerprint: Option<String>,
    embedding_model: Option<String>,
    embedding_dimensions: Option<i64>,
}

impl OxigraphSqliteGenerationBinding {
    fn from_row(row: &Row<'_>, offset: usize) -> rusqlite::Result<Self> {
        Ok(Self {
            generation: row.get(offset)?,
            fingerprint: row.get(offset + 1)?,
            fence_epoch: row.get(offset + 2)?,
            snapshot_cursor: row.get(offset + 3)?,
            provider: row.get(offset + 4)?,
            provider_fingerprint: row.get(offset + 5)?,
            canonical_item_count: row.get(offset + 6)?,
            canonical_digest: row.get(offset + 7)?,
            delivery_item_count: row.get(offset + 8)?,
            delivery_digest: row.get(offset + 9)?,
            corpus_schema: row.get(offset + 10)?,
            corpus_fingerprint: row.get(offset + 11)?,
            embedding_model: row.get(offset + 12)?,
            embedding_dimensions: row.get(offset + 13)?,
        })
    }

    fn to_binding(
        &self,
        store_name: &str,
        snapshot_cursor: Option<i64>,
    ) -> Result<ProjectionGenerationBinding> {
        let generation = self.generation.clone().ok_or_else(|| {
            KanbanError::Conflict(format!(
                "Oxigraph projection store {store_name} has no generation binding"
            ))
        })?;
        let fence_epoch = self.fence_epoch.ok_or_else(|| {
            KanbanError::Conflict(format!(
                "Oxigraph projection store {store_name} has no generation fence"
            ))
        })?;
        let provider = self.provider.clone().ok_or_else(|| {
            KanbanError::Conflict(format!(
                "Oxigraph projection store {store_name} has no generation provider"
            ))
        })?;
        let provider_fingerprint = self.provider_fingerprint.clone().ok_or_else(|| {
            KanbanError::Conflict(format!(
                "Oxigraph projection store {store_name} has no provider fingerprint"
            ))
        })?;
        let canonical_item_count = self.canonical_item_count.ok_or_else(|| {
            KanbanError::Conflict(format!(
                "Oxigraph projection store {store_name} has no canonical count"
            ))
        })?;
        let canonical_digest = self.canonical_digest.clone().ok_or_else(|| {
            KanbanError::Conflict(format!(
                "Oxigraph projection store {store_name} has no canonical digest"
            ))
        })?;
        let delivery_item_count = self.delivery_item_count.ok_or_else(|| {
            KanbanError::Conflict(format!(
                "Oxigraph projection store {store_name} has no delivery count"
            ))
        })?;
        let delivery_digest = self.delivery_digest.clone().ok_or_else(|| {
            KanbanError::Conflict(format!(
                "Oxigraph projection store {store_name} has no delivery digest"
            ))
        })?;
        let corpus = super::projection_v2::projection_corpus_from_values(
            self.corpus_schema.clone(),
            self.corpus_fingerprint.clone(),
            self.embedding_model.clone(),
            self.embedding_dimensions,
            store_name,
            "Oxigraph destructive authority",
        )?;
        Ok(ProjectionGenerationBinding {
            generation,
            fingerprint: self.fingerprint.clone(),
            fence_epoch,
            snapshot_cursor,
            provider,
            provider_fingerprint,
            canonical_count: canonical_item_count,
            canonical_digest,
            delivery_count: delivery_item_count,
            delivery_digest,
            corpus,
        })
    }
}

#[derive(Debug, Clone)]
struct OxigraphSqliteAuthorityState {
    database_instance_id: String,
    protocol_version: i64,
    schema_version: i64,
    control_plane: String,
    fence_epoch: i64,
    lease_owner: Option<String>,
    lease_token: Option<String>,
    lease_expires_at: Option<i64>,
    active: OxigraphSqliteGenerationBinding,
    previous: OxigraphSqliteGenerationBinding,
    building: OxigraphSqliteGenerationBinding,
    snapshot_cursor: i64,
    building_phase: Option<String>,
}

impl OxigraphSqliteAuthorityState {
    fn load(conn: &Connection) -> Result<Self> {
        conn.query_row(
            "SELECT database_instance_id,protocol_version,schema_version,control_plane,
                    fence_epoch,lease_owner,lease_token,lease_expires_at,
                    active_generation,active_fingerprint,active_fence_epoch,active_snapshot_cursor,
                    active_provider,active_provider_fingerprint,active_canonical_count,
                    active_canonical_digest,active_delivery_count,active_delivery_digest,
                    active_corpus_schema,active_corpus_fingerprint,active_embedding_model,
                    active_embedding_dimensions,
                    previous_generation,previous_fingerprint,previous_fence_epoch,
                    previous_snapshot_cursor,previous_provider,previous_provider_fingerprint,
                    previous_canonical_count,previous_canonical_digest,previous_delivery_count,
                    previous_delivery_digest,previous_corpus_schema,previous_corpus_fingerprint,
                    previous_embedding_model,previous_embedding_dimensions,
                    building_generation,building_fingerprint,building_fence_epoch,snapshot_cursor,
                    building_provider,building_provider_fingerprint,building_canonical_count,
                    building_canonical_digest,building_delivery_count,building_delivery_digest,
                    building_corpus_schema,building_corpus_fingerprint,building_embedding_model,
                    building_embedding_dimensions,snapshot_cursor,building_phase
             FROM projection_store_state WHERE store_name=?1",
            [OXIGRAPH_RELATIONS_STORE],
            |row| {
                Ok(Self {
                    database_instance_id: row.get(0)?,
                    protocol_version: row.get(1)?,
                    schema_version: row.get(2)?,
                    control_plane: row.get(3)?,
                    fence_epoch: row.get(4)?,
                    lease_owner: row.get(5)?,
                    lease_token: row.get(6)?,
                    lease_expires_at: row.get(7)?,
                    active: OxigraphSqliteGenerationBinding::from_row(row, 8)?,
                    previous: OxigraphSqliteGenerationBinding::from_row(row, 22)?,
                    building: OxigraphSqliteGenerationBinding::from_row(row, 36)?,
                    snapshot_cursor: row.get(50)?,
                    building_phase: row.get(51)?,
                })
            },
        )
        .optional()
        .map_err(|error| KanbanError::Storage(error.to_string()))?
        .ok_or_else(|| {
            KanbanError::Conflict(
                "Oxigraph projection store has no SQLite authority row".to_owned(),
            )
        })
    }

    fn binding_for(
        &self,
        role: ProjectionGenerationRole,
        store_name: &str,
    ) -> Result<ProjectionGenerationBinding> {
        match role {
            ProjectionGenerationRole::Active => self
                .active
                .to_binding(store_name, self.active.snapshot_cursor),
            ProjectionGenerationRole::Previous => self
                .previous
                .to_binding(store_name, self.previous.snapshot_cursor),
            ProjectionGenerationRole::Building => {
                let snapshot_cursor = if self.building_phase.as_deref() == Some("snapshotting") {
                    None
                } else {
                    Some(self.snapshot_cursor)
                };
                self.building.to_binding(store_name, snapshot_cursor)
            }
            ProjectionGenerationRole::Orphaned => Err(KanbanError::Conflict(
                "Oxigraph projection orphaned generations have no SQLite authority".to_owned(),
            )),
        }
    }
}

#[derive(Debug, Clone)]
struct OxigraphDestructiveValidation {
    role: ProjectionGenerationRole,
    current_provider_binding: bool,
}

fn oxigraph_authority_error(message: impl Into<String>) -> KanbanError {
    KanbanError::Conflict(format!(
        "Oxigraph projection destructive authority is stale or inconsistent: {}",
        message.into()
    ))
}

impl OxigraphProjectionStore {
    /// Validate the opaque capability and every SQLite generation binding before
    /// touching a physical generation. The caller may hold the generic store
    /// guard; this backend also holds its distinct helper authority guard.
    fn validate_destructive_authority(
        &self,
        generation: &str,
        authority: &ProjectionDestructiveAuthority,
    ) -> Result<OxigraphDestructiveValidation> {
        let validation = self.validate_exact_destructive_authority(generation, authority)?;
        if !validation.current_provider_binding {
            return Err(oxigraph_authority_error(
                "provider or corpus binding does not match Oxigraph",
            ));
        }
        Ok(validation)
    }

    /// Recovery is authorized by the exact historical SQLite binding, not by
    /// the provider compiled into this process. This still validates the live
    /// owner/token/lease/fence and the exact role, phase, manifest, and binding
    /// before any physical mutation.
    fn validate_recovery_destructive_authority(
        &self,
        generation: &str,
        authority: &ProjectionDestructiveAuthority,
    ) -> Result<OxigraphDestructiveValidation> {
        self.validate_exact_destructive_authority(generation, authority)
    }

    fn validate_exact_destructive_authority(
        &self,
        generation: &str,
        authority: &ProjectionDestructiveAuthority,
    ) -> Result<OxigraphDestructiveValidation> {
        let now = SystemClock.now_ms();
        if generation.trim().is_empty()
            || authority.generation != generation
            || authority.owner.trim().is_empty()
            || authority.lease_token.trim().is_empty()
            || authority.fence_epoch < 0
            || authority.lease_expires_at <= now
        {
            return Err(oxigraph_authority_error(
                "capability is incomplete or expired",
            ));
        }
        let conn = crate::db::connect_file(&self.db_path)?;
        let state = OxigraphSqliteAuthorityState::load(&conn)?;
        if state.database_instance_id != self.database_instance_id
            || state.protocol_version != 2
            || state.schema_version != 1
            || state.control_plane != "v2"
            || state.fence_epoch != authority.fence_epoch
            || state.lease_owner.as_deref() != Some(authority.owner.as_str())
            || state.lease_token.as_deref() != Some(authority.lease_token.as_str())
            || state
                .lease_expires_at
                .is_none_or(|expires_at| expires_at <= now)
        {
            return Err(oxigraph_authority_error(
                "owner, token, lease, database, protocol, or fence changed",
            ));
        }

        let candidates = [
            (
                ProjectionGenerationRole::Active,
                state.active.generation.as_deref(),
            ),
            (
                ProjectionGenerationRole::Previous,
                state.previous.generation.as_deref(),
            ),
            (
                ProjectionGenerationRole::Building,
                state.building.generation.as_deref(),
            ),
        ];
        let mut matched_role = None;
        for (role, candidate) in candidates {
            if candidate == Some(generation) {
                if matched_role.is_some() {
                    return Err(oxigraph_authority_error(
                        "generation is bound to more than one SQLite role",
                    ));
                }
                matched_role = Some(role);
            }
        }
        let role = matched_role.ok_or_else(|| {
            oxigraph_authority_error(
                "generation is not bound to an active, previous, or building role",
            )
        })?;
        if role != authority.role || authority.role == ProjectionGenerationRole::Orphaned {
            return Err(oxigraph_authority_error(
                "generation role does not match SQLite",
            ));
        }
        let binding = state.binding_for(role, OXIGRAPH_RELATIONS_STORE)?;
        if binding.generation != generation || binding != authority.expected_binding {
            return Err(oxigraph_authority_error(
                "generation binding does not match SQLite",
            ));
        }
        let phase = if role == ProjectionGenerationRole::Building {
            let phase = state.building_phase.as_deref();
            if !matches!(phase, Some("snapshotting" | "prepared" | "store_published")) {
                return Err(oxigraph_authority_error("building phase is invalid"));
            }
            phase.map(str::to_owned)
        } else {
            None
        };
        if authority.building_phase != phase {
            return Err(oxigraph_authority_error(
                "building phase does not match SQLite",
            ));
        }
        let expected_manifest = if binding.fingerprint.is_some() {
            Some(ProjectionArtifactManifest {
                store_name: OXIGRAPH_RELATIONS_STORE.to_owned(),
                database_instance_id: state.database_instance_id.clone(),
                protocol_version: state.protocol_version,
                schema_version: state.schema_version,
                generation: binding.generation.clone(),
                fence_epoch: binding.fence_epoch,
                snapshot_cursor: binding.snapshot_cursor.unwrap_or(state.snapshot_cursor),
                provider: binding.provider.clone(),
                provider_fingerprint: binding.provider_fingerprint.clone(),
                corpus: binding.corpus.clone(),
                canonical_item_count: binding.canonical_count,
                canonical_digest: binding.canonical_digest.clone(),
                delivery_item_count: binding.delivery_count,
                delivery_digest: binding.delivery_digest.clone(),
                fingerprint: binding.fingerprint.clone(),
            })
        } else {
            None
        };
        if authority.expected_manifest != expected_manifest {
            return Err(oxigraph_authority_error(
                "manifest does not match SQLite binding",
            ));
        }
        let current_provider_binding = binding.provider == OXIGRAPH_PROJECTION_PROVIDER
            && binding.provider_fingerprint == OXIGRAPH_PROJECTION_PROVIDER_FINGERPRINT
            && binding.corpus.is_none();
        Ok(OxigraphDestructiveValidation {
            role,
            current_provider_binding,
        })
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

    fn prepare_snapshot_with_authority(
        &self,
        snapshot: &ProjectionSnapshot,
        authority: &ProjectionDestructiveAuthority,
    ) -> Result<ProjectionArtifactEvidence> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            OXIGRAPH_PROJECTION_HELPER_LOCK,
        )?;
        self.validate_destructive_authority(&snapshot.manifest.generation, authority)?;
        let mut no_failpoint = |_| Ok(());
        self.prepare_snapshot_with_failpoint_while_helper_locked(snapshot, &mut no_failpoint)
    }

    fn apply_batch(&self, batch: &ProjectionBatch) -> Result<ProjectionBatchReceipt> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            OXIGRAPH_PROJECTION_HELPER_LOCK,
        )?;
        self.apply_batch_while_helper_locked(batch)
    }

    fn apply_batch_with_authority(
        &self,
        batch: &ProjectionBatch,
        authority: &ProjectionDestructiveAuthority,
    ) -> Result<ProjectionBatchReceipt> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            OXIGRAPH_PROJECTION_HELPER_LOCK,
        )?;
        self.validate_destructive_authority(&batch.target_generation, authority)?;
        self.apply_batch_while_helper_locked(batch)
    }

    fn publish_generation(
        &self,
        expected_active: Option<&ProjectionArtifactEvidence>,
        prepared: &ProjectionArtifactEvidence,
    ) -> Result<ProjectionPublishReceipt> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            OXIGRAPH_PROJECTION_HELPER_LOCK,
        )?;
        self.publish_generation_while_helper_locked(expected_active, prepared)
    }

    fn publish_generation_with_authority(
        &self,
        expected_active: Option<&ProjectionArtifactEvidence>,
        prepared: &ProjectionArtifactEvidence,
        authority: &ProjectionDestructiveAuthority,
    ) -> Result<ProjectionPublishReceipt> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            OXIGRAPH_PROJECTION_HELPER_LOCK,
        )?;
        self.validate_destructive_authority(&prepared.manifest.generation, authority)?;
        self.publish_generation_while_helper_locked(expected_active, prepared)
    }

    fn inspect_active(&self) -> Result<Option<ProjectionArtifactEvidence>> {
        Ok(self.inspect_published()?.pop())
    }

    fn inspect_generation(&self, generation: &str) -> Result<Option<ProjectionArtifactEvidence>> {
        let _authority_guard = crate::db::acquire_derived_store_read_guard(
            &self.db_path,
            OXIGRAPH_PROJECTION_HELPER_LOCK,
        )?;
        self.inspect_generation_while_helper_locked(generation)
    }

    fn validate_active_contents(&self, active: &ProjectionArtifactEvidence) -> Result<()> {
        let _authority_guard = crate::db::acquire_derived_store_read_guard(
            &self.db_path,
            OXIGRAPH_PROJECTION_HELPER_LOCK,
        )?;
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
        let _authority_guard = crate::db::acquire_derived_store_read_guard(
            &self.db_path,
            OXIGRAPH_PROJECTION_HELPER_LOCK,
        )?;
        let stored = self
            .inspect_generation_while_helper_locked(generation)?
            .ok_or_else(|| {
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

    fn validate_generation_publication_with_authority(
        &self,
        expected: &ProjectionArtifactEvidence,
        authority: &ProjectionDestructiveAuthority,
    ) -> Result<()> {
        let _authority_guard = crate::db::acquire_derived_store_read_guard(
            &self.db_path,
            OXIGRAPH_PROJECTION_HELPER_LOCK,
        )?;
        self.validate_destructive_authority(&expected.manifest.generation, authority)?;
        self.validate_generation_publication_while_helper_locked(expected)
    }

    fn repair_generation_publication(&self, expected: &ProjectionArtifactEvidence) -> Result<()> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            OXIGRAPH_PROJECTION_HELPER_LOCK,
        )?;
        self.repair_generation_publication_while_helper_locked(expected)
    }

    fn repair_generation_publication_with_authority(
        &self,
        expected: &ProjectionArtifactEvidence,
        authority: &ProjectionDestructiveAuthority,
    ) -> Result<()> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            OXIGRAPH_PROJECTION_HELPER_LOCK,
        )?;
        self.validate_destructive_authority(&expected.manifest.generation, authority)?;
        self.repair_generation_publication_while_helper_locked(expected)
    }

    fn quarantine_generation(&self, generation: &str) -> Result<()> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            OXIGRAPH_PROJECTION_HELPER_LOCK,
        )?;
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
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            OXIGRAPH_PROJECTION_HELPER_LOCK,
        )?;
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

    fn quarantine_generation_fenced(
        &self,
        generation: &str,
        authority: &ProjectionDestructiveAuthority,
    ) -> Result<()> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            OXIGRAPH_PROJECTION_HELPER_LOCK,
        )?;
        self.validate_managed_ancestors(false)?;
        let path = self.checked_generation_path(generation)?;
        let validation = self.validate_recovery_destructive_authority(generation, authority)?;

        // Keep an exact, readable canonical active artifact protected. A
        // corrupt or mismatched artifact is the recovery case this operation
        // is allowed to move aside.
        if validation.current_provider_binding
            && validation.role == ProjectionGenerationRole::Active
            && let Some(expected_manifest) = &authority.expected_manifest
            && let Ok(Some(actual)) = self.inspect_generation_while_helper_locked(generation)
            && actual.manifest == *expected_manifest
            && actual.fingerprint
                == authority
                    .expected_binding
                    .fingerprint
                    .clone()
                    .unwrap_or_default()
        {
            return Err(KanbanError::Conflict(format!(
                "cannot quarantine canonical active Oxigraph generation {generation}"
            )));
        }

        match fs::symlink_metadata(&path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(io_storage(error)),
        }
        // `_authority_guard` is the backend-specific physical write fence;
        // keep the authority check immediately adjacent to this durable move
        // so no unfenced legacy path can be substituted.
        durable_quarantine_entry(&path)
            .map(|_| ())
            .map_err(io_storage)
    }

    fn abort_generation_fenced(
        &self,
        generation: &str,
        authority: &ProjectionDestructiveAuthority,
    ) -> Result<()> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            OXIGRAPH_PROJECTION_HELPER_LOCK,
        )?;
        self.validate_managed_ancestors(false)?;
        let path = self.checked_generation_path(generation)?;
        let validation = self.validate_recovery_destructive_authority(generation, authority)?;
        if validation.role == ProjectionGenerationRole::Active {
            return Err(KanbanError::Conflict(format!(
                "cannot abort canonical active Oxigraph generation {generation}"
            )));
        }
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(io_storage(error)),
        };
        match fs::symlink_metadata(self.published_marker(generation)) {
            Ok(_) => {
                return Err(KanbanError::Conflict(format!(
                    "cannot abort published Oxigraph generation {generation}"
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_storage(error)),
        }
        // See the fenced quarantine path above: this delete is made while the
        // backend-specific authority guard remains held.
        if !metadata.is_dir() {
            durable_quarantine_entry(&path).map_err(io_storage)?;
            return Ok(());
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
    Board,
    Task { task_id: String },
    Run { run_id: String, task_id: String },
}

fn authorized_board_rebuilds(
    conn: &Connection,
    batch: &ProjectionBatch,
) -> Result<BTreeSet<String>> {
    let mut boards = BTreeSet::new();
    for item in &batch.items {
        if item.entity_uri == format!("kb://board/{}", item.board_id) && item.action == "rebuild" {
            validate_board_rebuild_delivery(conn, item, "oxigraph")?;
            boards.insert(item.board_id.clone());
        }
    }
    Ok(boards)
}

fn board_entity_uris(conn: &Connection, board_id: &str) -> Result<Vec<EntityUri>> {
    let mut statement = conn
        .prepare("SELECT uri FROM entities WHERE board_id=?1 ORDER BY uri")
        .map_err(storage)?;
    let uris = statement
        .query_map([board_id], |row| row.get::<_, String>(0))
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    uris.into_iter()
        .map(|uri| EntityUri::new(uri).map_err(|error| KanbanError::Conflict(error.to_string())))
        .collect()
}

fn legacy_missing_task_deletions(
    conn: &Connection,
    batch: &ProjectionBatch,
) -> Result<BTreeSet<(String, String)>> {
    let mut missing = BTreeSet::new();
    for item in &batch.items {
        if item.source_event_id.is_some() || item.action != "delete" {
            continue;
        }
        let Some(task_id) = item
            .entity_uri
            .strip_prefix("kb://task/")
            .filter(|task_id| !task_id.is_empty() && !task_id.contains('/'))
        else {
            continue;
        };
        let canonical_board = conn
            .query_row("SELECT board_id FROM tasks WHERE id=?1", [task_id], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(storage)?;
        if canonical_board.is_none() {
            missing.insert((item.board_id.clone(), item.entity_uri.clone()));
        }
    }
    Ok(missing)
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
                let event_target = conn
                    .query_row(
                        "SELECT e.task_id,e.run_id,r.task_id
                     FROM task_events e
                     LEFT JOIN task_runs r ON r.board_id=e.board_id AND r.id=e.run_id
                     WHERE e.id=?1 AND e.board_id=?2",
                        params![event_id, item.board_id],
                        |row| {
                            Ok((
                                row.get::<_, Option<String>>(0)?,
                                row.get::<_, Option<String>>(1)?,
                                row.get::<_, Option<String>>(2)?,
                            ))
                        },
                    )
                    .optional()
                    .map_err(storage)?;
                match event_target {
                    Some((Some(task_id), _, _)) => SourceEventTarget::Task { task_id },
                    Some((None, Some(run_id), Some(task_id))) => {
                        SourceEventTarget::Run { run_id, task_id }
                    }
                    Some((None, Some(_), None)) => {
                        return Err(KanbanError::Conflict(format!(
                            "Oxigraph delivery {} source event run is missing or belongs to another board",
                            item.id
                        )));
                    }
                    Some((None, None, _)) => SourceEventTarget::Board,
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
            if item.action == "rebuild" {
                validate_board_rebuild_delivery(conn, item, "oxigraph")?;
                continue;
            }
            if matches!(&event_target, SourceEventTarget::Board) && item.action == "upsert" {
                continue;
            }
            return Err(KanbanError::Conflict(format!(
                "Oxigraph delivery {} cannot be mapped to a board-scoped {} action",
                item.id, item.action
            )));
        }

        if !matches!(item.action.as_str(), "upsert" | "delete") {
            return Err(KanbanError::Conflict(format!(
                "Oxigraph delivery {} cannot be mapped to an entity action {}",
                item.id, item.action
            )));
        }

        let task_id = item
            .entity_uri
            .strip_prefix("kb://task/")
            .filter(|task_id| !task_id.is_empty() && !task_id.contains('/'));
        let run_id = item
            .entity_uri
            .strip_prefix("kb://run/")
            .filter(|run_id| !run_id.is_empty() && !run_id.contains('/'));
        if task_id.is_none() && run_id.is_none() {
            return Err(KanbanError::Conflict(format!(
                "Oxigraph delivery {} cannot be mapped to a board-scoped task or run",
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
        if let Some(task_id) = task_id {
            let event_matches = matches!(
                &event_target,
                SourceEventTarget::Task { task_id: event_task }
                    if event_task.as_str() == task_id
            );
            let legacy_task = matches!(&event_target, SourceEventTarget::Legacy);
            if entity_board.is_none() {
                if !(item.action == "delete" && (event_matches || legacy_task)) {
                    return Err(KanbanError::Conflict(format!(
                        "Oxigraph delivery {} task cannot be proven to belong to its board",
                        item.id
                    )));
                }
            }
            if entity_board
                .as_deref()
                .is_some_and(|board_id| board_id != item.board_id)
                || (!event_matches && !legacy_task)
            {
                return Err(KanbanError::Conflict(format!(
                    "Oxigraph delivery {} cannot be mapped to its source event entity",
                    item.id
                )));
            }
        } else if let Some(run_id) = run_id {
            let run = conn
                .query_row(
                    "SELECT r.board_id,r.task_id,t.board_id
                     FROM task_runs r
                     LEFT JOIN tasks t ON t.id=r.task_id
                     WHERE r.id=?1",
                    [run_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    },
                )
                .optional()
                .map_err(storage)?;
            let Some((run_board, Some(task_id), Some(task_board))) = run else {
                return Err(KanbanError::Conflict(format!(
                    "Oxigraph delivery {} run cannot be proven to belong to its board",
                    item.id
                )));
            };
            let event_matches = matches!(
                &event_target,
                SourceEventTarget::Run {
                    task_id: event_task,
                    run_id: event_run,
                } if event_task.as_str() == task_id && event_run.as_str() == run_id
            );
            if run_board != item.board_id
                || task_board != item.board_id
                || !event_matches
                || entity_board.is_none()
                || entity_board
                    .as_deref()
                    .is_some_and(|board_id| board_id != item.board_id)
            {
                return Err(KanbanError::Conflict(format!(
                    "Oxigraph delivery {} cannot be mapped to its source event run",
                    item.id
                )));
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
    use crate::service::{ProjectionDelivery, ProjectionSnapshotRecord, ProjectionStoreBackend};
    use rusqlite::params;

    fn store(temp: &tempfile::TempDir) -> OxigraphProjectionStore {
        let db_path = temp.path().join("kanban.db");
        crate::init::init_database(&db_path, "oxigraph-projection-test").unwrap();
        let store = OxigraphProjectionStore::new_bound(db_path, "db_test".to_owned()).unwrap();
        drop(
            crate::db::acquire_derived_store_write_guard(
                &store.db_path,
                OXIGRAPH_PROJECTION_HELPER_LOCK,
            )
            .unwrap(),
        );
        store
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
    fn affected_subjects_accepts_legacy_tasks_and_rejects_invalid_actions() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE tasks (id TEXT PRIMARY KEY, board_id TEXT NOT NULL);
             CREATE TABLE task_events (id INTEGER PRIMARY KEY, board_id TEXT NOT NULL, task_id TEXT, run_id TEXT);
             CREATE TABLE task_runs (id TEXT PRIMARY KEY, board_id TEXT NOT NULL, task_id TEXT);
             CREATE TABLE entities (uri TEXT PRIMARY KEY, board_id TEXT);
             INSERT INTO tasks(id,board_id) VALUES ('t_one','default');
             INSERT INTO task_runs(id,board_id,task_id) VALUES ('r_one','default','t_one');
             INSERT INTO task_events(id,board_id,task_id) VALUES (1,'default','t_one');
             INSERT INTO task_events(id,board_id) VALUES (2,'default');
             INSERT INTO task_events(id,board_id,run_id) VALUES (3,'default','r_one');
             INSERT INTO tasks(id,board_id) VALUES ('t_other','other');
             INSERT INTO entities(uri,board_id) VALUES
               ('kb://task/t_one','default'),('kb://task/t_other','other'),
               ('kb://run/r_one','default');",
        )
        .unwrap();

        let batch =
            |entity_uri: &str, source_event_id: Option<i64>, action: &str| ProjectionBatch {
                store_name: OXIGRAPH_RELATIONS_STORE.to_owned(),
                database_instance_id: "db_test".to_owned(),
                protocol_version: 2,
                schema_version: 1,
                provider: OXIGRAPH_PROJECTION_PROVIDER.to_owned(),
                provider_fingerprint: OXIGRAPH_PROJECTION_PROVIDER_FINGERPRINT.to_owned(),
                corpus: None,
                owner: "owner".to_owned(),
                lease_token: "lease".to_owned(),
                fence_epoch: 1,
                target_generation: "gen_test".to_owned(),
                claim_token: "claim".to_owned(),
                claim_expires_at: i64::MAX,
                items: vec![ProjectionDelivery {
                    id: 1,
                    outbox_id: 1,
                    store_name: OXIGRAPH_RELATIONS_STORE.to_owned(),
                    board_id: "default".to_owned(),
                    source_event_id,
                    cursor: 1,
                    action: action.to_owned(),
                    entity_uri: entity_uri.to_owned(),
                    payload_json: "{}".to_owned(),
                    attempts: 0,
                }],
            };

        assert!(affected_subjects(&conn, &batch("kb://task/t_one", Some(1), "upsert")).is_ok());
        assert!(affected_subjects(&conn, &batch("kb://task/t_one", None, "upsert")).is_ok());
        assert!(affected_subjects(&conn, &batch("kb://task/t_one", None, "delete")).is_ok());
        assert!(affected_subjects(&conn, &batch("kb://task/t_missing", None, "delete")).is_ok());
        assert!(affected_subjects(&conn, &batch("kb://run/r_one", Some(3), "upsert")).is_ok());
        let error =
            affected_subjects(&conn, &batch("kb://task/t_one", Some(3), "upsert")).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("cannot be mapped to its source event entity"),
            "{error}"
        );
        let error = affected_subjects(&conn, &batch("kb://run/r_one", None, "upsert")).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("cannot be mapped to its source event run"),
            "{error}"
        );
        for invalid in [
            batch("kb://task/t_one", Some(1), "rebuild"),
            batch("kb://run/r_one", Some(3), "rebuild"),
            batch("kb://board/default", Some(2), "delete"),
            batch("kb://task/t_other", None, "upsert"),
            batch("kb://task/t_missing", None, "upsert"),
        ] {
            let error = affected_subjects(&conn, &invalid).unwrap_err();
            assert!(error.to_string().contains("cannot be mapped"), "{error}");
        }
    }

    fn fenced_fixture(
        temp: &tempfile::TempDir,
        generation: &str,
    ) -> (
        OxigraphProjectionStore,
        ProjectionArtifactEvidence,
        ProjectionDestructiveAuthority,
    ) {
        let db_path = temp.path().join("kanban.db");
        crate::init::init_database(&db_path, "oxigraph-fenced-test").unwrap();
        let conn = crate::db::connect_file(&db_path).unwrap();
        let database_instance_id: String = conn
            .query_row(
                "SELECT database_instance_id FROM projection_database WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let store =
            OxigraphProjectionStore::new_bound(db_path, database_instance_id.clone()).unwrap();
        let mut snapshot = snapshot(generation);
        snapshot.manifest.database_instance_id = database_instance_id;
        let evidence = store.prepare_snapshot(&snapshot).unwrap();
        conn.execute(
            "UPDATE projection_store_state
             SET control_plane='v2',fence_epoch=?1,lease_owner=?2,lease_token=?3,
                 lease_expires_at=?4,building_generation=?5,building_fingerprint=?6,
                 building_fence_epoch=?7,building_provider=?8,
                 building_provider_fingerprint=?9,building_canonical_count=?10,
                 building_canonical_digest=?11,building_delivery_count=?12,
                 building_delivery_digest=?13,snapshot_cursor=?14,building_phase='prepared'
             WHERE store_name=?15",
            params![
                9_i64,
                "fenced-owner",
                "fenced-token",
                i64::MAX,
                evidence.manifest.generation,
                evidence.fingerprint,
                evidence.manifest.fence_epoch,
                evidence.manifest.provider,
                evidence.manifest.provider_fingerprint,
                evidence.manifest.canonical_item_count,
                evidence.manifest.canonical_digest,
                evidence.manifest.delivery_item_count,
                evidence.manifest.delivery_digest,
                evidence.manifest.snapshot_cursor,
                OXIGRAPH_RELATIONS_STORE,
            ],
        )
        .unwrap();
        let manifest = evidence.manifest.clone();
        let authority = ProjectionDestructiveAuthority {
            owner: "fenced-owner".to_owned(),
            lease_token: "fenced-token".to_owned(),
            fence_epoch: 9,
            lease_expires_at: i64::MAX,
            role: ProjectionGenerationRole::Building,
            generation: manifest.generation.clone(),
            expected_manifest: Some(manifest.clone()),
            expected_binding: ProjectionGenerationBinding {
                generation: manifest.generation.clone(),
                fingerprint: manifest.fingerprint.clone(),
                fence_epoch: manifest.fence_epoch,
                snapshot_cursor: Some(manifest.snapshot_cursor),
                provider: manifest.provider.clone(),
                provider_fingerprint: manifest.provider_fingerprint.clone(),
                canonical_count: manifest.canonical_item_count,
                canonical_digest: manifest.canonical_digest.clone(),
                delivery_count: manifest.delivery_item_count,
                delivery_digest: manifest.delivery_digest.clone(),
                corpus: None,
            },
            building_phase: Some("prepared".to_owned()),
        };
        (store, evidence, authority)
    }

    fn historical_fenced_authority(
        store: &OxigraphProjectionStore,
        mut authority: ProjectionDestructiveAuthority,
        provider_fingerprint: &str,
        corpus: Option<crate::service::ProjectionCorpusMetadata>,
    ) -> ProjectionDestructiveAuthority {
        let embedding_dimensions = corpus
            .as_ref()
            .map(|binding| i64::try_from(binding.embedding_dimensions).unwrap());
        let conn = crate::db::connect_file(&store.db_path).unwrap();
        if corpus.is_some() {
            conn.pragma_update(None, "ignore_check_constraints", true)
                .unwrap();
        }
        conn.execute(
            "UPDATE projection_store_state
             SET building_provider_fingerprint=?1,building_corpus_schema=?2,
                 building_corpus_fingerprint=?3,building_embedding_model=?4,
                 building_embedding_dimensions=?5
             WHERE store_name=?6",
            params![
                provider_fingerprint,
                corpus
                    .as_ref()
                    .map(|binding| binding.corpus_schema.as_str()),
                corpus
                    .as_ref()
                    .map(|binding| binding.corpus_fingerprint.as_str()),
                corpus
                    .as_ref()
                    .map(|binding| binding.embedding_model.as_str()),
                embedding_dimensions,
                OXIGRAPH_RELATIONS_STORE,
            ],
        )
        .unwrap();
        if corpus.is_some() {
            conn.pragma_update(None, "ignore_check_constraints", false)
                .unwrap();
        }
        if let Some(expected) = &corpus {
            let persisted: (String, String, String, i64) = crate::db::connect_file(&store.db_path)
                .unwrap()
                .query_row(
                    "SELECT building_corpus_schema,building_corpus_fingerprint,
                                building_embedding_model,building_embedding_dimensions
                         FROM projection_store_state WHERE store_name=?1",
                    [OXIGRAPH_RELATIONS_STORE],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .unwrap();
            assert_eq!(
                persisted,
                (
                    expected.corpus_schema.clone(),
                    expected.corpus_fingerprint.clone(),
                    expected.embedding_model.clone(),
                    i64::try_from(expected.embedding_dimensions).unwrap(),
                )
            );
        }
        authority.expected_binding.provider_fingerprint = provider_fingerprint.to_owned();
        authority.expected_binding.corpus = corpus.clone();
        let expected_manifest = authority
            .expected_manifest
            .as_mut()
            .expect("prepared generation manifest");
        expected_manifest.provider_fingerprint = provider_fingerprint.to_owned();
        expected_manifest.corpus = corpus;
        authority
    }

    fn historical_corpus() -> crate::service::ProjectionCorpusMetadata {
        crate::service::ProjectionCorpusMetadata {
            corpus_schema: "historical-oxigraph-corpus-v1".to_owned(),
            corpus_fingerprint: "historical-oxigraph-corpus-fingerprint".to_owned(),
            embedding_model: "historical-oxigraph-embedding".to_owned(),
            embedding_dimensions: 3,
        }
    }

    #[test]
    fn fenced_quarantine_and_abort_are_authorized_and_retry_idempotently() {
        let temp = tempfile::tempdir().unwrap();
        let (store, evidence, authority) = fenced_fixture(&temp, "gen_fenced_quarantine");
        store
            .quarantine_generation_fenced(&evidence.manifest.generation, &authority)
            .unwrap();
        assert!(
            fs::symlink_metadata(store.generation_path(&evidence.manifest.generation)).is_err()
        );
        store
            .quarantine_generation_fenced(&evidence.manifest.generation, &authority)
            .unwrap();

        let temp_abort = tempfile::tempdir().unwrap();
        let (store, evidence, authority) = fenced_fixture(&temp_abort, "gen_fenced_abort");
        store
            .abort_generation_fenced(&evidence.manifest.generation, &authority)
            .unwrap();
        assert!(
            fs::symlink_metadata(store.generation_path(&evidence.manifest.generation)).is_err()
        );
        store
            .abort_generation_fenced(&evidence.manifest.generation, &authority)
            .unwrap();
    }

    #[test]
    fn fenced_quarantine_rejects_stale_capabilities_without_physical_mutation() {
        let mutators: &[fn(&mut ProjectionDestructiveAuthority)] = &[
            |authority: &mut ProjectionDestructiveAuthority| {
                authority.owner = "stale-owner".to_owned()
            },
            |authority: &mut ProjectionDestructiveAuthority| {
                authority.lease_token = "stale-token".to_owned()
            },
            |authority: &mut ProjectionDestructiveAuthority| authority.fence_epoch += 1,
            |authority: &mut ProjectionDestructiveAuthority| {
                authority.role = ProjectionGenerationRole::Previous
            },
            |authority: &mut ProjectionDestructiveAuthority| {
                authority.expected_binding.delivery_digest = "stale-delivery".to_owned()
            },
            |authority: &mut ProjectionDestructiveAuthority| authority.lease_expires_at = 1,
        ];
        for mutate in mutators {
            let temp = tempfile::tempdir().unwrap();
            let (store, evidence, mut authority) = fenced_fixture(&temp, "gen_fenced_stale");
            mutate(&mut authority);
            let error = store
                .quarantine_generation_fenced(&evidence.manifest.generation, &authority)
                .unwrap_err();
            assert!(
                error.to_string().contains("destructive authority"),
                "{error}"
            );
            assert!(
                store
                    .generation_path(&evidence.manifest.generation)
                    .is_dir()
            );
            assert!(
                fs::read_dir(store.generations_root())
                    .unwrap()
                    .flatten()
                    .all(|entry| !entry.file_name().to_string_lossy().contains(".quarantine."))
            );
        }
    }

    #[test]
    fn historical_provider_fingerprint_requires_exact_authority_for_fenced_quarantine() {
        let temp = tempfile::tempdir().unwrap();
        let (store, evidence, authority) =
            fenced_fixture(&temp, "gen_historical_provider_quarantine");
        let authority =
            historical_fenced_authority(&store, authority, "oxigraph-provider-historical-v0", None);
        let generation = evidence.manifest.generation.as_str();
        let mut mismatched = authority.clone();
        mismatched.expected_binding.provider_fingerprint =
            "oxigraph-provider-mismatched".to_owned();

        let error = store
            .quarantine_generation_fenced(generation, &mismatched)
            .expect_err("mismatched historical authority must fail closed");
        assert!(
            error.to_string().contains("destructive authority"),
            "{error}"
        );
        assert!(store.generation_path(generation).is_dir());
        assert!(
            fs::read_dir(store.generations_root())
                .unwrap()
                .flatten()
                .all(|entry| !entry.file_name().to_string_lossy().contains(".quarantine."))
        );

        let error = store
            .repair_generation_publication_with_authority(&evidence, &authority)
            .expect_err("repair must continue to require the current provider binding");
        assert!(error.to_string().contains("provider or corpus"), "{error}");
        assert!(store.generation_path(generation).is_dir());

        store
            .quarantine_generation_fenced(generation, &authority)
            .expect("exact historical SQLite binding authorizes recovery quarantine");
        assert!(fs::symlink_metadata(store.generation_path(generation)).is_err());
    }

    #[test]
    fn historical_corpus_requires_exact_authority_for_fenced_abort() {
        let temp = tempfile::tempdir().unwrap();
        let (store, evidence, authority) = fenced_fixture(&temp, "gen_historical_corpus_abort");
        let authority = historical_fenced_authority(
            &store,
            authority,
            OXIGRAPH_PROJECTION_PROVIDER_FINGERPRINT,
            Some(historical_corpus()),
        );
        let generation = evidence.manifest.generation.as_str();
        let mut mismatched = authority.clone();
        mismatched
            .expected_binding
            .corpus
            .as_mut()
            .expect("historical corpus")
            .corpus_fingerprint = "mismatched-corpus".to_owned();

        let error = store
            .abort_generation_fenced(generation, &mismatched)
            .expect_err("mismatched historical corpus authority must fail closed");
        assert!(
            error.to_string().contains("destructive authority"),
            "{error}"
        );
        assert!(store.generation_path(generation).is_dir());

        let error = store
            .repair_generation_publication_with_authority(&evidence, &authority)
            .expect_err("repair must continue to reject a historical corpus binding");
        assert!(error.to_string().contains("provider or corpus"), "{error}");
        assert!(store.generation_path(generation).is_dir());

        store
            .abort_generation_fenced(generation, &authority)
            .expect("exact historical SQLite corpus binding authorizes fenced abort");
        assert!(fs::symlink_metadata(store.generation_path(generation)).is_err());
    }

    #[test]
    fn fenced_abort_protects_published_generation() {
        let temp = tempfile::tempdir().unwrap();
        let (store, evidence, mut authority) = fenced_fixture(&temp, "gen_fenced_published");
        store.publish_generation(None, &evidence).unwrap();
        let error = store
            .abort_generation_fenced(&evidence.manifest.generation, &authority)
            .unwrap_err();
        assert!(
            error.to_string().contains("cannot abort published"),
            "{error}"
        );
        assert!(
            store
                .generation_path(&evidence.manifest.generation)
                .is_dir()
        );

        authority.role = ProjectionGenerationRole::Active;
        let error = store
            .quarantine_generation_fenced(&evidence.manifest.generation, &authority)
            .unwrap_err();
        assert!(error.to_string().contains("generation role"), "{error}");
        assert!(
            store
                .generation_path(&evidence.manifest.generation)
                .is_dir()
        );
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
    fn prepare_failpoint_after_relations_publish_is_left_for_fenced_recovery() {
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

        let retry_error = store
            .prepare_snapshot(&snapshot("gen_partial"))
            .expect_err("partial staged generations require fenced recovery");
        assert!(
            retry_error
                .to_string()
                .contains("fenced recovery is required"),
            "{retry_error}"
        );
        assert!(staged.join("relations.json").is_file());
        assert!(!generation.exists());
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
    fn prepare_rejects_non_directory_generation_entries_without_following_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let store = store(&temp);
        fs::create_dir_all(store.generations_root()).unwrap();
        fs::write(
            store.generation_path("gen_file"),
            b"not-a-generation-directory",
        )
        .unwrap();
        let error = store.prepare_snapshot(&snapshot("gen_file")).unwrap_err();
        assert!(
            error.to_string().contains("fenced recovery is required"),
            "{error}"
        );
        assert_eq!(
            fs::read(store.generation_path("gen_file")).unwrap(),
            b"not-a-generation-directory"
        );

        let external = temp.path().join("external-generation");
        fs::create_dir(&external).unwrap();
        let sentinel = external.join("sentinel");
        fs::write(&sentinel, b"outside").unwrap();
        symlink(&external, store.generation_path("gen_symlink")).unwrap();
        let error = store
            .prepare_snapshot(&snapshot("gen_symlink"))
            .unwrap_err();
        assert!(
            error.to_string().contains("fenced recovery is required"),
            "{error}"
        );
        assert_eq!(fs::read(&sentinel).unwrap(), b"outside");
        assert!(
            fs::symlink_metadata(store.generation_path("gen_symlink"))
                .unwrap()
                .file_type()
                .is_symlink()
        );

        symlink(&external, store.staged_generation_path("gen_staged")).unwrap();
        let error = store.prepare_snapshot(&snapshot("gen_staged")).unwrap_err();
        assert!(
            error.to_string().contains("fenced recovery is required"),
            "{error}"
        );
        assert_eq!(fs::read(&sentinel).unwrap(), b"outside");
        assert!(
            fs::symlink_metadata(store.staged_generation_path("gen_staged"))
                .unwrap()
                .file_type()
                .is_symlink()
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
        let db_a_path = temp.path().join("a.db");
        let db_b_path = temp.path().join("b.db");
        crate::init::init_database(&db_a_path, "oxigraph-isolation-a").unwrap();
        crate::init::init_database(&db_b_path, "oxigraph-isolation-b").unwrap();
        let db_a = OxigraphProjectionStore::new_bound(db_a_path, "db_a".to_owned()).unwrap();
        let db_b = OxigraphProjectionStore::new_bound(db_b_path, "db_b".to_owned()).unwrap();
        assert_ne!(db_a.root, db_b.root);
        drop(
            crate::db::acquire_derived_store_write_guard(
                &db_b.db_path,
                OXIGRAPH_PROJECTION_HELPER_LOCK,
            )
            .unwrap(),
        );

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
