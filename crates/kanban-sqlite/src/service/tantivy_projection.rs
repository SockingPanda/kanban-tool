use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use kanban_core::{Clock, KanbanError, Result, SystemClock};
use kanban_indexer::TANTIVY_TASKS_STORE;
use kanban_local::{durable_create_new_file, durable_quarantine_entry, durable_remove_directory};
use kanban_search::{
    TaskSearchDocument,
    tantivy_backend::{
        TantivyTaskProjectionMetadata, TaskProjectionDocumentKey,
        prepare_task_projection_generation, search_task_projection_generation_against,
        sync_task_projection_generation, validate_task_projection_generation,
    },
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{
    ProjectionArtifactEvidence, ProjectionArtifactManifest, ProjectionBatch,
    ProjectionBatchReceipt, ProjectionDestructiveAuthority, ProjectionGenerationBinding,
    ProjectionGenerationRole, ProjectionPublishReceipt, ProjectionSnapshot, ProjectionStoreBackend,
    ProjectionStoreDescriptor, task_search_documents_for_task_ids,
};

pub(crate) const TANTIVY_PROJECTION_PROVIDER: &str = "tantivy";
pub(crate) const TANTIVY_PROJECTION_PROVIDER_FINGERPRINT: &str = "tantivy-tasks-v2";
const TANTIVY_PROJECTION_HELPER_LOCK: &str = "tantivy_tasks-projection-helper";
const GENERATIONS_DIR: &str = "generations";
const PUBLISHED_MARKER: &str = "published";

#[derive(Debug, Clone)]
pub(crate) struct TantivyProjectionStore {
    db_path: PathBuf,
    database_instance_id: String,
    root: PathBuf,
}

impl TantivyProjectionStore {
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
            TANTIVY_TASKS_STORE,
        )
        .map_err(|error| KanbanError::Storage(error.to_string()))?;
        let root = generations.parent().ok_or_else(|| {
            KanbanError::Storage("Tantivy generations path has no store parent".to_owned())
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
                TANTIVY_TASKS_STORE,
            )
        } else {
            kanban_local::checked_projection_store_generations_path(
                &self.db_path,
                &self.database_instance_id,
                TANTIVY_TASKS_STORE,
            )
        }
        .map_err(|error| KanbanError::Storage(error.to_string()))?;
        if actual != self.generations_root() {
            return Err(KanbanError::Storage(
                "Tantivy database namespace changed after backend construction".to_owned(),
            ));
        }
        Ok(())
    }

    pub(crate) fn search_active(
        &self,
        expected: &ProjectionArtifactEvidence,
        query: &kanban_search::SearchQuery,
    ) -> Result<(Vec<kanban_search::SearchHit>, kanban_search::SearchMeta)> {
        let _authority_guard = crate::db::acquire_derived_store_read_guard(
            &self.db_path,
            TANTIVY_PROJECTION_HELPER_LOCK,
        )?;
        self.validate_managed_ancestors(false)?;
        if expected.manifest.database_instance_id != self.database_instance_id {
            return Err(KanbanError::Conflict(
                "Tantivy active evidence belongs to another database".to_owned(),
            ));
        }
        let metadata = metadata_from_evidence(expected)?;
        let generation_path = self.checked_generation_path(&expected.manifest.generation)?;
        search_task_projection_generation_against(&generation_path, &metadata, query)
            .map_err(search_storage)
    }

    fn generations_root(&self) -> PathBuf {
        self.root.join(GENERATIONS_DIR)
    }

    fn generation_path(&self, generation: &str) -> PathBuf {
        self.generations_root().join(generation)
    }

    fn checked_generation_path(&self, generation: &str) -> Result<PathBuf> {
        kanban_local::projection_generation_path(&self.generations_root(), generation)
            .map_err(|error| KanbanError::Storage(error.to_string()))
    }

    fn published_marker(&self, generation: &str) -> PathBuf {
        self.generation_path(generation).join(PUBLISHED_MARKER)
    }

    fn inspect_published(&self) -> Result<Vec<ProjectionArtifactEvidence>> {
        let _authority_guard = crate::db::acquire_derived_store_read_guard(
            &self.db_path,
            TANTIVY_PROJECTION_HELPER_LOCK,
        )?;
        self.inspect_published_while_helper_locked()
    }

    fn inspect_published_while_helper_locked(&self) -> Result<Vec<ProjectionArtifactEvidence>> {
        self.validate_managed_ancestors(false)?;
        let generations_root = self.generations_root();
        let root_metadata = match fs::symlink_metadata(&generations_root) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(KanbanError::Storage(error.to_string())),
        };
        if !root_metadata.is_dir() {
            return Err(KanbanError::Storage(format!(
                "Tantivy generations root is not a directory: {}",
                generations_root.display()
            )));
        }
        let mut published = Vec::new();
        for entry in fs::read_dir(&generations_root)
            .map_err(|error| KanbanError::Storage(error.to_string()))?
        {
            let entry = entry.map_err(|error| KanbanError::Storage(error.to_string()))?;
            let file_type = entry
                .file_type()
                .map_err(|error| KanbanError::Storage(error.to_string()))?;
            if !file_type.is_dir() {
                continue;
            }
            let marker = entry.path().join(PUBLISHED_MARKER);
            match fs::symlink_metadata(&marker) {
                Ok(metadata) if metadata.is_file() => {}
                Ok(_) => continue,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(KanbanError::Storage(error.to_string())),
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

    fn prepare_snapshot_while_helper_locked(
        &self,
        snapshot: &ProjectionSnapshot,
    ) -> Result<ProjectionArtifactEvidence> {
        self.validate_managed_ancestors(true)?;
        if snapshot.manifest.store_name != TANTIVY_TASKS_STORE
            || snapshot.manifest.database_instance_id != self.database_instance_id
            || snapshot.manifest.corpus.is_some()
        {
            return Err(KanbanError::Conflict(
                "Tantivy projection received a different store or database manifest".to_owned(),
            ));
        }
        let documents = snapshot
            .records
            .iter()
            .map(|record| {
                let document: TaskSearchDocument = serde_json::from_str(&record.payload_json)
                    .map_err(|error| KanbanError::Storage(error.to_string()))?;
                if document.board_id != record.board_id
                    || record.identity != format!("kb://task/{}", document.task_id)
                {
                    return Err(KanbanError::Conflict(
                        "Tantivy projection snapshot record identity mismatch".to_owned(),
                    ));
                }
                Ok(document)
            })
            .collect::<Result<Vec<_>>>()?;
        let fingerprint = snapshot_fingerprint(snapshot);
        let mut manifest = snapshot.manifest.clone();
        manifest.fingerprint = Some(fingerprint.clone());
        let evidence = ProjectionArtifactEvidence {
            manifest,
            fingerprint,
        };
        let metadata = metadata_from_evidence(&evidence)?;
        let generation = &evidence.manifest.generation;
        let generation_path = self.checked_generation_path(generation)?;
        match fs::symlink_metadata(&generation_path) {
            Ok(metadata) if !metadata.is_dir() => {
                return Err(KanbanError::Conflict(format!(
                    "Tantivy generation {generation} has a non-directory entry; fenced recovery is required before prepare"
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(KanbanError::Storage(error.to_string())),
        }
        // A failed prepare may leave a partial generation behind. It is
        // deliberately left in place as recovery evidence: this method has no
        // opaque owner/token capability with which to authorize a destructive
        // quarantine. The service retries through the fenced abort/quarantine
        // API before starting a replacement generation.
        prepare_task_projection_generation(&generation_path, &metadata, &documents).map_err(
            |error| {
                KanbanError::Conflict(format!(
                    "Tantivy generation {generation} is not safely reusable; fenced recovery is required: {error}"
                ))
            },
        )?;
        Ok(evidence)
    }

    fn apply_batch_while_helper_locked(
        &self,
        batch: &ProjectionBatch,
    ) -> Result<ProjectionBatchReceipt> {
        self.validate_managed_ancestors(false)?;
        if batch.database_instance_id != self.database_instance_id {
            return Err(KanbanError::Conflict(
                "Tantivy batch belongs to another database".to_owned(),
            ));
        }
        if batch.corpus.is_some() {
            return Err(KanbanError::Conflict(
                "Tantivy batch has an unexpected corpus binding".to_owned(),
            ));
        }
        let generation_path = self.checked_generation_path(&batch.target_generation)?;
        let evidence = self
            .inspect_generation_while_helper_locked(&batch.target_generation)?
            .ok_or_else(|| {
                KanbanError::Conflict(format!(
                    "Tantivy target generation {} does not exist",
                    batch.target_generation
                ))
            })?;
        let metadata = metadata_from_evidence(&evidence)?;
        let conn = crate::db::connect_file(&self.db_path)?;
        let keys = affected_task_keys(&conn, batch)?;
        let mut documents = Vec::new();
        let mut deletes = Vec::new();
        for (board_id, task_ids) in keys {
            let task_ids = task_ids.into_iter().collect::<Vec<_>>();
            let current = task_search_documents_for_task_ids(&conn, &board_id, &task_ids)?;
            let current_ids = current
                .iter()
                .map(|document| document.task_id.as_str())
                .collect::<BTreeSet<_>>();
            deletes.extend(
                task_ids
                    .iter()
                    .filter(|task_id| !current_ids.contains(task_id.as_str()))
                    .map(|task_id| TaskProjectionDocumentKey {
                        board_id: board_id.clone(),
                        task_id: task_id.clone(),
                    }),
            );
            documents.extend(current);
        }
        sync_task_projection_generation(&generation_path, &metadata, &documents, &deletes)
            .map_err(search_storage)?;
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
                "Tantivy publish evidence belongs to another database".to_owned(),
            ));
        }
        if self.inspect_published_while_helper_locked()?.last() != expected_active {
            return Err(KanbanError::Conflict(
                "Tantivy active generation changed before publish".to_owned(),
            ));
        }
        let stored = self
            .inspect_generation_while_helper_locked(&prepared.manifest.generation)?
            .ok_or_else(|| {
                KanbanError::Conflict("prepared Tantivy generation is missing".to_owned())
            })?;
        if stored != *prepared {
            return Err(KanbanError::Conflict(
                "prepared Tantivy generation readback mismatch".to_owned(),
            ));
        }
        let generation_path = self.checked_generation_path(&prepared.manifest.generation)?;
        kanban_search::tantivy_backend::sync_task_projection_generation_files(&generation_path)
            .map_err(search_storage)?;
        self.repair_generation_publication_while_helper_locked(prepared)?;
        let active = self
            .inspect_published_while_helper_locked()?
            .pop()
            .ok_or_else(|| {
                KanbanError::Storage("published Tantivy generation is not discoverable".to_owned())
            })?;
        if active != *prepared {
            return Err(KanbanError::Conflict(
                "a newer Tantivy generation won the publish fence".to_owned(),
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
            Err(error) => return Err(KanbanError::Storage(error.to_string())),
        };
        if !metadata.is_dir() {
            return Err(KanbanError::Storage(format!(
                "Tantivy generation path is not a directory: {}",
                path.display()
            )));
        }
        let metadata = read_generation_metadata(&path)?;
        validate_task_projection_generation(
            &path,
            &metadata.database_instance_id,
            &metadata.generation,
        )
        .map_err(search_storage)?;
        let evidence = evidence_from_metadata(metadata);
        if evidence.manifest.database_instance_id != self.database_instance_id {
            return Err(KanbanError::Conflict(
                "Tantivy generation belongs to another database".to_owned(),
            ));
        }
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
                KanbanError::Storage(format!("Tantivy generation {generation} is missing"))
            })?;
        if stored != *expected {
            return Err(KanbanError::Storage(format!(
                "Tantivy generation {generation} evidence mismatch"
            )));
        }
        let marker = self.published_marker(generation);
        let metadata = fs::symlink_metadata(&marker)
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
        if !metadata.is_file() {
            return Err(KanbanError::Storage(format!(
                "Tantivy published marker is not a regular file: {}",
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
                    "Tantivy generation {generation} is missing during marker repair"
                ))
            })?;
        if stored != *expected {
            return Err(KanbanError::Conflict(format!(
                "Tantivy generation {generation} evidence mismatch during marker repair"
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
                durable_quarantine_entry(&marker)
                    .map_err(|error| KanbanError::Storage(error.to_string()))?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(KanbanError::Storage(error.to_string())),
        }
        durable_create_new_file(&marker, &published_marker_contents(expected))
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
        validate_published_marker(&marker, expected)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TantivySqliteGenerationBinding {
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

impl TantivySqliteGenerationBinding {
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
                "Tantivy projection store {store_name} has no generation binding"
            ))
        })?;
        let fence_epoch = self.fence_epoch.ok_or_else(|| {
            KanbanError::Conflict(format!(
                "Tantivy projection store {store_name} has no generation fence"
            ))
        })?;
        let provider = self.provider.clone().ok_or_else(|| {
            KanbanError::Conflict(format!(
                "Tantivy projection store {store_name} has no generation provider"
            ))
        })?;
        let provider_fingerprint = self.provider_fingerprint.clone().ok_or_else(|| {
            KanbanError::Conflict(format!(
                "Tantivy projection store {store_name} has no provider fingerprint"
            ))
        })?;
        let canonical_item_count = self.canonical_item_count.ok_or_else(|| {
            KanbanError::Conflict(format!(
                "Tantivy projection store {store_name} has no canonical count"
            ))
        })?;
        let canonical_digest = self.canonical_digest.clone().ok_or_else(|| {
            KanbanError::Conflict(format!(
                "Tantivy projection store {store_name} has no canonical digest"
            ))
        })?;
        let delivery_item_count = self.delivery_item_count.ok_or_else(|| {
            KanbanError::Conflict(format!(
                "Tantivy projection store {store_name} has no delivery count"
            ))
        })?;
        let delivery_digest = self.delivery_digest.clone().ok_or_else(|| {
            KanbanError::Conflict(format!(
                "Tantivy projection store {store_name} has no delivery digest"
            ))
        })?;
        let corpus = super::projection_v2::projection_corpus_from_values(
            self.corpus_schema.clone(),
            self.corpus_fingerprint.clone(),
            self.embedding_model.clone(),
            self.embedding_dimensions,
            store_name,
            "Tantivy destructive authority",
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
struct TantivySqliteAuthorityState {
    database_instance_id: String,
    protocol_version: i64,
    schema_version: i64,
    control_plane: String,
    fence_epoch: i64,
    lease_owner: Option<String>,
    lease_token: Option<String>,
    lease_expires_at: Option<i64>,
    active: TantivySqliteGenerationBinding,
    previous: TantivySqliteGenerationBinding,
    building: TantivySqliteGenerationBinding,
    snapshot_cursor: i64,
    building_phase: Option<String>,
}

impl TantivySqliteAuthorityState {
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
            [TANTIVY_TASKS_STORE],
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
                    active: TantivySqliteGenerationBinding::from_row(row, 8)?,
                    previous: TantivySqliteGenerationBinding::from_row(row, 22)?,
                    building: TantivySqliteGenerationBinding::from_row(row, 36)?,
                    snapshot_cursor: row.get(50)?,
                    building_phase: row.get(51)?,
                })
            },
        )
        .optional()
        .map_err(|error| KanbanError::Storage(error.to_string()))?
        .ok_or_else(|| {
            KanbanError::Conflict("Tantivy projection store has no SQLite authority row".to_owned())
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
                "Tantivy projection orphaned generations have no SQLite authority".to_owned(),
            )),
        }
    }
}

