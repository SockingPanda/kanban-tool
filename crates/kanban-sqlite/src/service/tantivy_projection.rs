use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use kanban_core::{KanbanError, Result};
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
use rusqlite::{Connection, OptionalExtension, params};

use super::{
    ProjectionArtifactEvidence, ProjectionArtifactManifest, ProjectionBatch,
    ProjectionBatchReceipt, ProjectionPublishReceipt, ProjectionSnapshot, ProjectionStoreBackend,
    ProjectionStoreDescriptor, task_search_documents_for_task_ids,
};

pub(crate) const TANTIVY_PROJECTION_PROVIDER: &str = "tantivy";
pub(crate) const TANTIVY_PROJECTION_PROVIDER_FINGERPRINT: &str = "tantivy-tasks-v2";
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
}

impl ProjectionStoreBackend for TantivyProjectionStore {
    fn descriptor(&self) -> Result<ProjectionStoreDescriptor> {
        Ok(ProjectionStoreDescriptor {
            store_name: TANTIVY_TASKS_STORE.to_owned(),
            provider: TANTIVY_PROJECTION_PROVIDER.to_owned(),
            provider_fingerprint: TANTIVY_PROJECTION_PROVIDER_FINGERPRINT.to_owned(),
        })
    }

    fn prepare_snapshot(
        &self,
        snapshot: &ProjectionSnapshot,
    ) -> Result<ProjectionArtifactEvidence> {
        self.validate_managed_ancestors(true)?;
        if snapshot.manifest.store_name != TANTIVY_TASKS_STORE
            || snapshot.manifest.database_instance_id != self.database_instance_id
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
                durable_quarantine_entry(&generation_path)
                    .map_err(|error| KanbanError::Storage(error.to_string()))?;
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(KanbanError::Storage(error.to_string())),
        }
        if let Err(error) =
            prepare_task_projection_generation(&generation_path, &metadata, &documents)
        {
            let generation_metadata = match fs::symlink_metadata(&generation_path) {
                Ok(metadata) => Some(metadata),
                Err(io_error) if io_error.kind() == std::io::ErrorKind::NotFound => None,
                Err(io_error) => return Err(KanbanError::Storage(io_error.to_string())),
            };
            let marker_absent = match fs::symlink_metadata(self.published_marker(generation)) {
                Ok(_) => false,
                Err(marker_error) if marker_error.kind() == std::io::ErrorKind::NotFound => true,
                Err(marker_error) => {
                    return Err(KanbanError::Storage(marker_error.to_string()));
                }
            };
            let can_rebuild =
                generation_metadata.is_some_and(|metadata| metadata.is_dir()) && marker_absent;
            if !can_rebuild {
                return Err(search_storage(error));
            }
            self.abort_generation(generation)?;
            prepare_task_projection_generation(&generation_path, &metadata, &documents)
                .map_err(search_storage)?;
        }
        Ok(evidence)
    }

    fn apply_batch(&self, batch: &ProjectionBatch) -> Result<ProjectionBatchReceipt> {
        self.validate_managed_ancestors(false)?;
        if batch.database_instance_id != self.database_instance_id {
            return Err(KanbanError::Conflict(
                "Tantivy batch belongs to another database".to_owned(),
            ));
        }
        let generation_path = self.checked_generation_path(&batch.target_generation)?;
        let evidence = self
            .inspect_generation(&batch.target_generation)?
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
                "Tantivy publish evidence belongs to another database".to_owned(),
            ));
        }
        if self.inspect_active()?.as_ref() != expected_active {
            return Err(KanbanError::Conflict(
                "Tantivy active generation changed before publish".to_owned(),
            ));
        }
        let stored = self
            .inspect_generation(&prepared.manifest.generation)?
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
        self.repair_generation_publication(prepared)?;
        let active = self.inspect_active()?.ok_or_else(|| {
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

    fn inspect_active(&self) -> Result<Option<ProjectionArtifactEvidence>> {
        Ok(self.inspect_published()?.pop())
    }

    fn inspect_generation(&self, generation: &str) -> Result<Option<ProjectionArtifactEvidence>> {
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

    fn validate_generation_publication(&self, expected: &ProjectionArtifactEvidence) -> Result<()> {
        let generation = &expected.manifest.generation;
        let stored = self.inspect_generation(generation)?.ok_or_else(|| {
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

    fn repair_generation_publication(&self, expected: &ProjectionArtifactEvidence) -> Result<()> {
        let generation = &expected.manifest.generation;
        let stored = self.inspect_generation(generation)?.ok_or_else(|| {
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

    fn quarantine_generation(&self, generation: &str) -> Result<()> {
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

    fn store(temp: &tempfile::TempDir) -> TantivyProjectionStore {
        TantivyProjectionStore::new_bound(temp.path().join("kanban.db"), "db_test".to_owned())
            .unwrap()
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
    fn prepare_rebuilds_a_corrupt_unpublished_generation() {
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

        let rebuilt = store.prepare_snapshot(&snapshot).unwrap();

        assert_eq!(
            store.inspect_generation("gen_retry").unwrap(),
            Some(rebuilt)
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
        let db_a =
            TantivyProjectionStore::new_bound(temp.path().join("a.db"), "db_a".to_owned()).unwrap();
        let db_b =
            TantivyProjectionStore::new_bound(temp.path().join("b.db"), "db_b".to_owned()).unwrap();
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
        fs::write(&real_db, b"sqlite-placeholder").unwrap();
        symlink(&real_db, &alias_db).unwrap();
        let real = TantivyProjectionStore::new_bound(real_db, "db_test".to_owned()).unwrap();
        let alias = TantivyProjectionStore::new_bound(alias_db, "db_test".to_owned()).unwrap();
        assert_eq!(real.root, alias.root);

        let evidence = real.prepare_snapshot(&snapshot("gen_active")).unwrap();
        real.publish_generation(None, &evidence).unwrap();

        assert_eq!(alias.inspect_active().unwrap(), Some(evidence));
    }
}
