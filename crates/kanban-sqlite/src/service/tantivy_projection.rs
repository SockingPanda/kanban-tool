use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use kanban_core::{KanbanError, Result};
use kanban_indexer::TANTIVY_TASKS_STORE;
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
    root: PathBuf,
}

impl TantivyProjectionStore {
    pub(crate) fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        let root = kanban_local::projection_store_root_path(&db_path, TANTIVY_TASKS_STORE)
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
        Ok(Self { db_path, root })
    }

    pub(crate) fn search_active(
        &self,
        expected: &ProjectionArtifactEvidence,
        query: &kanban_search::SearchQuery,
    ) -> Result<(Vec<kanban_search::SearchHit>, kanban_search::SearchMeta)> {
        let metadata = metadata_from_evidence(expected)?;
        search_task_projection_generation_against(
            &self.generation_path(&expected.manifest.generation),
            &metadata,
            query,
        )
        .map_err(search_storage)
    }

    fn generations_root(&self) -> PathBuf {
        self.root.join(GENERATIONS_DIR)
    }

    fn generation_path(&self, generation: &str) -> PathBuf {
        self.generations_root().join(generation)
    }

    fn published_marker(&self, generation: &str) -> PathBuf {
        self.generation_path(generation).join(PUBLISHED_MARKER)
    }

    fn inspect_published(&self) -> Result<Vec<ProjectionArtifactEvidence>> {
        let generations_root = self.generations_root();
        if !generations_root.exists() {
            return Ok(Vec::new());
        }
        let mut published = Vec::new();
        for entry in fs::read_dir(&generations_root)
            .map_err(|error| KanbanError::Storage(error.to_string()))?
        {
            let entry = entry.map_err(|error| KanbanError::Storage(error.to_string()))?;
            let file_type = entry
                .file_type()
                .map_err(|error| KanbanError::Storage(error.to_string()))?;
            if !file_type.is_dir() || !entry.path().join(PUBLISHED_MARKER).is_file() {
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
        if snapshot.manifest.store_name != TANTIVY_TASKS_STORE {
            return Err(KanbanError::Conflict(
                "Tantivy projection received a different store manifest".to_owned(),
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
        prepare_task_projection_generation(
            &self.generation_path(&evidence.manifest.generation),
            &metadata,
            &documents,
        )
        .map_err(search_storage)?;
        Ok(evidence)
    }

    fn apply_batch(&self, batch: &ProjectionBatch) -> Result<ProjectionBatchReceipt> {
        let generation_path = self.generation_path(&batch.target_generation);
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
        kanban_search::tantivy_backend::sync_task_projection_generation_files(
            &self.generation_path(&prepared.manifest.generation),
        )
        .map_err(search_storage)?;
        let marker = self.published_marker(&prepared.manifest.generation);
        if !marker.exists() {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&marker)
                .map_err(|error| KanbanError::Storage(error.to_string()))?;
            writeln!(
                file,
                "database_instance_id={}\ngeneration={}\nfence_epoch={}",
                prepared.manifest.database_instance_id,
                prepared.manifest.generation,
                prepared.manifest.fence_epoch
            )
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
            file.sync_all()
                .map_err(|error| KanbanError::Storage(error.to_string()))?;
            sync_directory(marker.parent().expect("published marker has parent"))?;
        }
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
        let path = self.generation_path(generation);
        if !path.exists() {
            return Ok(None);
        }
        let metadata = read_generation_metadata(&path)?;
        validate_task_projection_generation(
            &path,
            &metadata.database_instance_id,
            &metadata.generation,
        )
        .map_err(search_storage)?;
        Ok(Some(evidence_from_metadata(metadata)))
    }

    fn quarantine_generation(&self, generation: &str) -> Result<()> {
        let marker = self.published_marker(generation);
        if !marker.is_file() {
            return Err(KanbanError::Conflict(format!(
                "Tantivy generation {generation} is not published"
            )));
        }
        fs::remove_file(&marker).map_err(|error| KanbanError::Storage(error.to_string()))?;
        sync_directory(
            marker
                .parent()
                .expect("published marker always has a generation directory"),
        )?;
        sync_directory(&self.generations_root())?;
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

fn sync_directory(path: &Path) -> Result<()> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| KanbanError::Storage(error.to_string()))
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
    let bytes = fs::read(path.join("kb-projection-meta.json"))
        .map_err(|error| KanbanError::Storage(error.to_string()))?;
    serde_json::from_slice(&bytes).map_err(|error| KanbanError::Storage(error.to_string()))
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