#[derive(Debug, Clone)]
struct TantivyDestructiveValidation {
    role: ProjectionGenerationRole,
    current_provider_binding: bool,
}

fn tantivy_authority_error(message: impl Into<String>) -> KanbanError {
    KanbanError::Conflict(format!(
        "Tantivy projection destructive authority is stale or inconsistent: {}",
        message.into()
    ))
}

impl TantivyProjectionStore {
    /// Validate the opaque capability and every SQLite generation binding before
    /// touching a physical generation. The caller may hold the generic store
    /// guard; this backend also holds its distinct helper authority guard.
    fn validate_destructive_authority(
        &self,
        generation: &str,
        authority: &ProjectionDestructiveAuthority,
    ) -> Result<TantivyDestructiveValidation> {
        let validation = self.validate_exact_destructive_authority(generation, authority)?;
        if !validation.current_provider_binding {
            return Err(tantivy_authority_error(
                "provider or corpus binding does not match Tantivy",
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
    ) -> Result<TantivyDestructiveValidation> {
        self.validate_exact_destructive_authority(generation, authority)
    }

    fn validate_exact_destructive_authority(
        &self,
        generation: &str,
        authority: &ProjectionDestructiveAuthority,
    ) -> Result<TantivyDestructiveValidation> {
        let now = SystemClock.now_ms();
        if generation.trim().is_empty()
            || authority.generation != generation
            || authority.owner.trim().is_empty()
            || authority.lease_token.trim().is_empty()
            || authority.fence_epoch < 0
            || authority.lease_expires_at <= now
        {
            return Err(tantivy_authority_error(
                "capability is incomplete or expired",
            ));
        }
        let conn = crate::db::connect_file(&self.db_path)?;
        let state = TantivySqliteAuthorityState::load(&conn)?;
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
            return Err(tantivy_authority_error(
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
                    return Err(tantivy_authority_error(
                        "generation is bound to more than one SQLite role",
                    ));
                }
                matched_role = Some(role);
            }
        }
        let role = matched_role.ok_or_else(|| {
            tantivy_authority_error(
                "generation is not bound to an active, previous, or building role",
            )
        })?;
        if role != authority.role || authority.role == ProjectionGenerationRole::Orphaned {
            return Err(tantivy_authority_error(
                "generation role does not match SQLite",
            ));
        }
        let binding = state.binding_for(role, TANTIVY_TASKS_STORE)?;
        if binding.generation != generation || binding != authority.expected_binding {
            return Err(tantivy_authority_error(
                "generation binding does not match SQLite",
            ));
        }
        let phase = if role == ProjectionGenerationRole::Building {
            let phase = state.building_phase.as_deref();
            if !matches!(phase, Some("snapshotting" | "prepared" | "store_published")) {
                return Err(tantivy_authority_error("building phase is invalid"));
            }
            phase.map(str::to_owned)
        } else {
            None
        };
        if authority.building_phase != phase {
            return Err(tantivy_authority_error(
                "building phase does not match SQLite",
            ));
        }
        let expected_manifest = if binding.fingerprint.is_some() {
            Some(ProjectionArtifactManifest {
                store_name: TANTIVY_TASKS_STORE.to_owned(),
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
            return Err(tantivy_authority_error(
                "manifest does not match SQLite binding",
            ));
        }
        let current_provider_binding = binding.provider == TANTIVY_PROJECTION_PROVIDER
            && binding.provider_fingerprint == TANTIVY_PROJECTION_PROVIDER_FINGERPRINT
            && binding.corpus.is_none();
        Ok(TantivyDestructiveValidation {
            role,
            current_provider_binding,
        })
    }
}

impl ProjectionStoreBackend for TantivyProjectionStore {
    fn descriptor(&self) -> Result<ProjectionStoreDescriptor> {
        Ok(ProjectionStoreDescriptor {
            store_name: TANTIVY_TASKS_STORE.to_owned(),
            provider: TANTIVY_PROJECTION_PROVIDER.to_owned(),
            provider_fingerprint: TANTIVY_PROJECTION_PROVIDER_FINGERPRINT.to_owned(),
            corpus: None,
        })
    }

    fn prepare_snapshot(
        &self,
        snapshot: &ProjectionSnapshot,
    ) -> Result<ProjectionArtifactEvidence> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            TANTIVY_PROJECTION_HELPER_LOCK,
        )?;
        self.prepare_snapshot_while_helper_locked(snapshot)
    }

    fn prepare_snapshot_with_authority(
        &self,
        snapshot: &ProjectionSnapshot,
        authority: &ProjectionDestructiveAuthority,
    ) -> Result<ProjectionArtifactEvidence> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            TANTIVY_PROJECTION_HELPER_LOCK,
        )?;
        self.validate_destructive_authority(&snapshot.manifest.generation, authority)?;
        self.prepare_snapshot_while_helper_locked(snapshot)
    }

    fn apply_batch(&self, batch: &ProjectionBatch) -> Result<ProjectionBatchReceipt> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            TANTIVY_PROJECTION_HELPER_LOCK,
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
            TANTIVY_PROJECTION_HELPER_LOCK,
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
            TANTIVY_PROJECTION_HELPER_LOCK,
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
            TANTIVY_PROJECTION_HELPER_LOCK,
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
            TANTIVY_PROJECTION_HELPER_LOCK,
        )?;
        self.inspect_generation_while_helper_locked(generation)
    }

    fn validate_generation_publication(&self, expected: &ProjectionArtifactEvidence) -> Result<()> {
        let generation = &expected.manifest.generation;
        let _authority_guard = crate::db::acquire_derived_store_read_guard(
            &self.db_path,
            TANTIVY_PROJECTION_HELPER_LOCK,
        )?;
        let stored = self
            .inspect_generation_while_helper_locked(generation)?
            .ok_or_else(|| {
                KanbanError::Storage(format!("Tantivy generation {generation} is missing"))
            })?;
        if stored != *expected {
            return Err(KanbanError::Storage(format!(
                "Tantivy generation {generation} evidence mismatch"
            )));
        }
        let marker = self.published_marker(generation);
        let metadata = fs::symlink_metadata(&marker)
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
        if !metadata.is_file() {
            return Err(KanbanError::Storage(format!(
                "Tantivy published marker is not a regular file: {}",
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
            TANTIVY_PROJECTION_HELPER_LOCK,
        )?;
        self.validate_destructive_authority(&expected.manifest.generation, authority)?;
        self.validate_generation_publication_while_helper_locked(expected)
    }

    fn repair_generation_publication(&self, expected: &ProjectionArtifactEvidence) -> Result<()> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            TANTIVY_PROJECTION_HELPER_LOCK,
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
            TANTIVY_PROJECTION_HELPER_LOCK,
        )?;
        self.validate_destructive_authority(&expected.manifest.generation, authority)?;
        self.repair_generation_publication_while_helper_locked(expected)
    }

    fn quarantine_generation(&self, generation: &str) -> Result<()> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            TANTIVY_PROJECTION_HELPER_LOCK,
        )?;
        self.validate_managed_ancestors(false)?;
        let generation_path = self.checked_generation_path(generation)?;
        match fs::symlink_metadata(&generation_path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(KanbanError::Storage(error.to_string())),
        }
        durable_quarantine_entry(&generation_path)
            .map(|_| ())
            .map_err(|error| KanbanError::Storage(error.to_string()))
    }

    fn abort_generation(&self, generation: &str) -> Result<()> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            TANTIVY_PROJECTION_HELPER_LOCK,
        )?;
        self.validate_managed_ancestors(false)?;
        let path = self.checked_generation_path(generation)?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(KanbanError::Storage(error.to_string())),
        };
        if !metadata.is_dir() {
            durable_quarantine_entry(&path)
                .map_err(|error| KanbanError::Storage(error.to_string()))?;
            return Ok(());
        }
        match fs::symlink_metadata(self.published_marker(generation)) {
            Ok(_) => {
                return Err(KanbanError::Conflict(format!(
                    "cannot abort published Tantivy generation {generation}"
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(KanbanError::Storage(error.to_string())),
        }
        durable_remove_directory(&path).map_err(|error| KanbanError::Storage(error.to_string()))?;
        Ok(())
    }

    fn quarantine_generation_fenced(
        &self,
        generation: &str,
        authority: &ProjectionDestructiveAuthority,
    ) -> Result<()> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            TANTIVY_PROJECTION_HELPER_LOCK,
        )?;
        self.validate_managed_ancestors(false)?;
        let path = self.checked_generation_path(generation)?;
        let validation = self.validate_recovery_destructive_authority(generation, authority)?;

        // An active generation that still reads back as the exact canonical
        // artifact is protected. A corrupt/mismatched artifact is precisely
        // what this recovery operation is allowed to move aside.
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
                "cannot quarantine canonical active Tantivy generation {generation}"
            )));
        }

        match fs::symlink_metadata(&path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(KanbanError::Storage(error.to_string())),
        }
        // `_authority_guard` is the backend-specific physical write fence;
        // keep the authority check immediately adjacent to this durable move
        // so no unfenced legacy path can be substituted.
        durable_quarantine_entry(&path)
            .map(|_| ())
            .map_err(|error| KanbanError::Storage(error.to_string()))
    }

    fn abort_generation_fenced(
        &self,
        generation: &str,
        authority: &ProjectionDestructiveAuthority,
    ) -> Result<()> {
        let _authority_guard = crate::db::acquire_derived_store_write_guard(
            &self.db_path,
            TANTIVY_PROJECTION_HELPER_LOCK,
        )?;
        self.validate_managed_ancestors(false)?;
        let path = self.checked_generation_path(generation)?;
        let validation = self.validate_recovery_destructive_authority(generation, authority)?;
        if validation.role == ProjectionGenerationRole::Active {
            return Err(KanbanError::Conflict(format!(
                "cannot abort canonical active Tantivy generation {generation}"
            )));
        }
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(KanbanError::Storage(error.to_string())),
        };
        match fs::symlink_metadata(self.published_marker(generation)) {
            Ok(_) => {
                return Err(KanbanError::Conflict(format!(
                    "cannot abort published Tantivy generation {generation}"
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(KanbanError::Storage(error.to_string())),
        }
        // See the fenced quarantine path above: this delete is made while the
        // backend-specific authority guard remains held.
        if !metadata.is_dir() {
            durable_quarantine_entry(&path)
                .map_err(|error| KanbanError::Storage(error.to_string()))?;
            return Ok(());
        }
        durable_remove_directory(&path).map_err(|error| KanbanError::Storage(error.to_string()))?;
        Ok(())
    }
}

fn affected_task_keys(
    conn: &Connection,
    batch: &ProjectionBatch,
) -> Result<BTreeMap<String, BTreeSet<String>>> {
    let mut keys: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for item in &batch.items {
        let task_id = if let Some(task_id) = item
            .entity_uri
            .strip_prefix("kb://task/")
            .filter(|task_id| !task_id.is_empty())
        {
            Some(task_id.to_owned())
        } else if let Some(source_event_id) = item.source_event_id {
            let event_task = conn
                .query_row(
                    "SELECT COALESCE(e.task_id,r.task_id)
                 FROM task_events e
                 LEFT JOIN task_runs r ON r.board_id=e.board_id AND r.id=e.run_id
                 WHERE e.id=?1 AND e.board_id=?2",
                    params![source_event_id, item.board_id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()
                .map_err(super::storage)?;
            match event_task {
                Some(Some(task_id)) => Some(task_id),
                Some(None) if item.entity_uri == format!("kb://board/{}", item.board_id) => {
                    continue;
                }
                _ => None,
            }
        } else {
            None
        }
        .ok_or_else(|| {
            KanbanError::Conflict(format!(
                "Tantivy delivery {} cannot be mapped to a board-scoped task",
                item.id
            ))
        })?;
        keys.entry(item.board_id.clone())
            .or_default()
            .insert(task_id);
    }
    Ok(keys)
}

fn metadata_from_evidence(
    evidence: &ProjectionArtifactEvidence,
) -> Result<TantivyTaskProjectionMetadata> {
    if evidence.fingerprint.trim().is_empty() {
        return Err(KanbanError::Conflict(
            "Tantivy projection fingerprint cannot be empty".to_owned(),
        ));
    }
    if evidence.manifest.corpus.is_some() {
        return Err(KanbanError::Conflict(
            "Tantivy projection has an unexpected corpus binding".to_owned(),
        ));
    }
    Ok(TantivyTaskProjectionMetadata {
        database_instance_id: evidence.manifest.database_instance_id.clone(),
        protocol_version: evidence.manifest.protocol_version,
        schema_version: evidence.manifest.schema_version,
        generation: evidence.manifest.generation.clone(),
        fence_epoch: evidence.manifest.fence_epoch,
        snapshot_cursor: evidence.manifest.snapshot_cursor,
        provider: evidence.manifest.provider.clone(),
        provider_fingerprint: evidence.manifest.provider_fingerprint.clone(),
        canonical_item_count: evidence.manifest.canonical_item_count,
        canonical_digest: evidence.manifest.canonical_digest.clone(),
        delivery_item_count: evidence.manifest.delivery_item_count,
        delivery_digest: evidence.manifest.delivery_digest.clone(),
        fingerprint: evidence.fingerprint.clone(),
    })
}

fn evidence_from_metadata(metadata: TantivyTaskProjectionMetadata) -> ProjectionArtifactEvidence {
    ProjectionArtifactEvidence {
        fingerprint: metadata.fingerprint.clone(),
        manifest: ProjectionArtifactManifest {
            store_name: TANTIVY_TASKS_STORE.to_owned(),
            database_instance_id: metadata.database_instance_id,
            protocol_version: metadata.protocol_version,
            schema_version: metadata.schema_version,
            generation: metadata.generation,
            fence_epoch: metadata.fence_epoch,
            snapshot_cursor: metadata.snapshot_cursor,
            provider: metadata.provider,
            provider_fingerprint: metadata.provider_fingerprint,
            corpus: None,
            canonical_item_count: metadata.canonical_item_count,
            canonical_digest: metadata.canonical_digest,
            delivery_item_count: metadata.delivery_item_count,
            delivery_digest: metadata.delivery_digest,
            fingerprint: Some(metadata.fingerprint),
        },
    }
}

fn read_generation_metadata(path: &Path) -> Result<TantivyTaskProjectionMetadata> {
    let metadata_path = path.join("kb-projection-meta.json");
    let bytes = fs::read(&metadata_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            KanbanError::Storage(format!(
                "Tantivy physical metadata is missing: {}",
                metadata_path.display()
            ))
        } else {
            KanbanError::Storage(error.to_string())
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        KanbanError::Storage(format!(
            "Tantivy physical metadata is corrupt at {}: {error}",
            metadata_path.display()
        ))
    })
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
    let actual = fs::read(path).map_err(|error| KanbanError::Storage(error.to_string()))?;
    if actual != published_marker_contents(evidence) {
        return Err(KanbanError::Storage(format!(
            "Tantivy published marker does not match generation evidence: {}",
            path.display()
        )));
    }
    Ok(())
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

fn search_storage(error: impl std::error::Error) -> KanbanError {
    KanbanError::Storage(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::{ProjectionSnapshotRecord, ProjectionStoreBackend};
    use kanban_core::TaskStatus;
    use rusqlite::params;

    fn store(temp: &tempfile::TempDir) -> TantivyProjectionStore {
        let db_path = temp.path().join("kanban.db");
        crate::init::init_database(&db_path, "tantivy-projection-test").unwrap();
        let store = TantivyProjectionStore::new_bound(db_path, "db_test".to_owned()).unwrap();
        drop(
            crate::db::acquire_derived_store_write_guard(
                &store.db_path,
                TANTIVY_PROJECTION_HELPER_LOCK,
            )
            .unwrap(),
        );
        store
    }

    fn snapshot(generation: &str) -> ProjectionSnapshot {
        let document = TaskSearchDocument {
            board_id: "b_test".to_owned(),
            task_id: "t_test".to_owned(),
            seq: 1,
            status: TaskStatus::Ready,
            assignee: None,
            priority: 1,
            created_at: 10,
            updated_at: 11,
            due_at: None,
            title: "durable Tantivy generation".to_owned(),
            description: None,
            comments: String::new(),
            run_text: String::new(),
            event_text: String::new(),
        };
        ProjectionSnapshot {
            manifest: ProjectionArtifactManifest {
                store_name: TANTIVY_TASKS_STORE.to_owned(),
                database_instance_id: "db_test".to_owned(),
                protocol_version: 2,
                schema_version: 1,
                generation: generation.to_owned(),
                fence_epoch: 7,
                snapshot_cursor: 11,
                provider: TANTIVY_PROJECTION_PROVIDER.to_owned(),
                provider_fingerprint: TANTIVY_PROJECTION_PROVIDER_FINGERPRINT.to_owned(),
                corpus: None,
                canonical_item_count: 1,
                canonical_digest: "fnv64:canonical".to_owned(),
                delivery_item_count: 1,
                delivery_digest: "fnv64:delivery".to_owned(),
                fingerprint: None,
            },
            records: vec![ProjectionSnapshotRecord {
                board_id: document.board_id.clone(),
                identity: format!("kb://task/{}", document.task_id),
                payload_json: serde_json::to_string(&document).unwrap(),
                content_hash: "fnv64:record".to_owned(),
            }],
        }
    }

    fn fenced_fixture(
        temp: &tempfile::TempDir,
        generation: &str,
    ) -> (
        TantivyProjectionStore,
        ProjectionArtifactEvidence,
        ProjectionDestructiveAuthority,
    ) {
        let db_path = temp.path().join("kanban.db");
        crate::init::init_database(&db_path, "tantivy-fenced-test").unwrap();
        let conn = crate::db::connect_file(&db_path).unwrap();
        let database_instance_id: String = conn
            .query_row(
                "SELECT database_instance_id FROM projection_database WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let store =
            TantivyProjectionStore::new_bound(db_path, database_instance_id.clone()).unwrap();
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
                TANTIVY_TASKS_STORE,
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
        store: &TantivyProjectionStore,
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
                TANTIVY_TASKS_STORE,
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
                    [TANTIVY_TASKS_STORE],
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
            corpus_schema: "historical-tantivy-corpus-v1".to_owned(),
            corpus_fingerprint: "historical-tantivy-corpus-fingerprint".to_owned(),
            embedding_model: "historical-tantivy-embedding".to_owned(),
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
            historical_fenced_authority(&store, authority, "tantivy-provider-historical-v0", None);
        let generation = evidence.manifest.generation.as_str();
        let mut mismatched = authority.clone();
        mismatched.expected_binding.provider_fingerprint = "tantivy-provider-mismatched".to_owned();

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
            TANTIVY_PROJECTION_PROVIDER_FINGERPRINT,
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
        assert!(
            fs::read_dir(store.generation_path("gen_active"))
                .unwrap()
                .flatten()
                .any(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .contains("published.quarantine"))
        );

        fs::remove_file(store.published_marker("gen_active")).unwrap();
        fs::create_dir(store.published_marker("gen_active")).unwrap();
        assert_eq!(store.inspect_active().unwrap(), None);
        store.repair_generation_publication(&evidence).unwrap();
        assert_eq!(store.inspect_active().unwrap(), Some(evidence));
        assert!(store.published_marker("gen_active").is_file());
    }

    #[test]
    fn unpublished_generation_can_be_aborted_but_published_generation_cannot() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(&temp);

        store
            .prepare_snapshot(&snapshot("gen_unpublished"))
            .unwrap();
        store.abort_generation("gen_unpublished").unwrap();
        assert!(
            store
                .inspect_generation("gen_unpublished")
                .unwrap()
                .is_none()
        );

        let evidence = store.prepare_snapshot(&snapshot("gen_published")).unwrap();
        store.publish_generation(None, &evidence).unwrap();
        let error = store.abort_generation("gen_published").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("cannot abort published Tantivy generation"),
            "{error}"
        );
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
    fn inspect_generation_fails_closed_for_corrupt_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(&temp);
        store.prepare_snapshot(&snapshot("gen_corrupt")).unwrap();
        fs::write(
            store
                .generation_path("gen_corrupt")
                .join("kb-projection-meta.json"),
            b"{not-json",
        )
        .unwrap();

        let error = store.inspect_generation("gen_corrupt").unwrap_err();
        assert!(
            error.to_string().contains("physical metadata is corrupt"),
            "{error}"
        );
    }

    #[test]
    fn prepare_leaves_a_corrupt_unpublished_generation_for_fenced_recovery() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(&temp);
        let snapshot = snapshot("gen_retry");
        store.prepare_snapshot(&snapshot).unwrap();
        fs::write(
            store
                .generation_path("gen_retry")
                .join("kb-projection-meta.json"),
            b"{not-json",
        )
        .unwrap();

        let error = store.prepare_snapshot(&snapshot).unwrap_err();
        assert!(
            error.to_string().contains("fenced recovery is required"),
            "{error}"
        );
        assert_eq!(
            fs::read(
                store
                    .generation_path("gen_retry")
                    .join("kb-projection-meta.json")
            )
            .unwrap(),
            b"{not-json"
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
        crate::init::init_database(&db_a_path, "tantivy-isolation-a").unwrap();
        crate::init::init_database(&db_b_path, "tantivy-isolation-b").unwrap();
        let db_a = TantivyProjectionStore::new_bound(db_a_path, "db_a".to_owned()).unwrap();
        let db_b = TantivyProjectionStore::new_bound(db_b_path, "db_b".to_owned()).unwrap();
        assert_ne!(db_a.root, db_b.root);
        drop(
            crate::db::acquire_derived_store_write_guard(
                &db_b.db_path,
                TANTIVY_PROJECTION_HELPER_LOCK,
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
            .join(TANTIVY_TASKS_STORE);
        fs::create_dir_all(&legacy_root).unwrap();
        let sentinel = legacy_root.join("legacy-sentinel");
        fs::write(&sentinel, b"unscoped-v2-evidence").unwrap();
        let mut snapshot_b = snapshot("gen_shared");
        snapshot_b.manifest.database_instance_id = "db_b".to_owned();
        db_b.prepare_snapshot(&snapshot_b).unwrap();
        assert_eq!(fs::read(&sentinel).unwrap(), b"unscoped-v2-evidence");
    }

    #[cfg(unix)]
    #[test]
    fn database_file_symlink_alias_reads_the_same_active_generation() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let real_parent = temp.path().join("real");
        let alias_parent = temp.path().join("alias");
        fs::create_dir_all(&real_parent).unwrap();
        fs::create_dir_all(&alias_parent).unwrap();
        let real_db = real_parent.join("kanban.db");
        let alias_db = alias_parent.join("kanban.db");
        crate::init::init_database(&real_db, "tantivy-alias-test").unwrap();
        symlink(&real_db, &alias_db).unwrap();
        let real = TantivyProjectionStore::new_bound(real_db, "db_test".to_owned()).unwrap();
        let alias = TantivyProjectionStore::new_bound(alias_db, "db_test".to_owned()).unwrap();
        assert_eq!(real.root, alias.root);

        let evidence = real.prepare_snapshot(&snapshot("gen_active")).unwrap();
        real.publish_generation(None, &evidence).unwrap();

        assert_eq!(alias.inspect_active().unwrap(), Some(evidence));
    }
}
