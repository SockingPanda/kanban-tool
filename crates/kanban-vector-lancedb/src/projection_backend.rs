use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use kanban_contract::{
    ProjectionArtifactEvidence, ProjectionArtifactManifest, ProjectionBatch,
    ProjectionCorpusMetadata, ProjectionDelivery, ProjectionDeliveryAction,
    ProjectionPublishReceipt, ProjectionSnapshot, ProjectionStoreDescriptor,
    VECTOR_PROJECTION_PROTOCOL_VERSION, VectorProjectionApplyBatchRequest,
    VectorProjectionApplyBatchResponse, VectorProjectionBatchApplicationReceipt,
    VectorProjectionCleanupProtection, VectorProjectionCleanupRequest,
    VectorProjectionCleanupResponse, VectorProjectionGenerationInventoryEntry,
    VectorProjectionGenerationMutationRequest, VectorProjectionGenerationState,
    VectorProjectionHelperDescriptor, VectorProjectionHelperError, VectorProjectionHelperErrorKind,
    VectorProjectionHelperOperation, VectorProjectionHelperRequest, VectorProjectionHelperResponse,
    VectorProjectionInspectActiveResponse, VectorProjectionInspectGenerationResponse,
    VectorProjectionInventoryResponse, VectorProjectionMutationAck,
    VectorProjectionMutationContext, VectorProjectionPrepareSnapshotRequest,
    VectorProjectionPrepareSnapshotResponse, VectorProjectionProtectedGeneration,
    VectorProjectionProtectionReason, VectorProjectionPublishRequest,
    VectorProjectionPublishResponse, VectorProjectionRepairPublicationRequest,
    VectorProjectionSkippedGeneration, VectorProjectionValidateActiveRequest,
    VectorProjectionValidateGenerationRequest, VectorProjectionValidationResponse,
};
use kanban_derived_io::{
    board_id, current_last_event_id, derived_status_by_name, has_pending_vector_outbox_for_board,
    label_atom_index_status_from_base,
};
use kanban_indexer::{
    DERIVED_STORE_SCHEMA_VERSION, LANCEDB_CHUNKS_STORE, LANCEDB_LABEL_ATOMS_STORE,
};
use kanban_local::{
    DerivedStoreReadGuard, DerivedStoreWriteGuard, DirectoryIdentityGuard,
    checked_projection_store_generations_path, durable_create_dir_all, durable_create_new_file,
    durable_quarantine_entry, durable_remove_directory, durable_replace_file_contents,
    durable_sync_directory_tree, ensure_projection_store_generations_path,
    projection_generation_path,
};
use kanban_sqlite::db::{DatabaseConnection, connect_existing_read_only};
use kanban_vector::{
    ChunkBuilder, ChunkVectorStore, EmbeddingProvider, LABEL_ATOMS_CORPUS_SCHEMA, LabelAtomHit,
    LabelAtomQuery, LabelAtomVector, LabelAtomVectorHit, LabelAtomVectorQuery,
    LabelAtomVectorStore, TASK_CHUNKS_CORPUS_SCHEMA, TaskChunkSource, VectorError, VectorHit,
    VectorQuery, VectorStoreStatus, corpus_provider_fingerprint, ensure_dimensions,
    normalize_semantic_text, semantic_content_hash,
};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::lancedb_store::{
    LanceDbProjectionReader, ProjectionContentRow, expected_chunk_projection_content_rows,
    expected_label_atom_projection_content_rows,
};
use crate::{EmbeddingExecutionPolicy, LanceDbConfig, LanceDbStore};

const EVIDENCE_FILE: &str = "projection-evidence.json";
const SNAPSHOT_FILE: &str = "projection-snapshot.json";
const PUBLISHED_MARKER: &str = "published";
const LANCE_DATA_DIR: &str = "lance";
const EMBEDDING_CACHE_FILE: &str = "embedding-cache.json";
const DELIVERY_STATE_FILE: &str = "delivery-state.json";
const CONTENT_METADATA_FILE: &str = "projection-content.json";

const GENERATION_OPERATIONS: [VectorProjectionHelperOperation; 13] = [
    VectorProjectionHelperOperation::Descriptor,
    VectorProjectionHelperOperation::PrepareSnapshot,
    VectorProjectionHelperOperation::ApplyBatch,
    VectorProjectionHelperOperation::Publish,
    VectorProjectionHelperOperation::InspectActive,
    VectorProjectionHelperOperation::InspectGeneration,
    VectorProjectionHelperOperation::ValidateGenerationPublication,
    VectorProjectionHelperOperation::ValidateActiveContents,
    VectorProjectionHelperOperation::RepairPublication,
    VectorProjectionHelperOperation::Quarantine,
    VectorProjectionHelperOperation::Abort,
    VectorProjectionHelperOperation::Inventory,
    VectorProjectionHelperOperation::Cleanup,
];

#[derive(Debug, Error)]
pub enum VectorProjectionBackendError {
    #[error("{0}")]
    Protocol(String),
    #[error("{message}")]
    Provider { message: String, retryable: bool },
    #[error("{0}")]
    Busy(String),
    #[error("{0}")]
    Delivery(String),
    #[error("{0}")]
    Backend(String),
}

impl VectorProjectionBackendError {
    fn kind(&self) -> VectorProjectionHelperErrorKind {
        match self {
            Self::Protocol(_) => VectorProjectionHelperErrorKind::Protocol,
            Self::Provider { .. } => VectorProjectionHelperErrorKind::Provider,
            Self::Busy(_) => VectorProjectionHelperErrorKind::Backend,
            Self::Delivery(_) => VectorProjectionHelperErrorKind::Delivery,
            Self::Backend(_) => VectorProjectionHelperErrorKind::Backend,
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::Protocol(_) => "projection_protocol_mismatch",
            Self::Provider { .. } => "embedding_provider_failure",
            Self::Busy(_) => "projection_backend_busy",
            Self::Delivery(_) => "projection_delivery_mismatch",
            Self::Backend(_) => "projection_backend_failure",
        }
    }
}

/// Filesystem owner for the LanceDB Projection v2 generation namespace.
///
/// Snapshot payloads and read-only SQLite hydration are materialized into
/// generation-local Lance tables. Incremental deliveries are treated only as
/// fenced invalidation/correlation records; they never become canonical
/// documents themselves.
pub struct VectorProjectionBackend {
    db_path: PathBuf,
    database_instance_id: String,
    provider: Arc<dyn EmbeddingProvider + Send + Sync>,
    execution_policy: EmbeddingExecutionPolicy,
    stores: Vec<ProjectionStoreDescriptor>,
}

/// Read-only resolver for one SQLite-authorized active LanceDB Projection v2
/// generation.
///
/// Construction validates the complete SQLite authority, publication marker,
/// evidence, highest published fence, auxiliary bindings, physical row
/// fingerprint, and canonical SQLite content before exposing query methods.
/// It deliberately implements no mutation trait and never opens a legacy v1
/// store or an `ensure_*` table path.
pub struct ActiveLanceProjectionReader {
    generation: String,
    store_name: String,
    resolved_board_id: Option<String>,
    provider: Arc<dyn EmbeddingProvider + Send + Sync>,
    reader: LanceDbProjectionReader,
    generation_guard: DirectoryIdentityGuard,
    table_guard: DirectoryIdentityGuard,
    _read_guard: DerivedStoreReadGuard,
}

struct GuardedLanceDbStore {
    store: LanceDbStore,
    generation_guard: DirectoryIdentityGuard,
    table_guard: Option<DirectoryIdentityGuard>,
}

struct GuardedProjectionReader {
    reader: LanceDbProjectionReader,
    generation_guard: DirectoryIdentityGuard,
    table_guard: DirectoryIdentityGuard,
}

impl std::ops::Deref for GuardedLanceDbStore {
    type Target = LanceDbStore;

    fn deref(&self) -> &Self::Target {
        &self.store
    }
}

impl GuardedLanceDbStore {
    fn validate_path_identity(&self) -> Result<(), VectorProjectionBackendError> {
        self.generation_guard
            .validate_path_identity()
            .map_err(backend_io)?;
        if let Some(table_guard) = &self.table_guard {
            table_guard.validate_path_identity().map_err(backend_io)?;
        }
        Ok(())
    }
}

impl std::ops::Deref for GuardedProjectionReader {
    type Target = LanceDbProjectionReader;

    fn deref(&self) -> &Self::Target {
        &self.reader
    }
}

impl GuardedProjectionReader {
    fn validate_path_identity(&self) -> Result<(), VectorProjectionBackendError> {
        self.generation_guard
            .validate_path_identity()
            .map_err(backend_io)?;
        self.table_guard
            .validate_path_identity()
            .map_err(backend_io)
    }
}

impl std::fmt::Debug for ActiveLanceProjectionReader {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ActiveLanceProjectionReader")
            .field("generation", &self.generation)
            .field("store_name", &self.store_name)
            .field("resolved_board_id", &self.resolved_board_id)
            .field("provider", &self.provider.provider_name())
            .field("embedding_model", &self.provider.embedding_model())
            .field("embedding_dimensions", &self.provider.dimensions())
            .finish()
    }
}

impl ActiveLanceProjectionReader {
    pub fn resolve_board(
        db_path: impl AsRef<Path>,
        board_reference: &str,
    ) -> Result<String, VectorProjectionBackendError> {
        let mut database = open_readonly_database(db_path.as_ref())?;
        let transaction = database
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(backend_sql)?;
        let _database_instance_id = read_database_instance_id(&transaction)?;
        let resolved = board_id(&transaction, board_reference).map_err(derived_backend)?;
        transaction.commit().map_err(backend_sql)?;
        Ok(resolved)
    }

    pub fn open(
        db_path: impl AsRef<Path>,
        store_name: &str,
        provider: Arc<dyn EmbeddingProvider + Send + Sync>,
    ) -> Result<Self, VectorProjectionBackendError> {
        let db_path = db_path.as_ref().to_path_buf();
        VectorProjectionBackend::new(&db_path, provider)?.open_active_reader(store_name)
    }

    pub fn open_for_board(
        db_path: impl AsRef<Path>,
        store_name: &str,
        board: &str,
        expected_board_id: Option<&str>,
        provider: Arc<dyn EmbeddingProvider + Send + Sync>,
    ) -> Result<Self, VectorProjectionBackendError> {
        let db_path = db_path.as_ref().to_path_buf();
        VectorProjectionBackend::new(&db_path, provider)?
            .open_active_reader_for_board(store_name, board, expected_board_id, None)
            .map(|(reader, _)| reader)
    }

    pub fn open_for_board_with_status(
        db_path: impl AsRef<Path>,
        store_name: &str,
        board: &str,
        expected_board_id: Option<&str>,
        provider: Arc<dyn EmbeddingProvider + Send + Sync>,
        base_status: VectorStoreStatus,
    ) -> Result<(Self, VectorStoreStatus), VectorProjectionBackendError> {
        let db_path = db_path.as_ref().to_path_buf();
        let (reader, status) = VectorProjectionBackend::new(&db_path, provider)?
            .open_active_reader_for_board(
                store_name,
                board,
                expected_board_id,
                Some(base_status),
            )?;
        Ok((
            reader,
            status.expect("status was requested for the active read session"),
        ))
    }

    pub fn generation(&self) -> &str {
        &self.generation
    }

    pub fn store_name(&self) -> &str {
        &self.store_name
    }

    pub fn resolved_board_id(&self) -> Option<&str> {
        self.resolved_board_id.as_deref()
    }

    fn require_query_board_scope(
        &self,
        requested_board_id: &str,
    ) -> Result<(), VectorProjectionBackendError> {
        if let Some(resolved_board_id) = self.resolved_board_id()
            && requested_board_id != resolved_board_id
        {
            return Err(VectorProjectionBackendError::Protocol(format!(
                "board mismatch: active reader is scoped to {resolved_board_id}, got request board {requested_board_id}"
            )));
        }
        Ok(())
    }

    fn validate_projection_path(&self) -> Result<(), VectorProjectionBackendError> {
        self.generation_guard
            .validate_path_identity()
            .map_err(backend_io)?;
        self.table_guard
            .validate_path_identity()
            .map_err(backend_io)
    }

    pub fn query_chunks(
        &self,
        query: &VectorQuery,
    ) -> Result<Vec<VectorHit>, VectorProjectionBackendError> {
        if self.store_name != LANCEDB_CHUNKS_STORE || query.board_id.trim().is_empty() {
            return Err(VectorProjectionBackendError::Protocol(
                "active task chunk queries require the lancedb_chunks store and one resolved board"
                    .to_owned(),
            ));
        }
        self.require_query_board_scope(&query.board_id)?;
        self.validate_projection_path()?;
        let result = self
            .reader
            .query_chunks(self.provider.as_ref(), query)
            .map_err(map_vector_error);
        self.validate_projection_path()?;
        result
    }

    pub fn query_label_atoms(
        &self,
        query: &LabelAtomQuery,
    ) -> Result<Vec<LabelAtomHit>, VectorProjectionBackendError> {
        if self.store_name != LANCEDB_LABEL_ATOMS_STORE
            || query
                .board_id
                .as_deref()
                .is_none_or(|board_id| board_id.trim().is_empty())
        {
            return Err(VectorProjectionBackendError::Protocol(
                "active label atom queries require the lancedb_label_atoms store and one resolved board"
                    .to_owned(),
            ));
        }
        self.require_query_board_scope(query.board_id.as_deref().unwrap_or_default())?;
        self.validate_projection_path()?;
        let result = self
            .reader
            .query_label_atoms(self.provider.as_ref(), query)
            .map_err(map_vector_error);
        self.validate_projection_path()?;
        result
    }

    pub fn query_label_atoms_by_vector(
        &self,
        query: &LabelAtomVectorQuery,
    ) -> Result<Vec<LabelAtomVectorHit>, VectorProjectionBackendError> {
        if self.store_name != LANCEDB_LABEL_ATOMS_STORE
            || query
                .board_id
                .as_deref()
                .is_none_or(|board_id| board_id.trim().is_empty())
        {
            return Err(VectorProjectionBackendError::Protocol(
                "active label atom vector queries require the lancedb_label_atoms store and one resolved board"
                    .to_owned(),
            ));
        }
        self.require_query_board_scope(query.board_id.as_deref().unwrap_or_default())?;
        self.validate_projection_path()?;
        let result = self
            .reader
            .query_label_atoms_by_vector(self.provider.as_ref(), query)
            .map_err(map_vector_error);
        self.validate_projection_path()?;
        result
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SqliteGenerationAuthority {
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
    phase: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SqliteMutationAuthority {
    database_instance_id: String,
    protocol_version: i64,
    schema_version: i64,
    control_plane: String,
    fence_epoch: i64,
    lease_owner: Option<String>,
    lease_token: Option<String>,
    lease_expires_at: Option<i64>,
    building: SqliteGenerationAuthority,
    active: SqliteGenerationAuthority,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SqliteDeliveryClaim {
    id: i64,
    outbox_id: i64,
    store_name: String,
    board_id: String,
    source_event_id: Option<i64>,
    cursor: i64,
    action: String,
    entity_uri: String,
    payload_json: String,
    status: String,
    attempts: i64,
    claim_owner: Option<String>,
    claim_token: Option<String>,
    claim_lease_token: Option<String>,
    claim_fence_epoch: Option<i64>,
    claim_generation: Option<String>,
    claim_expires_at: Option<i64>,
}

impl SqliteGenerationAuthority {
    fn matches_manifest(
        &self,
        manifest: &ProjectionArtifactManifest,
        fingerprint: Option<&str>,
    ) -> bool {
        let corpus = manifest.corpus.as_ref();
        self.generation.as_deref() == Some(manifest.generation.as_str())
            && self.fingerprint.as_deref() == fingerprint
            && self.fence_epoch == Some(manifest.fence_epoch)
            && self.snapshot_cursor == Some(manifest.snapshot_cursor)
            && self.provider.as_deref() == Some(manifest.provider.as_str())
            && self.provider_fingerprint.as_deref() == Some(manifest.provider_fingerprint.as_str())
            && self.canonical_item_count == Some(manifest.canonical_item_count)
            && self.canonical_digest.as_deref() == Some(manifest.canonical_digest.as_str())
            && self.delivery_item_count == Some(manifest.delivery_item_count)
            && self.delivery_digest.as_deref() == Some(manifest.delivery_digest.as_str())
            && self.corpus_schema.as_deref() == corpus.map(|corpus| corpus.corpus_schema.as_str())
            && self.corpus_fingerprint.as_deref()
                == corpus.map(|corpus| corpus.corpus_fingerprint.as_str())
            && self.embedding_model.as_deref()
                == corpus.map(|corpus| corpus.embedding_model.as_str())
            && self.embedding_dimensions
                == corpus.and_then(|corpus| i64::try_from(corpus.embedding_dimensions).ok())
    }

    fn is_absent(&self) -> bool {
        self.generation.is_none()
            && self.fingerprint.is_none()
            && self.fence_epoch.is_none()
            && self.snapshot_cursor.is_none()
            && self.provider.is_none()
            && self.provider_fingerprint.is_none()
            && self.canonical_item_count.is_none()
            && self.canonical_digest.is_none()
            && self.delivery_item_count.is_none()
            && self.delivery_digest.is_none()
            && self.corpus_schema.is_none()
            && self.corpus_fingerprint.is_none()
            && self.embedding_model.is_none()
            && self.embedding_dimensions.is_none()
            && self.phase.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CachedEmbedding {
    content_hash: String,
    normalized_text: String,
    vector: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmbeddingCacheFile {
    format_version: i64,
    database_instance_id: String,
    store_name: String,
    generation_id: String,
    provider_fingerprint: String,
    corpus_fingerprint: String,
    embedding_model: String,
    embedding_dimensions: usize,
    entries: BTreeMap<String, CachedEmbedding>,
}

struct PersistentEmbeddingProvider {
    inner: Arc<dyn EmbeddingProvider + Send + Sync>,
    path: PathBuf,
    cache_key_prefix: String,
    state: Mutex<EmbeddingCacheFile>,
}

impl std::fmt::Debug for PersistentEmbeddingProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PersistentEmbeddingProvider")
            .field("path", &self.path)
            .field("provider", &self.inner.provider_name())
            .field("embedding_model", &self.inner.embedding_model())
            .field("embedding_dimensions", &self.inner.dimensions())
            .finish_non_exhaustive()
    }
}

impl EmbeddingProvider for PersistentEmbeddingProvider {
    fn provider_name(&self) -> &str {
        self.inner.provider_name()
    }

    fn embedding_model(&self) -> &str {
        self.inner.embedding_model()
    }

    fn dimensions(&self) -> usize {
        self.inner.dimensions()
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, VectorError> {
        self.embed_batch(&[text.to_owned()])?
            .into_iter()
            .next()
            .ok_or_else(|| VectorError::Store("persistent embedding result was missing".to_owned()))
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, VectorError> {
        let requested = texts
            .iter()
            .map(|text| {
                let normalized_text = normalize_semantic_text(text);
                let content_hash = semantic_content_hash(&normalized_text);
                let cache_key = embedding_cache_key(&self.cache_key_prefix, &content_hash);
                (cache_key, content_hash, normalized_text)
            })
            .collect::<Vec<_>>();
        let missing = {
            let state = self.lock_state()?;
            let mut missing = BTreeMap::<String, (String, String)>::new();
            for (cache_key, content_hash, normalized_text) in &requested {
                match state.entries.get(cache_key) {
                    Some(cached)
                        if cached.content_hash == *content_hash
                            && cached.normalized_text == *normalized_text =>
                    {
                        ensure_dimensions(&cached.vector, self.dimensions())?;
                    }
                    Some(_) => {
                        return Err(VectorError::Store(format!(
                            "embedding cache content hash collision for {content_hash}"
                        )));
                    }
                    None => {
                        missing.insert(
                            cache_key.clone(),
                            (content_hash.clone(), normalized_text.clone()),
                        );
                    }
                }
            }
            missing
        };

        if !missing.is_empty() {
            let missing_items = missing.into_iter().collect::<Vec<_>>();
            let missing_texts = missing_items
                .iter()
                .map(|(_, (_, text))| text.clone())
                .collect::<Vec<_>>();
            let embeddings = self.inner.embed_batch(&missing_texts)?;
            if embeddings.len() != missing_items.len() {
                return Err(VectorError::Store(format!(
                    "embedding provider batch cardinality mismatch: expected {}, got {}",
                    missing_items.len(),
                    embeddings.len()
                )));
            }
            for embedding in &embeddings {
                ensure_dimensions(embedding, self.dimensions())?;
            }

            let mut state = self.lock_state()?;
            for ((cache_key, (content_hash, normalized_text)), vector) in
                missing_items.into_iter().zip(embeddings)
            {
                match state.entries.get(&cache_key) {
                    Some(cached)
                        if cached.content_hash != content_hash
                            || cached.normalized_text != normalized_text =>
                    {
                        return Err(VectorError::Store(format!(
                            "embedding cache content hash collision for {content_hash}"
                        )));
                    }
                    Some(_) => {}
                    None => {
                        state.entries.insert(
                            cache_key,
                            CachedEmbedding {
                                content_hash,
                                normalized_text,
                                vector,
                            },
                        );
                    }
                }
            }
            persist_json(&self.path, &*state).map_err(vector_backend_io)?;
        }

        let state = self.lock_state()?;
        requested
            .into_iter()
            .map(|(cache_key, content_hash, normalized_text)| {
                let cached = state.entries.get(&cache_key).ok_or_else(|| {
                    VectorError::Store(format!(
                        "persistent embedding cache lost content hash {content_hash}"
                    ))
                })?;
                if cached.content_hash != content_hash || cached.normalized_text != normalized_text
                {
                    return Err(VectorError::Store(format!(
                        "embedding cache content hash collision for {content_hash}"
                    )));
                }
                ensure_dimensions(&cached.vector, self.dimensions())?;
                Ok(cached.vector.clone())
            })
            .collect()
    }

    fn provider_fingerprint(&self) -> String {
        self.inner.provider_fingerprint()
    }
}

impl PersistentEmbeddingProvider {
    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, EmbeddingCacheFile>, VectorError> {
        self.state
            .lock()
            .map_err(|_| VectorError::Store("persistent embedding cache lock poisoned".to_owned()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeliveryStateFile {
    format_version: i64,
    database_instance_id: String,
    store_name: String,
    generation_id: String,
    provider_fingerprint: String,
    corpus_fingerprint: String,
    evidence_fingerprint: String,
    applied: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectionContentMetadata {
    format_version: i64,
    database_instance_id: String,
    store_name: String,
    generation_id: String,
    provider_fingerprint: String,
    corpus_fingerprint: String,
    evidence_fingerprint: String,
    row_count: usize,
    content_fingerprint: String,
}

#[derive(Debug, Deserialize)]
struct TaskSnapshotPayload {
    board_id: String,
    task_id: String,
    status: String,
    title: String,
    description: Option<String>,
    comments: String,
    run_text: String,
    event_text: String,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Deserialize)]
struct LabelAtomSnapshotPayload {
    atom_id: String,
    label_id: String,
    label_name: String,
    polarity: String,
    kind: String,
    text: String,
    ordinal: i64,
    content_hash: String,
    created_at: i64,
    updated_at: i64,
}

impl std::fmt::Debug for VectorProjectionBackend {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VectorProjectionBackend")
            .field("db_path", &self.db_path)
            .field(
                "database_instance_id",
                &if self.database_instance_id.is_empty() {
                    "<unbound>"
                } else {
                    self.database_instance_id.as_str()
                },
            )
            .field("provider", &self.provider.provider_name())
            .field("embedding_model", &self.provider.embedding_model())
            .field("embedding_dimensions", &self.provider.dimensions())
            .finish_non_exhaustive()
    }
}

impl VectorProjectionBackend {
    pub fn new(
        db_path: impl AsRef<Path>,
        provider: Arc<dyn EmbeddingProvider + Send + Sync>,
    ) -> Result<Self, VectorProjectionBackendError> {
        if provider.provider_name().trim().is_empty()
            || provider.embedding_model().trim().is_empty()
            || provider.dimensions() == 0
        {
            return Err(VectorProjectionBackendError::Protocol(
                "embedding provider identity, model and dimensions must be non-empty".to_owned(),
            ));
        }
        let db_path = db_path.as_ref().to_path_buf();
        let provider_fingerprint = provider.provider_fingerprint();
        let stores = [
            (LANCEDB_CHUNKS_STORE, TASK_CHUNKS_CORPUS_SCHEMA),
            (LANCEDB_LABEL_ATOMS_STORE, LABEL_ATOMS_CORPUS_SCHEMA),
        ]
        .into_iter()
        .map(|(store_name, corpus_schema)| ProjectionStoreDescriptor {
            store_name: store_name.to_owned(),
            schema_version: DERIVED_STORE_SCHEMA_VERSION,
            provider: provider.provider_name().to_owned(),
            provider_fingerprint: provider_fingerprint.clone(),
            corpus: Some(ProjectionCorpusMetadata {
                corpus_schema: corpus_schema.to_owned(),
                corpus_fingerprint: corpus_provider_fingerprint(
                    corpus_schema,
                    &provider_fingerprint,
                ),
                embedding_model: provider.embedding_model().to_owned(),
                embedding_dimensions: provider.dimensions(),
            }),
        })
        .collect();
        Ok(Self {
            db_path,
            database_instance_id: String::new(),
            provider,
            execution_policy: EmbeddingExecutionPolicy::default(),
            stores,
        })
    }

    pub fn with_execution_policy(mut self, execution_policy: EmbeddingExecutionPolicy) -> Self {
        self.execution_policy = execution_policy;
        self
    }

    fn bind_database_instance(&self, database_instance_id: String) -> Self {
        Self {
            db_path: self.db_path.clone(),
            database_instance_id,
            provider: Arc::clone(&self.provider),
            execution_policy: self.execution_policy.clone(),
            stores: self.stores.clone(),
        }
    }

    fn open_active_reader(
        self,
        store_name: &str,
    ) -> Result<ActiveLanceProjectionReader, VectorProjectionBackendError> {
        self.open_active_reader_session(store_name, None, None)
            .map(|(reader, _)| reader)
    }

    fn open_active_reader_for_board(
        self,
        store_name: &str,
        board_reference: &str,
        expected_board_id: Option<&str>,
        base_status: Option<VectorStoreStatus>,
    ) -> Result<
        (ActiveLanceProjectionReader, Option<VectorStoreStatus>),
        VectorProjectionBackendError,
    > {
        self.open_active_reader_session(
            store_name,
            Some((board_reference, expected_board_id)),
            base_status,
        )
    }

    fn open_active_reader_session(
        self,
        store_name: &str,
        board: Option<(&str, Option<&str>)>,
        base_status: Option<VectorStoreStatus>,
    ) -> Result<
        (ActiveLanceProjectionReader, Option<VectorStoreStatus>),
        VectorProjectionBackendError,
    > {
        let mut database = open_readonly_database(&self.db_path)?;
        let transaction = database
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(backend_sql)?;
        // The identity read is deliberately first so SQLite pins one WAL
        // snapshot before authority or canonical content is observed.
        let database_instance_id = read_database_instance_id(&transaction)?;
        let resolved_board_id = board
            .map(|(board_reference, expected_board_id)| {
                let resolved = board_id(&transaction, board_reference).map_err(derived_backend)?;
                if expected_board_id.is_some_and(|expected| expected != resolved) {
                    return Err(VectorProjectionBackendError::Protocol(format!(
                        "board mismatch: {board_reference} resolved to {resolved}, got expected board id {}",
                        expected_board_id.unwrap_or_default()
                    )));
                }
                Ok(resolved)
            })
            .transpose()?;
        let bound = self.bind_database_instance(database_instance_id);
        let mut reader = bound.open_active_reader_in_session(store_name, &transaction)?;
        reader.resolved_board_id.clone_from(&resolved_board_id);
        let status = base_status
            .map(|status| {
                let resolved = resolved_board_id.as_deref().ok_or_else(|| {
                    VectorProjectionBackendError::Protocol(
                        "projection status requires one resolved board".to_owned(),
                    )
                })?;
                projection_status_from_base(
                    &transaction,
                    store_name,
                    resolved,
                    reader.generation(),
                    status,
                )
            })
            .transpose()?;
        transaction.commit().map_err(backend_sql)?;
        Ok((reader, status))
    }

    fn open_active_reader_in_session(
        self,
        store_name: &str,
        sqlite: &Connection,
    ) -> Result<ActiveLanceProjectionReader, VectorProjectionBackendError> {
        let descriptor = self.require_store(store_name)?;
        let authority = self.load_sqlite_mutation_authority_from(sqlite, store_name)?;
        self.require_sqlite_read_identity(&authority, store_name)?;
        let read_guard = acquire_helper_read_guard(&self.db_path, store_name)?;
        if authority.active.is_absent() {
            return Err(VectorProjectionBackendError::Delivery(format!(
                "SQLite Projection v2 has no active generation for {store_name}"
            )));
        }
        let generation = authority
            .active
            .generation
            .as_deref()
            .ok_or_else(|| {
                VectorProjectionBackendError::Delivery(format!(
                    "SQLite Projection v2 active generation is incomplete for {store_name}"
                ))
            })?
            .to_owned();
        let generations = self.generations_root(store_name, false)?;
        let generation_path = checked_generation_path(&generations, &generation)?;
        require_real_directory(&generation_path, "active LanceDB generation")?;
        let evidence = self.read_evidence_at(descriptor, &generation_path)?;
        self.validate_manifest_current(&evidence.manifest, descriptor)?;
        if !authority
            .active
            .matches_manifest(&evidence.manifest, Some(evidence.fingerprint.as_str()))
        {
            return Err(VectorProjectionBackendError::Delivery(format!(
                "SQLite Projection v2 active authority does not match {store_name}/{generation}"
            )));
        }
        if !self.marker_is_valid(&evidence)? {
            return Err(VectorProjectionBackendError::Backend(format!(
                "active published marker is missing or corrupt for {store_name}/{generation}"
            )));
        }
        let highest = self
            .published_generations(store_name)?
            .pop()
            .ok_or_else(|| {
                VectorProjectionBackendError::Backend(format!(
                    "no published Projection v2 generation exists for {store_name}"
                ))
            })?;
        if highest != evidence {
            return Err(VectorProjectionBackendError::Delivery(format!(
                "SQLite active generation is not the highest published fence for {store_name}"
            )));
        }
        self.validate_historical_materialization(&evidence, &generation_path)?;
        self.validate_canonical_content_from(sqlite, descriptor, &generation_path, &evidence)?;
        self.validate_historical_auxiliary_state(&generation_path, &evidence)?;
        let corpus = evidence.manifest.corpus.as_ref().ok_or_else(|| {
            VectorProjectionBackendError::Delivery(
                "active LanceDB evidence has no corpus binding".to_owned(),
            )
        })?;
        let lance_path = generation_path.join(LANCE_DATA_DIR);
        let generation_guard = DirectoryIdentityGuard::acquire(&lance_path).map_err(backend_io)?;
        let table_path = lance_path.join(lance_table_directory_name(store_name)?);
        let table_guard = DirectoryIdentityGuard::acquire(&table_path).map_err(backend_io)?;
        generation_guard
            .validate_path_identity()
            .map_err(backend_io)?;
        table_guard.validate_path_identity().map_err(backend_io)?;
        let reader = LanceDbProjectionReader::open_existing(
            generation_guard.canonical_path(),
            corpus.embedding_dimensions,
        )
        .map_err(map_vector_error)?;
        generation_guard
            .validate_path_identity()
            .map_err(backend_io)?;
        table_guard.validate_path_identity().map_err(backend_io)?;
        Ok(ActiveLanceProjectionReader {
            generation,
            store_name: store_name.to_owned(),
            resolved_board_id: None,
            provider: self.provider,
            reader,
            generation_guard,
            table_guard,
            _read_guard: read_guard,
        })
    }

    pub fn descriptor(&self, request_id: impl Into<String>) -> VectorProjectionHelperDescriptor {
        VectorProjectionHelperDescriptor {
            request_id: request_id.into(),
            protocol_version: VECTOR_PROJECTION_PROTOCOL_VERSION,
            build_identity: crate::vector_helper_build_identity().to_owned(),
            supported_stores: self.stores.clone(),
            supported_operations: GENERATION_OPERATIONS.to_vec(),
        }
    }

    pub fn execute(
        &self,
        request: &VectorProjectionHelperRequest,
    ) -> VectorProjectionHelperResponse {
        match self.try_execute(request) {
            Ok(response) => response,
            Err(error) => self.error_response(request, error),
        }
    }

    fn try_execute(
        &self,
        request: &VectorProjectionHelperRequest,
    ) -> Result<VectorProjectionHelperResponse, VectorProjectionBackendError> {
        if !matches!(request, VectorProjectionHelperRequest::Descriptor(_))
            && self.database_instance_id.is_empty()
        {
            let database = open_readonly_database(&self.db_path)?;
            let database_instance_id = read_database_instance_id(&database)?;
            let bound = self.bind_database_instance(database_instance_id);
            let result = bound.try_execute(request);
            drop(database);
            return result;
        }
        match request {
            VectorProjectionHelperRequest::Descriptor(request) => {
                require_non_empty(&request.request_id, "request_id")?;
                Ok(VectorProjectionHelperResponse::Descriptor(
                    self.descriptor(&request.request_id),
                ))
            }
            VectorProjectionHelperRequest::PrepareSnapshot(request) => {
                let _guard = self.acquire_mutation_guard(&request.context.projection_store)?;
                self.validate_prepare_authority(request)?;
                let evidence = self.prepare_snapshot(request)?;
                Ok(VectorProjectionHelperResponse::PrepareSnapshot(
                    VectorProjectionPrepareSnapshotResponse {
                        ack: ack(&request.context),
                        evidence,
                    },
                ))
            }
            VectorProjectionHelperRequest::ApplyBatch(request) => {
                let _guard = self.acquire_mutation_guard(&request.context.projection_store)?;
                self.validate_apply_authority(request)?;
                let receipt = self.apply_batch(request)?;
                Ok(VectorProjectionHelperResponse::ApplyBatch(
                    VectorProjectionApplyBatchResponse {
                        ack: ack(&request.context),
                        receipt,
                    },
                ))
            }
            VectorProjectionHelperRequest::Publish(request) => {
                let _guard = self.acquire_mutation_guard(&request.context.projection_store)?;
                self.validate_publish_authority(request)?;
                let receipt = self.publish(request)?;
                Ok(VectorProjectionHelperResponse::Publish(Box::new(
                    VectorProjectionPublishResponse {
                        ack: ack(&request.context),
                        receipt,
                    },
                )))
            }
            VectorProjectionHelperRequest::InspectActive(request) => {
                require_non_empty(&request.request_id, "request_id")?;
                self.require_store(&request.projection_store)?;
                let _guard = acquire_helper_read_guard(&self.db_path, &request.projection_store)?;
                Ok(VectorProjectionHelperResponse::InspectActive(
                    VectorProjectionInspectActiveResponse {
                        request_id: request.request_id.clone(),
                        projection_store: request.projection_store.clone(),
                        active: self.inspect_active(&request.projection_store)?,
                    },
                ))
            }
            VectorProjectionHelperRequest::InspectGeneration(request) => {
                require_non_empty(&request.request_id, "request_id")?;
                self.require_store(&request.projection_store)?;
                let _guard = acquire_helper_read_guard(&self.db_path, &request.projection_store)?;
                Ok(VectorProjectionHelperResponse::InspectGeneration(
                    VectorProjectionInspectGenerationResponse {
                        request_id: request.request_id.clone(),
                        projection_store: request.projection_store.clone(),
                        generation_id: request.generation_id.clone(),
                        evidence: self.inspect_generation(
                            &request.projection_store,
                            &request.generation_id,
                        )?,
                    },
                ))
            }
            VectorProjectionHelperRequest::ValidateGenerationPublication(request) => {
                let _guard = acquire_helper_read_guard(&self.db_path, &request.projection_store)?;
                Ok(
                    VectorProjectionHelperResponse::ValidateGenerationPublication(
                        self.validate_generation_publication(request)?,
                    ),
                )
            }
            VectorProjectionHelperRequest::ValidateActiveContents(request) => {
                let _guard = acquire_helper_read_guard(&self.db_path, &request.projection_store)?;
                Ok(VectorProjectionHelperResponse::ValidateActiveContents(
                    self.validate_active_contents(request)?,
                ))
            }
            VectorProjectionHelperRequest::RepairPublication(request) => {
                let _guard = self.acquire_mutation_guard(&request.context.projection_store)?;
                self.validate_repair_authority(request)?;
                self.repair_publication(request)?;
                Ok(VectorProjectionHelperResponse::RepairPublication(ack(
                    &request.context,
                )))
            }
            VectorProjectionHelperRequest::Quarantine(request) => {
                let _guard = self.acquire_mutation_guard(&request.context.projection_store)?;
                self.quarantine(request)?;
                Ok(VectorProjectionHelperResponse::Quarantine(ack(
                    &request.context
                )))
            }
            VectorProjectionHelperRequest::Abort(request) => {
                let _guard = self.acquire_mutation_guard(&request.context.projection_store)?;
                self.abort(request)?;
                Ok(VectorProjectionHelperResponse::Abort(ack(&request.context)))
            }
            VectorProjectionHelperRequest::Inventory(request) => {
                require_non_empty(&request.request_id, "request_id")?;
                self.require_store(&request.projection_store)?;
                let _guard = acquire_helper_read_guard(&self.db_path, &request.projection_store)?;
                Ok(VectorProjectionHelperResponse::Inventory(
                    VectorProjectionInventoryResponse {
                        request_id: request.request_id.clone(),
                        projection_store: request.projection_store.clone(),
                        generations: self.inventory(&request.projection_store)?,
                    },
                ))
            }
            VectorProjectionHelperRequest::Cleanup(request) => {
                if request.dry_run {
                    let _guard = acquire_helper_read_guard(
                        &self.db_path,
                        &request.context.projection_store,
                    )?;
                    Ok(VectorProjectionHelperResponse::Cleanup(
                        self.cleanup(request)?,
                    ))
                } else {
                    let _guard = self.acquire_mutation_guard(&request.context.projection_store)?;
                    Ok(VectorProjectionHelperResponse::Cleanup(
                        self.cleanup(request)?,
                    ))
                }
            }
        }
    }

    fn prepare_snapshot(
        &self,
        request: &VectorProjectionPrepareSnapshotRequest,
    ) -> Result<ProjectionArtifactEvidence, VectorProjectionBackendError> {
        self.validate_context(&request.context)?;
        let descriptor = self.require_store(&request.context.projection_store)?;
        if request.metadata
            != descriptor.corpus.clone().ok_or_else(|| {
                VectorProjectionBackendError::Protocol(
                    "projection descriptor is missing its corpus binding".to_owned(),
                )
            })?
        {
            return Err(VectorProjectionBackendError::Delivery(
                "prepare metadata does not match the configured corpus".to_owned(),
            ));
        }
        self.validate_snapshot(&request.snapshot, descriptor)?;
        if request.context.generation_id != request.snapshot.manifest.generation
            || request.context.delivery_digest != request.snapshot.manifest.delivery_digest
        {
            return Err(VectorProjectionBackendError::Delivery(
                "prepare mutation context does not match the snapshot manifest".to_owned(),
            ));
        }

        let fingerprint = snapshot_fingerprint(&request.snapshot);
        let mut persisted_snapshot = request.snapshot.clone();
        persisted_snapshot.manifest.fingerprint = Some(fingerprint.clone());
        let evidence = ProjectionArtifactEvidence {
            manifest: persisted_snapshot.manifest.clone(),
            fingerprint,
        };
        let generations = self.generations_root(&descriptor.store_name, true)?;
        let generation_path = checked_generation_path(&generations, &evidence.manifest.generation)?;
        let mut resume = false;
        match fs::symlink_metadata(&generation_path) {
            Ok(metadata) if metadata.is_dir() => {
                if let Ok(stored) = self.read_evidence_at(descriptor, &generation_path) {
                    if stored == evidence {
                        match self.validate_generation_materialization(descriptor, &generation_path)
                        {
                            Ok(())
                                if self
                                    .validate_content_metadata(&generation_path, &stored)
                                    .is_ok() =>
                            {
                                return Ok(stored);
                            }
                            Err(error) if marker_exists(&generation_path)? => return Err(error),
                            Ok(()) | Err(_) => {
                                if marker_exists(&generation_path)? {
                                    return Err(VectorProjectionBackendError::Backend(
                                        "published generation physical contents are invalid"
                                            .to_owned(),
                                    ));
                                }
                                durable_quarantine_entry(&generation_path).map_err(backend_io)?;
                            }
                        }
                    } else if marker_exists(&generation_path)? {
                        return Err(VectorProjectionBackendError::Delivery(format!(
                            "published generation {} cannot be replaced",
                            evidence.manifest.generation
                        )));
                    } else {
                        durable_quarantine_entry(&generation_path).map_err(backend_io)?;
                    }
                } else if marker_exists(&generation_path)? {
                    return Err(VectorProjectionBackendError::Delivery(format!(
                        "published generation {} cannot be replaced",
                        evidence.manifest.generation
                    )));
                } else if path_exists(&generation_path.join(EVIDENCE_FILE))? {
                    durable_quarantine_entry(&generation_path).map_err(backend_io)?;
                } else {
                    match self.read_snapshot_at(descriptor, &generation_path) {
                        Ok(stored_snapshot) if stored_snapshot == persisted_snapshot => {
                            resume = true;
                        }
                        Ok(_) | Err(_) => {
                            durable_quarantine_entry(&generation_path).map_err(backend_io)?;
                        }
                    }
                }
            }
            Ok(_) => {
                durable_quarantine_entry(&generation_path).map_err(backend_io)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(backend_io(error)),
        }

        if !resume {
            durable_create_dir_all(&generation_path).map_err(backend_io)?;
            let snapshot_bytes = serde_json::to_vec(&persisted_snapshot).map_err(backend_json)?;
            durable_create_new_file(&generation_path.join(SNAPSHOT_FILE), &snapshot_bytes)
                .map_err(backend_io)?;
        }
        self.hydrate_snapshot(descriptor, &generation_path, &persisted_snapshot)?;
        self.validate_generation_materialization(descriptor, &generation_path)?;
        self.validate_snapshot_materialization(descriptor, &generation_path, &persisted_snapshot)?;
        self.write_content_metadata(descriptor, &generation_path, &evidence)?;
        let evidence_bytes = serde_json::to_vec(&evidence).map_err(backend_json)?;
        durable_create_new_file(&generation_path.join(EVIDENCE_FILE), &evidence_bytes)
            .map_err(backend_io)?;
        durable_sync_directory_tree(&generation_path).map_err(backend_io)?;
        let stored = self.read_evidence_at(descriptor, &generation_path)?;
        if stored != evidence {
            return Err(VectorProjectionBackendError::Backend(
                "prepared generation read-back did not match persisted evidence".to_owned(),
            ));
        }
        Ok(stored)
    }

    fn read_snapshot_at(
        &self,
        descriptor: &ProjectionStoreDescriptor,
        generation_path: &Path,
    ) -> Result<ProjectionSnapshot, VectorProjectionBackendError> {
        require_regular_file(&generation_path.join(SNAPSHOT_FILE))?;
        let snapshot: ProjectionSnapshot = serde_json::from_slice(
            &fs::read(generation_path.join(SNAPSHOT_FILE)).map_err(backend_io)?,
        )
        .map_err(backend_json)?;
        self.validate_persisted_snapshot(&snapshot, descriptor)?;
        self.validate_snapshot_path_identity(&snapshot, generation_path)?;
        Ok(snapshot)
    }

    fn read_historical_snapshot_at(
        &self,
        store_name: &str,
        generation_path: &Path,
    ) -> Result<ProjectionSnapshot, VectorProjectionBackendError> {
        require_regular_file(&generation_path.join(SNAPSHOT_FILE))?;
        let snapshot: ProjectionSnapshot = serde_json::from_slice(
            &fs::read(generation_path.join(SNAPSHOT_FILE)).map_err(backend_io)?,
        )
        .map_err(backend_json)?;
        self.validate_persisted_snapshot_historical(&snapshot, store_name)?;
        self.validate_snapshot_path_identity(&snapshot, generation_path)?;
        Ok(snapshot)
    }

    fn validate_snapshot_path_identity(
        &self,
        snapshot: &ProjectionSnapshot,
        generation_path: &Path,
    ) -> Result<(), VectorProjectionBackendError> {
        if snapshot.manifest.generation != generation_path_id(generation_path)? {
            return Err(VectorProjectionBackendError::Backend(
                "generation directory name does not match its persisted snapshot".to_owned(),
            ));
        }
        Ok(())
    }

    fn generation_store(
        &self,
        descriptor: &ProjectionStoreDescriptor,
        generation_path: &Path,
        allow_create: bool,
    ) -> Result<GuardedLanceDbStore, VectorProjectionBackendError> {
        let expected = self.expected_embedding_cache(descriptor, generation_path)?;
        let cache_path = generation_path.join(EMBEDDING_CACHE_FILE);
        let cache_key_prefix = embedding_cache_key_prefix(&expected);
        let state = match fs::symlink_metadata(&cache_path) {
            Ok(metadata) if metadata.is_file() => {
                let stored: EmbeddingCacheFile =
                    serde_json::from_slice(&fs::read(&cache_path).map_err(backend_io)?)
                        .map_err(backend_json)?;
                require_embedding_cache_binding(&stored, &expected)?;
                stored
            }
            Ok(_) => {
                return Err(VectorProjectionBackendError::Backend(format!(
                    "embedding cache path is not a regular file: {}",
                    cache_path.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && allow_create => {
                persist_json(&cache_path, &expected).map_err(backend_io)?;
                expected
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(VectorProjectionBackendError::Backend(format!(
                    "embedding cache is missing: {}",
                    cache_path.display()
                )));
            }
            Err(error) => return Err(backend_io(error)),
        };
        let provider: Arc<dyn EmbeddingProvider + Send + Sync> =
            Arc::new(PersistentEmbeddingProvider {
                inner: Arc::clone(&self.provider),
                path: cache_path,
                cache_key_prefix,
                state: Mutex::new(state),
            });
        let lance_path = generation_path.join(LANCE_DATA_DIR);
        match fs::symlink_metadata(&lance_path) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(VectorProjectionBackendError::Backend(format!(
                    "LanceDB generation root is not a real directory: {}",
                    lance_path.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && allow_create => {
                durable_create_dir_all(&lance_path).map_err(backend_io)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(VectorProjectionBackendError::Backend(format!(
                    "LanceDB generation root is missing: {}",
                    lance_path.display()
                )));
            }
            Err(error) => return Err(backend_io(error)),
        }
        let generation_guard = DirectoryIdentityGuard::acquire(&lance_path).map_err(backend_io)?;
        let table_guard = if allow_create {
            None
        } else {
            let table_path = lance_path.join(lance_table_directory_name(&descriptor.store_name)?);
            require_real_directory(&table_path, "LanceDB generation table")?;
            Some(DirectoryIdentityGuard::acquire(&table_path).map_err(backend_io)?)
        };
        generation_guard
            .validate_path_identity()
            .map_err(backend_io)?;
        if let Some(table_guard) = &table_guard {
            table_guard.validate_path_identity().map_err(backend_io)?;
        }
        let config = LanceDbConfig::new(generation_guard.canonical_path(), provider)
            .with_execution_policy(self.execution_policy.clone());
        let store = LanceDbStore::connect(config).map_err(map_vector_error)?;
        generation_guard
            .validate_path_identity()
            .map_err(backend_io)?;
        if let Some(table_guard) = &table_guard {
            table_guard.validate_path_identity().map_err(backend_io)?;
        }
        Ok(GuardedLanceDbStore {
            store,
            generation_guard,
            table_guard,
        })
    }

    fn expected_embedding_cache(
        &self,
        descriptor: &ProjectionStoreDescriptor,
        generation_path: &Path,
    ) -> Result<EmbeddingCacheFile, VectorProjectionBackendError> {
        let corpus = descriptor.corpus.as_ref().ok_or_else(|| {
            VectorProjectionBackendError::Protocol(
                "projection store is missing its corpus binding".to_owned(),
            )
        })?;
        let generation_id = generation_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                VectorProjectionBackendError::Backend(
                    "generation path has no UTF-8 generation id".to_owned(),
                )
            })?;
        Ok(EmbeddingCacheFile {
            format_version: 1,
            database_instance_id: self.database_instance_id.clone(),
            store_name: descriptor.store_name.clone(),
            generation_id: generation_id.to_owned(),
            provider_fingerprint: descriptor.provider_fingerprint.clone(),
            corpus_fingerprint: corpus.corpus_fingerprint.clone(),
            embedding_model: corpus.embedding_model.clone(),
            embedding_dimensions: corpus.embedding_dimensions,
            entries: BTreeMap::new(),
        })
    }

    fn current_inspection_store(
        &self,
        descriptor: &ProjectionStoreDescriptor,
        generation_path: &Path,
    ) -> Result<GuardedProjectionReader, VectorProjectionBackendError> {
        let expected = self.expected_embedding_cache(descriptor, generation_path)?;
        let cache_path = generation_path.join(EMBEDDING_CACHE_FILE);
        require_regular_file(&cache_path)?;
        let stored: EmbeddingCacheFile =
            serde_json::from_slice(&fs::read(cache_path).map_err(backend_io)?)
                .map_err(backend_json)?;
        require_embedding_cache_binding(&stored, &expected)?;

        let corpus = descriptor.corpus.as_ref().ok_or_else(|| {
            VectorProjectionBackendError::Protocol(
                "projection store is missing its corpus binding".to_owned(),
            )
        })?;
        let lance_path = generation_path.join(LANCE_DATA_DIR);
        require_real_directory(&lance_path, "LanceDB generation root")?;
        let generation_guard = DirectoryIdentityGuard::acquire(&lance_path).map_err(backend_io)?;
        let table_path = lance_path.join(lance_table_directory_name(&descriptor.store_name)?);
        require_real_directory(&table_path, "LanceDB generation table")?;
        let table_guard = DirectoryIdentityGuard::acquire(&table_path).map_err(backend_io)?;
        generation_guard
            .validate_path_identity()
            .map_err(backend_io)?;
        table_guard.validate_path_identity().map_err(backend_io)?;
        let reader = LanceDbProjectionReader::open_existing(
            generation_guard.canonical_path(),
            corpus.embedding_dimensions,
        )
        .map_err(map_vector_error)?;
        generation_guard
            .validate_path_identity()
            .map_err(backend_io)?;
        table_guard.validate_path_identity().map_err(backend_io)?;
        Ok(GuardedProjectionReader {
            reader,
            generation_guard,
            table_guard,
        })
    }

    fn historical_inspection_store(
        &self,
        evidence: &ProjectionArtifactEvidence,
        generation_path: &Path,
    ) -> Result<GuardedProjectionReader, VectorProjectionBackendError> {
        self.validate_evidence_historical(evidence)?;
        let corpus = evidence.manifest.corpus.as_ref().ok_or_else(|| {
            VectorProjectionBackendError::Delivery(
                "historical evidence is missing its corpus binding".to_owned(),
            )
        })?;
        let lance_path = generation_path.join(LANCE_DATA_DIR);
        require_real_directory(&lance_path, "LanceDB generation root")?;
        let generation_guard = DirectoryIdentityGuard::acquire(&lance_path).map_err(backend_io)?;
        let table_path =
            lance_path.join(lance_table_directory_name(&evidence.manifest.store_name)?);
        require_real_directory(&table_path, "LanceDB historical generation table")?;
        let table_guard = DirectoryIdentityGuard::acquire(&table_path).map_err(backend_io)?;
        generation_guard
            .validate_path_identity()
            .map_err(backend_io)?;
        table_guard.validate_path_identity().map_err(backend_io)?;
        let reader = LanceDbProjectionReader::open_existing(
            generation_guard.canonical_path(),
            corpus.embedding_dimensions,
        )
        .map_err(map_vector_error)?;
        generation_guard
            .validate_path_identity()
            .map_err(backend_io)?;
        table_guard.validate_path_identity().map_err(backend_io)?;
        Ok(GuardedProjectionReader {
            reader,
            generation_guard,
            table_guard,
        })
    }

    fn hydrate_snapshot(
        &self,
        descriptor: &ProjectionStoreDescriptor,
        generation_path: &Path,
        snapshot: &ProjectionSnapshot,
    ) -> Result<(), VectorProjectionBackendError> {
        let store = self.generation_store(descriptor, generation_path, true)?;
        store.validate_path_identity()?;
        let result = match descriptor.store_name.as_str() {
            LANCEDB_CHUNKS_STORE => {
                let (boards, chunks) = self.task_chunks_from_snapshot(snapshot)?;
                for board_id in boards {
                    store.delete_board(&board_id).map_err(map_vector_error)?;
                }
                store.upsert(&chunks).map_err(map_vector_error)
            }
            LANCEDB_LABEL_ATOMS_STORE => {
                store
                    .ensure_label_atom_projection_table()
                    .map_err(map_vector_error)?;
                let (boards, atoms) = self.label_atoms_from_snapshot(snapshot)?;
                for board_id in boards {
                    store
                        .delete_label_atoms_for_board(&board_id)
                        .map_err(map_vector_error)?;
                }
                store.upsert_label_atoms(&atoms).map_err(map_vector_error)
            }
            _ => Err(VectorProjectionBackendError::Protocol(format!(
                "unsupported LanceDB projection store: {}",
                descriptor.store_name
            ))),
        };
        result?;
        store.validate_path_identity()
    }

    fn task_chunks_from_snapshot(
        &self,
        snapshot: &ProjectionSnapshot,
    ) -> Result<(BTreeSet<String>, Vec<kanban_vector::EmbeddingChunk>), VectorProjectionBackendError>
    {
        let mut boards = BTreeSet::new();
        let mut chunks = Vec::new();
        let builder = ChunkBuilder::new(self.provider.embedding_model());
        for record in &snapshot.records {
            let payload: TaskSnapshotPayload =
                serde_json::from_str(&record.payload_json).map_err(delivery_json)?;
            if record.identity != format!("kb://task/{}", payload.task_id)
                || record.board_id != payload.board_id
            {
                return Err(VectorProjectionBackendError::Delivery(
                    "task snapshot identity or board does not match its payload".to_owned(),
                ));
            }
            boards.insert(payload.board_id.clone());
            if payload.status == "archived" {
                continue;
            }
            let source = TaskChunkSource {
                task_uri: record.identity.clone(),
                project_id: None,
                board_id: Some(payload.board_id),
                task_id: payload.task_id,
                title: payload.title,
                description: payload.description,
                comments: payload.comments,
                run_text: payload.run_text,
                event_text: payload.event_text,
                source_event_id: None,
                created_at: payload.created_at,
                updated_at: payload.updated_at,
            };
            chunks.extend(
                builder
                    .build_task_chunks(&source)
                    .map_err(map_vector_error)?,
            );
        }
        Ok((boards, chunks))
    }

    fn label_atoms_from_snapshot(
        &self,
        snapshot: &ProjectionSnapshot,
    ) -> Result<(BTreeSet<String>, Vec<LabelAtomVector>), VectorProjectionBackendError> {
        let mut boards = BTreeSet::new();
        let mut atoms = Vec::new();
        for record in &snapshot.records {
            let payload: LabelAtomSnapshotPayload =
                serde_json::from_str(&record.payload_json).map_err(delivery_json)?;
            if record.identity != format!("kb://label-atom/{}", payload.atom_id) {
                return Err(VectorProjectionBackendError::Delivery(
                    "label atom snapshot identity does not match its payload".to_owned(),
                ));
            }
            boards.insert(record.board_id.clone());
            atoms.push(LabelAtomVector {
                atom_id: payload.atom_id,
                label_id: payload.label_id,
                label_name: payload.label_name,
                board_id: record.board_id.clone(),
                polarity: payload.polarity,
                kind: payload.kind,
                text: payload.text,
                ordinal: payload.ordinal,
                content_hash: payload.content_hash,
                embedding_model: self.provider.embedding_model().to_owned(),
                created_at: payload.created_at,
                updated_at: payload.updated_at,
            });
        }
        Ok((boards, atoms))
    }

    fn validate_generation_materialization(
        &self,
        descriptor: &ProjectionStoreDescriptor,
        generation_path: &Path,
    ) -> Result<(), VectorProjectionBackendError> {
        let table_name = match descriptor.store_name.as_str() {
            LANCEDB_CHUNKS_STORE => "kb_chunks.lance",
            LANCEDB_LABEL_ATOMS_STORE => "kb_label_atoms.lance",
            _ => {
                return Err(VectorProjectionBackendError::Protocol(format!(
                    "unsupported LanceDB projection store: {}",
                    descriptor.store_name
                )));
            }
        };
        require_real_directory(
            &generation_path.join(LANCE_DATA_DIR).join(table_name),
            "LanceDB generation table",
        )?;
        let reader = self.current_inspection_store(descriptor, generation_path)?;
        reader.validate_path_identity()?;
        let validation = match descriptor.store_name.as_str() {
            LANCEDB_CHUNKS_STORE => reader
                .validate_chunk_projection_table()
                .map_err(map_vector_error),
            LANCEDB_LABEL_ATOMS_STORE => reader
                .validate_label_atom_projection_table()
                .map_err(map_vector_error),
            _ => unreachable!("projection store was validated above"),
        };
        reader.validate_path_identity()?;
        validation
    }

    fn validate_historical_materialization(
        &self,
        evidence: &ProjectionArtifactEvidence,
        generation_path: &Path,
    ) -> Result<(), VectorProjectionBackendError> {
        let table_name = match evidence.manifest.store_name.as_str() {
            LANCEDB_CHUNKS_STORE => "kb_chunks.lance",
            LANCEDB_LABEL_ATOMS_STORE => "kb_label_atoms.lance",
            store_name => {
                return Err(VectorProjectionBackendError::Protocol(format!(
                    "unsupported LanceDB projection store: {store_name}"
                )));
            }
        };
        require_real_directory(
            &generation_path.join(LANCE_DATA_DIR).join(table_name),
            "LanceDB historical generation table",
        )?;
        let store = self.historical_inspection_store(evidence, generation_path)?;
        store.validate_path_identity()?;
        let validation = match evidence.manifest.store_name.as_str() {
            LANCEDB_CHUNKS_STORE => store
                .validate_chunk_projection_table()
                .map_err(map_vector_error),
            LANCEDB_LABEL_ATOMS_STORE => store
                .validate_label_atom_projection_table()
                .map_err(map_vector_error),
            _ => unreachable!("historical store was validated above"),
        };
        store.validate_path_identity()?;
        validation
    }

    fn actual_content_rows(
        &self,
        descriptor: &ProjectionStoreDescriptor,
        generation_path: &Path,
    ) -> Result<Vec<ProjectionContentRow>, VectorProjectionBackendError> {
        let reader = self.current_inspection_store(descriptor, generation_path)?;
        reader.validate_path_identity()?;
        let rows = match descriptor.store_name.as_str() {
            LANCEDB_CHUNKS_STORE => reader
                .chunk_projection_content_rows()
                .map_err(map_vector_error),
            LANCEDB_LABEL_ATOMS_STORE => reader
                .label_atom_projection_content_rows()
                .map_err(map_vector_error),
            _ => Err(VectorProjectionBackendError::Protocol(format!(
                "unsupported LanceDB projection store: {}",
                descriptor.store_name
            ))),
        };
        reader.validate_path_identity()?;
        rows
    }

    fn historical_content_rows(
        &self,
        evidence: &ProjectionArtifactEvidence,
        generation_path: &Path,
    ) -> Result<Vec<ProjectionContentRow>, VectorProjectionBackendError> {
        let store = self.historical_inspection_store(evidence, generation_path)?;
        store.validate_path_identity()?;
        let rows = match evidence.manifest.store_name.as_str() {
            LANCEDB_CHUNKS_STORE => store
                .chunk_projection_content_rows()
                .map_err(map_vector_error),
            LANCEDB_LABEL_ATOMS_STORE => store
                .label_atom_projection_content_rows()
                .map_err(map_vector_error),
            store_name => Err(VectorProjectionBackendError::Protocol(format!(
                "unsupported LanceDB projection store: {store_name}"
            ))),
        };
        store.validate_path_identity()?;
        rows
    }

    fn expected_snapshot_content_rows(
        &self,
        descriptor: &ProjectionStoreDescriptor,
        snapshot: &ProjectionSnapshot,
    ) -> Result<Vec<ProjectionContentRow>, VectorProjectionBackendError> {
        match descriptor.store_name.as_str() {
            LANCEDB_CHUNKS_STORE => {
                let (_, chunks) = self.task_chunks_from_snapshot(snapshot)?;
                let rows =
                    expected_chunk_projection_content_rows(&chunks).map_err(map_vector_error)?;
                self.bind_expected_vectors(rows)
            }
            LANCEDB_LABEL_ATOMS_STORE => {
                let (_, atoms) = self.label_atoms_from_snapshot(snapshot)?;
                let rows = expected_label_atom_projection_content_rows(&atoms)
                    .map_err(map_vector_error)?;
                self.bind_expected_vectors(rows)
            }
            _ => Err(VectorProjectionBackendError::Protocol(format!(
                "unsupported LanceDB projection store: {}",
                descriptor.store_name
            ))),
        }
    }

    fn expected_canonical_content_rows_from(
        &self,
        conn: &Connection,
        descriptor: &ProjectionStoreDescriptor,
    ) -> Result<Vec<ProjectionContentRow>, VectorProjectionBackendError> {
        match descriptor.store_name.as_str() {
            LANCEDB_CHUNKS_STORE => {
                let builder = ChunkBuilder::new(self.provider.embedding_model());
                let mut chunks = Vec::new();
                for source in all_task_sources(conn)? {
                    chunks.extend(
                        builder
                            .build_task_chunks(&source)
                            .map_err(map_vector_error)?,
                    );
                }
                let rows =
                    expected_chunk_projection_content_rows(&chunks).map_err(map_vector_error)?;
                self.bind_expected_vectors(rows)
            }
            LANCEDB_LABEL_ATOMS_STORE => {
                let atoms = all_label_atoms(conn, self.provider.embedding_model())?;
                let rows = expected_label_atom_projection_content_rows(&atoms)
                    .map_err(map_vector_error)?;
                self.bind_expected_vectors(rows)
            }
            _ => Err(VectorProjectionBackendError::Protocol(format!(
                "unsupported LanceDB projection store: {}",
                descriptor.store_name
            ))),
        }
    }

    fn bind_expected_vectors(
        &self,
        mut rows: Vec<ProjectionContentRow>,
    ) -> Result<Vec<ProjectionContentRow>, VectorProjectionBackendError> {
        let texts = rows
            .iter()
            .map(|row| {
                serde_json::from_str::<serde_json::Value>(&row.content_json)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("text")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_owned)
                    })
                    .ok_or_else(|| {
                        VectorProjectionBackendError::Backend(
                            "canonical projection row is missing text evidence".to_owned(),
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let vectors = self
            .provider
            .embed_batch(&texts)
            .map_err(map_vector_error)?;
        if vectors.len() != rows.len() {
            return Err(VectorProjectionBackendError::Backend(
                "embedding provider returned an unexpected row count".to_owned(),
            ));
        }
        for (row, vector) in rows.iter_mut().zip(vectors) {
            ensure_dimensions(&vector, self.provider.dimensions()).map_err(map_vector_error)?;
            if vector.iter().any(|value| !value.is_finite()) {
                return Err(VectorProjectionBackendError::Backend(
                    "embedding provider returned a non-finite vector".to_owned(),
                ));
            }
            row.vector_bits = Some(vector.into_iter().map(f32::to_bits).collect());
        }
        Ok(rows)
    }

    fn validate_snapshot_materialization(
        &self,
        descriptor: &ProjectionStoreDescriptor,
        generation_path: &Path,
        snapshot: &ProjectionSnapshot,
    ) -> Result<(), VectorProjectionBackendError> {
        let expected = self.expected_snapshot_content_rows(descriptor, snapshot)?;
        let actual = self.actual_content_rows(descriptor, generation_path)?;
        if !same_projection_content_rows(&actual, &expected) {
            return Err(VectorProjectionBackendError::Backend(
                "materialized LanceDB row set does not match the prepared canonical snapshot"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    fn write_content_metadata(
        &self,
        descriptor: &ProjectionStoreDescriptor,
        generation_path: &Path,
        evidence: &ProjectionArtifactEvidence,
    ) -> Result<(), VectorProjectionBackendError> {
        let rows = self.actual_content_rows(descriptor, generation_path)?;
        let metadata = self.content_metadata(evidence, &rows)?;
        persist_json(&generation_path.join(CONTENT_METADATA_FILE), &metadata).map_err(backend_io)
    }

    fn validate_content_metadata(
        &self,
        generation_path: &Path,
        evidence: &ProjectionArtifactEvidence,
    ) -> Result<(), VectorProjectionBackendError> {
        let path = generation_path.join(CONTENT_METADATA_FILE);
        require_regular_file(&path)?;
        let stored: ProjectionContentMetadata =
            serde_json::from_slice(&fs::read(path).map_err(backend_io)?).map_err(backend_json)?;
        let rows = self.historical_content_rows(evidence, generation_path)?;
        let expected = self.content_metadata(evidence, &rows)?;
        if stored != expected {
            return Err(VectorProjectionBackendError::Backend(
                "LanceDB physical row fingerprint does not match generation evidence".to_owned(),
            ));
        }
        Ok(())
    }

    fn validate_historical_auxiliary_state(
        &self,
        generation_path: &Path,
        evidence: &ProjectionArtifactEvidence,
    ) -> Result<(), VectorProjectionBackendError> {
        self.validate_evidence_historical(evidence)?;
        let corpus = evidence.manifest.corpus.as_ref().ok_or_else(|| {
            VectorProjectionBackendError::Delivery(
                "projection evidence is missing its corpus binding".to_owned(),
            )
        })?;
        let expected_cache = EmbeddingCacheFile {
            format_version: 1,
            database_instance_id: evidence.manifest.database_instance_id.clone(),
            store_name: evidence.manifest.store_name.clone(),
            generation_id: evidence.manifest.generation.clone(),
            provider_fingerprint: evidence.manifest.provider_fingerprint.clone(),
            corpus_fingerprint: corpus.corpus_fingerprint.clone(),
            embedding_model: corpus.embedding_model.clone(),
            embedding_dimensions: corpus.embedding_dimensions,
            entries: BTreeMap::new(),
        };
        let cache_path = generation_path.join(EMBEDDING_CACHE_FILE);
        require_regular_file(&cache_path)?;
        let stored_cache: EmbeddingCacheFile =
            serde_json::from_slice(&fs::read(cache_path).map_err(backend_io)?)
                .map_err(backend_json)?;
        require_embedding_cache_binding(&stored_cache, &expected_cache)?;

        let state_path = generation_path.join(DELIVERY_STATE_FILE);
        match fs::symlink_metadata(&state_path) {
            Ok(metadata) if metadata.is_file() => {
                let stored_state: DeliveryStateFile =
                    serde_json::from_slice(&fs::read(state_path).map_err(backend_io)?)
                        .map_err(backend_json)?;
                let expected_state = DeliveryStateFile {
                    format_version: 1,
                    database_instance_id: evidence.manifest.database_instance_id.clone(),
                    store_name: evidence.manifest.store_name.clone(),
                    generation_id: evidence.manifest.generation.clone(),
                    provider_fingerprint: evidence.manifest.provider_fingerprint.clone(),
                    corpus_fingerprint: corpus.corpus_fingerprint.clone(),
                    evidence_fingerprint: evidence.fingerprint.clone(),
                    applied: BTreeMap::new(),
                };
                require_delivery_state_binding(&stored_state, &expected_state)?;
            }
            Ok(_) => {
                return Err(VectorProjectionBackendError::Backend(format!(
                    "delivery state path is not a regular file: {}",
                    state_path.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(backend_io(error)),
        }
        Ok(())
    }

    fn content_metadata(
        &self,
        evidence: &ProjectionArtifactEvidence,
        rows: &[ProjectionContentRow],
    ) -> Result<ProjectionContentMetadata, VectorProjectionBackendError> {
        self.validate_evidence_historical(evidence)?;
        let corpus = evidence.manifest.corpus.as_ref().ok_or_else(|| {
            VectorProjectionBackendError::Delivery(
                "projection evidence is missing its corpus binding".to_owned(),
            )
        })?;
        Ok(ProjectionContentMetadata {
            format_version: 1,
            database_instance_id: self.database_instance_id.clone(),
            store_name: evidence.manifest.store_name.clone(),
            generation_id: evidence.manifest.generation.clone(),
            provider_fingerprint: evidence.manifest.provider_fingerprint.clone(),
            corpus_fingerprint: corpus.corpus_fingerprint.clone(),
            evidence_fingerprint: evidence.fingerprint.clone(),
            row_count: rows.len(),
            content_fingerprint: projection_content_fingerprint(rows),
        })
    }

    fn validate_canonical_content(
        &self,
        descriptor: &ProjectionStoreDescriptor,
        generation_path: &Path,
        evidence: &ProjectionArtifactEvidence,
    ) -> Result<(), VectorProjectionBackendError> {
        let conn = open_readonly_database(&self.db_path)?;
        self.validate_canonical_content_from(&conn, descriptor, generation_path, evidence)
    }

    fn validate_canonical_content_from(
        &self,
        conn: &Connection,
        descriptor: &ProjectionStoreDescriptor,
        generation_path: &Path,
        evidence: &ProjectionArtifactEvidence,
    ) -> Result<(), VectorProjectionBackendError> {
        self.validate_content_metadata(generation_path, evidence)?;
        let actual = self.historical_content_rows(evidence, generation_path)?;
        let canonical = self.expected_canonical_content_rows_from(conn, descriptor)?;
        if !same_projection_content_rows(&actual, &canonical) {
            return Err(VectorProjectionBackendError::Delivery(
                "LanceDB physical row set does not match canonical SQLite truth".to_owned(),
            ));
        }
        Ok(())
    }

    fn apply_batch(
        &self,
        request: &VectorProjectionApplyBatchRequest,
    ) -> Result<VectorProjectionBatchApplicationReceipt, VectorProjectionBackendError> {
        self.validate_context(&request.context)?;
        let descriptor = self.require_store(&request.context.projection_store)?;
        let evidence = self
            .inspect_generation(
                &request.context.projection_store,
                &request.context.generation_id,
            )?
            .ok_or_else(|| {
                VectorProjectionBackendError::Delivery(
                    "apply target generation is not prepared".to_owned(),
                )
            })?;
        self.validate_batch_binding(&request.context, &request.batch, descriptor, &evidence)?;
        let generations = self.generations_root(&descriptor.store_name, false)?;
        let generation_path =
            checked_generation_path(&generations, &request.context.generation_id)?;
        self.validate_generation_materialization(descriptor, &generation_path)?;

        let mut state = self.load_delivery_state(descriptor, &generation_path, &evidence)?;
        let mut pending = Vec::new();
        let mut signatures = BTreeMap::new();
        for delivery in &request.batch.items {
            let key = delivery.id.to_string();
            let signature = delivery_signature(delivery);
            match state.applied.get(&key) {
                Some(stored) if stored == &signature => {}
                Some(_) => {
                    return Err(VectorProjectionBackendError::Delivery(format!(
                        "delivery {} was replayed with different immutable contents",
                        delivery.id
                    )));
                }
                None => pending.push(delivery),
            }
            signatures.insert(key, signature);
        }

        if !pending.is_empty() {
            match descriptor.store_name.as_str() {
                LANCEDB_CHUNKS_STORE => {
                    self.apply_task_deliveries(descriptor, &generation_path, &pending)?
                }
                LANCEDB_LABEL_ATOMS_STORE => {
                    self.apply_label_deliveries(descriptor, &generation_path, &pending)?
                }
                _ => {
                    return Err(VectorProjectionBackendError::Protocol(format!(
                        "unsupported LanceDB projection store: {}",
                        descriptor.store_name
                    )));
                }
            }
            self.write_content_metadata(descriptor, &generation_path, &evidence)?;
            state.applied.extend(signatures);
            persist_json(&generation_path.join(DELIVERY_STATE_FILE), &state).map_err(backend_io)?;
        } else {
            self.validate_content_metadata(&generation_path, &evidence)?;
        }

        Ok(VectorProjectionBatchApplicationReceipt {
            store_name: request.batch.store_name.clone(),
            database_instance_id: request.batch.database_instance_id.clone(),
            protocol_version: request.batch.protocol_version,
            schema_version: request.batch.schema_version,
            provider: request.batch.provider.clone(),
            provider_fingerprint: request.batch.provider_fingerprint.clone(),
            target_generation: request.batch.target_generation.clone(),
            fence_epoch: request.batch.fence_epoch,
            applied_item_count: request.batch.items.len(),
        })
    }

    fn validate_batch_binding(
        &self,
        context: &VectorProjectionMutationContext,
        batch: &ProjectionBatch,
        descriptor: &ProjectionStoreDescriptor,
        evidence: &ProjectionArtifactEvidence,
    ) -> Result<(), VectorProjectionBackendError> {
        self.validate_evidence_current(evidence)?;
        if context.projection_store != batch.store_name
            || context.generation_id != batch.target_generation
            || context.delivery_digest != evidence.manifest.delivery_digest
            || batch.store_name != descriptor.store_name
            || batch.database_instance_id != self.database_instance_id
            || batch.protocol_version != VECTOR_PROJECTION_PROTOCOL_VERSION
            || batch.schema_version != descriptor.schema_version
            || batch.provider != descriptor.provider
            || batch.provider_fingerprint != descriptor.provider_fingerprint
            || batch.target_generation != evidence.manifest.generation
            || batch.fence_epoch != evidence.manifest.fence_epoch
            || batch.fence_epoch < 0
            || batch.owner.trim().is_empty()
            || batch.lease_token.trim().is_empty()
            || batch.claim_token.trim().is_empty()
            || batch.claim_expires_at <= current_time_ms()?
        {
            return Err(VectorProjectionBackendError::Delivery(
                "projection batch does not match its prepared generation/provider/lease binding"
                    .to_owned(),
            ));
        }
        let mut ids = BTreeSet::new();
        for item in &batch.items {
            if item.id <= 0
                || item.outbox_id <= 0
                || !ids.insert(item.id)
                || item.store_name != batch.store_name
                || item.generation_id != batch.target_generation
                || item.board_id.trim().is_empty()
                || item.source_event_id.is_some_and(|id| id <= 0)
                || item.cursor <= evidence.manifest.snapshot_cursor
                || !item.entity_uri.starts_with("kb://")
                || serde_json::from_str::<serde_json::Value>(&item.payload_json).is_err()
                || item.attempts < 0
            {
                return Err(VectorProjectionBackendError::Delivery(format!(
                    "projection delivery {} is invalid or outside the generation fence",
                    item.id
                )));
            }
        }
        Ok(())
    }

    fn load_delivery_state(
        &self,
        descriptor: &ProjectionStoreDescriptor,
        generation_path: &Path,
        evidence: &ProjectionArtifactEvidence,
    ) -> Result<DeliveryStateFile, VectorProjectionBackendError> {
        let corpus = descriptor.corpus.as_ref().ok_or_else(|| {
            VectorProjectionBackendError::Protocol(
                "projection store is missing its corpus binding".to_owned(),
            )
        })?;
        let expected = DeliveryStateFile {
            format_version: 1,
            database_instance_id: self.database_instance_id.clone(),
            store_name: descriptor.store_name.clone(),
            generation_id: evidence.manifest.generation.clone(),
            provider_fingerprint: descriptor.provider_fingerprint.clone(),
            corpus_fingerprint: corpus.corpus_fingerprint.clone(),
            evidence_fingerprint: evidence.fingerprint.clone(),
            applied: BTreeMap::new(),
        };
        let path = generation_path.join(DELIVERY_STATE_FILE);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() => {
                let stored: DeliveryStateFile =
                    serde_json::from_slice(&fs::read(path).map_err(backend_io)?)
                        .map_err(backend_json)?;
                require_delivery_state_binding(&stored, &expected)?;
                Ok(stored)
            }
            Ok(_) => Err(VectorProjectionBackendError::Backend(format!(
                "delivery state path is not a regular file: {}",
                path.display()
            ))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(expected),
            Err(error) => Err(backend_io(error)),
        }
    }

    fn apply_task_deliveries(
        &self,
        descriptor: &ProjectionStoreDescriptor,
        generation_path: &Path,
        deliveries: &[&ProjectionDelivery],
    ) -> Result<(), VectorProjectionBackendError> {
        let conn = open_readonly_database(&self.db_path)?;
        let store = self.generation_store(descriptor, generation_path, false)?;
        store.validate_path_identity()?;
        let mut ordered = deliveries.to_vec();
        ordered.sort_by_key(|delivery| (delivery.cursor, delivery.id));
        let mut rebuild_boards = BTreeMap::<String, (i64, i64)>::new();
        let mut task_actions =
            BTreeMap::<(String, String), ((i64, i64), String, ProjectionDeliveryAction)>::new();
        for delivery in ordered {
            let board_uri = format!("kb://board/{}", delivery.board_id);
            if delivery.entity_uri == board_uri {
                match delivery.action {
                    ProjectionDeliveryAction::Rebuild => {
                        require_delivery_source_event_board(&conn, delivery)?;
                        rebuild_boards
                            .insert(delivery.board_id.clone(), (delivery.cursor, delivery.id));
                    }
                    ProjectionDeliveryAction::Upsert => {
                        require_taskless_board_upsert_source(&conn, delivery)?;
                    }
                    ProjectionDeliveryAction::Delete => {
                        return Err(VectorProjectionBackendError::Delivery(format!(
                            "task delivery {} board delete action/entity correlation is invalid",
                            delivery.id
                        )));
                    }
                }
                continue;
            }
            if delivery.action == ProjectionDeliveryAction::Rebuild {
                return Err(VectorProjectionBackendError::Delivery(format!(
                    "task delivery {} board rebuild action/entity correlation is invalid",
                    delivery.id
                )));
            }
            let task_id = self.delivery_task_id(&conn, delivery)?;
            task_actions.insert(
                (delivery.board_id.clone(), task_id.clone()),
                (
                    (delivery.cursor, delivery.id),
                    format!("kb://task/{task_id}"),
                    delivery.action,
                ),
            );
        }
        task_actions.retain(|(board_id, _), (order, _, _)| {
            rebuild_boards
                .get(board_id)
                .is_none_or(|rebuild_order| *order > *rebuild_order)
        });

        let mut chunks = Vec::new();
        let builder = ChunkBuilder::new(self.provider.embedding_model());
        for board_id in rebuild_boards.keys() {
            store.delete_board(board_id).map_err(map_vector_error)?;
            for source in task_sources_for_board(&conn, board_id)? {
                chunks.extend(
                    builder
                        .build_task_chunks(&source)
                        .map_err(map_vector_error)?,
                );
            }
        }

        let entity_uris = task_actions
            .values()
            .map(|(_, entity_uri, _)| entity_uri.clone())
            .collect::<Vec<_>>();
        store
            .delete_entities(&entity_uris)
            .map_err(map_vector_error)?;
        for ((board_id, task_id), (_, _, _action)) in task_actions {
            if let Some(source) = task_source(&conn, &board_id, &task_id)? {
                chunks.extend(
                    builder
                        .build_task_chunks(&source)
                        .map_err(map_vector_error)?,
                );
            }
        }
        store.upsert(&chunks).map_err(map_vector_error)?;
        store.validate_path_identity()
    }

    fn delivery_task_id(
        &self,
        conn: &Connection,
        delivery: &ProjectionDelivery,
    ) -> Result<String, VectorProjectionBackendError> {
        let event_task = match delivery.source_event_id {
            Some(source_event_id) => conn
                .query_row(
                    "SELECT COALESCE(e.task_id,r.task_id)
                     FROM task_events e
                     LEFT JOIN task_runs r ON r.board_id=e.board_id AND r.id=e.run_id
                     WHERE e.id=?1 AND e.board_id=?2",
                    params![source_event_id, delivery.board_id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()
                .map_err(backend_sql)?
                .ok_or_else(|| {
                    VectorProjectionBackendError::Delivery(format!(
                        "delivery {} source event is missing or belongs to another board",
                        delivery.id
                    ))
                })?,
            None => None,
        };
        if let Some(task_id) = delivery
            .entity_uri
            .strip_prefix("kb://task/")
            .filter(|task_id| !task_id.is_empty() && !task_id.contains('/'))
        {
            let canonical_board = conn
                .query_row("SELECT board_id FROM tasks WHERE id=?1", [task_id], |row| {
                    row.get::<_, String>(0)
                })
                .optional()
                .map_err(backend_sql)?;
            if canonical_board
                .as_deref()
                .is_some_and(|board_id| board_id != delivery.board_id)
                || (canonical_board.is_none() && event_task.as_deref() != Some(task_id))
            {
                return Err(VectorProjectionBackendError::Delivery(format!(
                    "delivery {} task cannot be proven to belong to its board",
                    delivery.id
                )));
            }
            if event_task
                .as_deref()
                .is_some_and(|event_task| event_task != task_id)
            {
                return Err(VectorProjectionBackendError::Delivery(format!(
                    "delivery {} entity does not match its source event",
                    delivery.id
                )));
            }
            return Ok(task_id.to_owned());
        }
        event_task.ok_or_else(|| {
            VectorProjectionBackendError::Delivery(format!(
                "delivery {} cannot be mapped to a board-scoped task",
                delivery.id
            ))
        })
    }

    fn apply_label_deliveries(
        &self,
        descriptor: &ProjectionStoreDescriptor,
        generation_path: &Path,
        deliveries: &[&ProjectionDelivery],
    ) -> Result<(), VectorProjectionBackendError> {
        let conn = open_readonly_database(&self.db_path)?;
        let store = self.generation_store(descriptor, generation_path, false)?;
        store.validate_path_identity()?;
        store
            .ensure_label_atom_projection_table()
            .map_err(map_vector_error)?;
        for delivery in deliveries {
            if delivery.action != ProjectionDeliveryAction::Rebuild
                || delivery.entity_uri != format!("kb://board/{}", delivery.board_id)
            {
                return Err(VectorProjectionBackendError::Delivery(format!(
                    "label delivery {} must be a board-scoped rebuild",
                    delivery.id
                )));
            }
            require_delivery_source_event_board(&conn, delivery)?;
        }
        let boards = deliveries
            .iter()
            .map(|delivery| delivery.board_id.clone())
            .collect::<BTreeSet<_>>();
        let mut atoms = Vec::new();
        for board_id in boards {
            store
                .delete_label_atoms_for_board(&board_id)
                .map_err(map_vector_error)?;
            atoms.extend(label_atoms_for_board(
                &conn,
                &board_id,
                self.provider.embedding_model(),
            )?);
        }
        store.upsert_label_atoms(&atoms).map_err(map_vector_error)?;
        store.validate_path_identity()
    }

    fn publish(
        &self,
        request: &VectorProjectionPublishRequest,
    ) -> Result<ProjectionPublishReceipt, VectorProjectionBackendError> {
        self.validate_context(&request.context)?;
        self.validate_evidence_current(&request.prepared)?;
        if request.context.projection_store != request.prepared.manifest.store_name
            || request.context.generation_id != request.prepared.manifest.generation
            || request.context.delivery_digest != request.prepared.manifest.delivery_digest
        {
            return Err(VectorProjectionBackendError::Delivery(
                "publish context does not match prepared evidence".to_owned(),
            ));
        }
        if let Some(expected) = &request.expected_active {
            self.validate_evidence_historical(expected)?;
            if expected.manifest.store_name != request.context.projection_store {
                return Err(VectorProjectionBackendError::Delivery(
                    "publish expected active belongs to another store".to_owned(),
                ));
            }
            if request.prepared.manifest.fence_epoch <= expected.manifest.fence_epoch {
                return Err(VectorProjectionBackendError::Delivery(
                    "published generation fence must advance past the previous generation"
                        .to_owned(),
                ));
            }
            if !self.marker_is_valid(expected)? {
                return Err(VectorProjectionBackendError::Delivery(
                    "publish expected active marker is missing or corrupt".to_owned(),
                ));
            }
            let previous_generations =
                self.generations_root(&expected.manifest.store_name, false)?;
            let previous_path =
                checked_generation_path(&previous_generations, &expected.manifest.generation)?;
            self.validate_historical_materialization(expected, &previous_path)?;
            self.validate_content_metadata(&previous_path, expected)?;
            self.validate_historical_auxiliary_state(&previous_path, expected)?;
        }
        let actual_active = self
            .published_generations_for_repair(&request.prepared)?
            .pop();
        if actual_active.as_ref() != request.expected_active.as_ref() {
            return Err(VectorProjectionBackendError::Delivery(
                "active generation changed before publish".to_owned(),
            ));
        }
        let stored = self
            .inspect_generation(
                &request.context.projection_store,
                &request.context.generation_id,
            )?
            .ok_or_else(|| {
                VectorProjectionBackendError::Delivery(
                    "prepared generation is missing before publish".to_owned(),
                )
            })?;
        if stored != request.prepared {
            return Err(VectorProjectionBackendError::Delivery(
                "prepared generation evidence changed before publish".to_owned(),
            ));
        }
        let generations = self.generations_root(&request.context.projection_store, false)?;
        let generation_path =
            checked_generation_path(&generations, &request.context.generation_id)?;
        self.validate_historical_materialization(&request.prepared, &generation_path)?;
        self.validate_content_metadata(&generation_path, &request.prepared)?;
        self.repair_marker(&request.prepared)?;
        let active = self
            .inspect_active(&request.context.projection_store)?
            .ok_or_else(|| {
                VectorProjectionBackendError::Backend(
                    "published generation is not discoverable".to_owned(),
                )
            })?;
        if active != request.prepared {
            return Err(VectorProjectionBackendError::Delivery(
                "a different generation won the publication fence".to_owned(),
            ));
        }
        if let Some(previous) = &request.expected_active {
            let retained = self
                .inspect_generation(&previous.manifest.store_name, &previous.manifest.generation)?;
            if retained.as_ref() != Some(previous) {
                return Err(VectorProjectionBackendError::Backend(
                    "previous generation was not retained after publication".to_owned(),
                ));
            }
        }
        Ok(ProjectionPublishReceipt {
            active,
            retained_previous: request.expected_active.clone(),
        })
    }

    fn validate_generation_publication(
        &self,
        request: &VectorProjectionValidateGenerationRequest,
    ) -> Result<VectorProjectionValidationResponse, VectorProjectionBackendError> {
        require_non_empty(&request.request_id, "request_id")?;
        self.validate_evidence_historical(&request.expected)?;
        let stored = self.inspect_generation(
            &request.projection_store,
            &request.expected.manifest.generation,
        )?;
        let materialized = stored.as_ref().is_some_and(|_| {
            self.generations_root(&request.projection_store, false)
                .and_then(|root| {
                    checked_generation_path(&root, &request.expected.manifest.generation)
                })
                .and_then(|path| {
                    self.validate_historical_materialization(&request.expected, &path)?;
                    self.validate_content_metadata(&path, &request.expected)?;
                    self.validate_historical_auxiliary_state(&path, &request.expected)
                })
                .is_ok()
        });
        let valid = stored.as_ref() == Some(&request.expected)
            && materialized
            && stored
                .as_ref()
                .is_some_and(|evidence| self.marker_is_valid(evidence).unwrap_or(false));
        Ok(VectorProjectionValidationResponse {
            request_id: request.request_id.clone(),
            projection_store: request.projection_store.clone(),
            valid,
        })
    }

    fn validate_active_contents(
        &self,
        request: &VectorProjectionValidateActiveRequest,
    ) -> Result<VectorProjectionValidationResponse, VectorProjectionBackendError> {
        require_non_empty(&request.request_id, "request_id")?;
        self.validate_evidence_historical(&request.active)?;
        let canonical = self
            .generations_root(&request.projection_store, false)
            .and_then(|root| checked_generation_path(&root, &request.active.manifest.generation))
            .and_then(|path| {
                let descriptor = self.require_store(&request.projection_store)?;
                self.validate_historical_materialization(&request.active, &path)?;
                self.validate_canonical_content(descriptor, &path, &request.active)?;
                self.validate_historical_auxiliary_state(&path, &request.active)
            })
            .is_ok();
        let valid = self.inspect_active(&request.projection_store)?.as_ref()
            == Some(&request.active)
            && canonical
            && self
                .inspect_generation(
                    &request.projection_store,
                    &request.active.manifest.generation,
                )?
                .as_ref()
                == Some(&request.active);
        Ok(VectorProjectionValidationResponse {
            request_id: request.request_id.clone(),
            projection_store: request.projection_store.clone(),
            valid,
        })
    }

    fn repair_publication(
        &self,
        request: &VectorProjectionRepairPublicationRequest,
    ) -> Result<(), VectorProjectionBackendError> {
        self.validate_context(&request.context)?;
        self.validate_evidence_current(&request.expected)?;
        if request.context.projection_store != request.expected.manifest.store_name
            || request.context.generation_id != request.expected.manifest.generation
            || request.context.delivery_digest != request.expected.manifest.delivery_digest
        {
            return Err(VectorProjectionBackendError::Delivery(
                "repair context does not match expected evidence".to_owned(),
            ));
        }
        let stored = self
            .inspect_generation(
                &request.context.projection_store,
                &request.context.generation_id,
            )?
            .ok_or_else(|| {
                VectorProjectionBackendError::Delivery(
                    "generation is missing during publication repair".to_owned(),
                )
            })?;
        if stored != request.expected {
            return Err(VectorProjectionBackendError::Delivery(
                "generation evidence changed before publication repair".to_owned(),
            ));
        }
        if let Some(active) = self
            .published_generations_for_repair(&request.expected)?
            .pop()
            && active != request.expected
            && active.manifest.fence_epoch >= request.expected.manifest.fence_epoch
        {
            return Err(VectorProjectionBackendError::Delivery(
                "publication repair cannot supersede an equal or newer active generation"
                    .to_owned(),
            ));
        }
        self.repair_marker(&request.expected)
    }

    fn quarantine(
        &self,
        request: &VectorProjectionGenerationMutationRequest,
    ) -> Result<(), VectorProjectionBackendError> {
        self.validate_context(&request.context)?;
        let generations = self.generations_root(&request.context.projection_store, false)?;
        let path = checked_generation_path(&generations, &request.context.generation_id)?;
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                if let Ok(Some(evidence)) = self.inspect_generation(
                    &request.context.projection_store,
                    &request.context.generation_id,
                ) && evidence.manifest.delivery_digest != request.context.delivery_digest
                {
                    return Err(VectorProjectionBackendError::Delivery(
                        "quarantine context delivery digest does not match generation evidence"
                            .to_owned(),
                    ));
                }
                durable_quarantine_entry(&path).map_err(backend_io)?;
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(backend_io(error)),
        }
    }

    fn abort(
        &self,
        request: &VectorProjectionGenerationMutationRequest,
    ) -> Result<(), VectorProjectionBackendError> {
        self.validate_context(&request.context)?;
        let generations = self.generations_root(&request.context.projection_store, false)?;
        let path = checked_generation_path(&generations, &request.context.generation_id)?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(backend_io(error)),
        };
        if !metadata.is_dir() {
            durable_quarantine_entry(&path).map_err(backend_io)?;
            return Ok(());
        }
        let snapshot_path = path.join(SNAPSHOT_FILE);
        require_regular_file(&snapshot_path)?;
        let snapshot: ProjectionSnapshot =
            serde_json::from_slice(&fs::read(snapshot_path).map_err(backend_io)?)
                .map_err(backend_json)?;
        self.validate_persisted_snapshot_historical(&snapshot, &request.context.projection_store)?;
        if snapshot.manifest.generation != request.context.generation_id
            || snapshot.manifest.delivery_digest != request.context.delivery_digest
        {
            return Err(VectorProjectionBackendError::Delivery(
                "abort context does not match the persisted generation snapshot".to_owned(),
            ));
        }
        if marker_exists(&path)? {
            return Err(VectorProjectionBackendError::Delivery(format!(
                "cannot abort published generation {}",
                request.context.generation_id
            )));
        }
        durable_remove_directory(&path).map_err(backend_io)
    }

    fn cleanup(
        &self,
        request: &VectorProjectionCleanupRequest,
    ) -> Result<VectorProjectionCleanupResponse, VectorProjectionBackendError> {
        self.validate_context(&request.context)?;
        validate_cleanup_protection(&request.protection)?;
        self.validate_cleanup_context_artifact(&request.context)?;
        let published = self.published_generations(&request.context.projection_store)?;
        let active = published.last().cloned();
        let previous = published.iter().rev().nth(1).cloned();
        let mut protected = BTreeMap::<String, VectorProjectionProtectionReason>::new();
        protect_optional(
            &mut protected,
            request.protection.active_generation.as_deref(),
            VectorProjectionProtectionReason::Active,
        );
        protect_optional(
            &mut protected,
            request.protection.previous_generation.as_deref(),
            VectorProjectionProtectionReason::Previous,
        );
        protect_optional(
            &mut protected,
            request.protection.building_generation.as_deref(),
            VectorProjectionProtectionReason::Building,
        );
        for generation in &request.protection.additional_generations {
            protected
                .entry(generation.clone())
                .or_insert(VectorProjectionProtectionReason::Explicit);
        }
        protect_evidence(
            &mut protected,
            active.as_ref(),
            VectorProjectionProtectionReason::Active,
        );
        protect_evidence(
            &mut protected,
            previous.as_ref(),
            VectorProjectionProtectionReason::Previous,
        );

        let generations = self.generations_root(&request.context.projection_store, false)?;
        let mut removed_generations = Vec::new();
        let mut skipped_generations = Vec::new();
        if path_is_directory_or_missing(&generations)? {
            for entry in sorted_entries(&generations)? {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !name.starts_with("gen_") {
                    continue;
                }
                if protected.contains_key(&name) {
                    continue;
                }
                let file_type = entry.file_type().map_err(backend_io)?;
                if !file_type.is_dir() {
                    skipped_generations.push(VectorProjectionSkippedGeneration {
                        generation_id: name,
                        reason: "generation_entry_is_not_a_directory".to_owned(),
                    });
                    continue;
                }
                if request.dry_run {
                    skipped_generations.push(VectorProjectionSkippedGeneration {
                        generation_id: name,
                        reason: "dry_run".to_owned(),
                    });
                } else {
                    durable_remove_directory(&entry.path()).map_err(backend_io)?;
                    removed_generations.push(name);
                }
            }
        }
        let protected_generations = protected
            .into_iter()
            .map(
                |(generation_id, reason)| VectorProjectionProtectedGeneration {
                    generation_id,
                    reason,
                },
            )
            .collect();
        Ok(VectorProjectionCleanupResponse {
            ack: ack(&request.context),
            dry_run: request.dry_run,
            removed_generations,
            protected_generations,
            skipped_generations,
        })
    }

    fn validate_cleanup_context_artifact(
        &self,
        context: &VectorProjectionMutationContext,
    ) -> Result<(), VectorProjectionBackendError> {
        let generations = self.generations_root(&context.projection_store, false)?;
        let generation_path = checked_generation_path(&generations, &context.generation_id)?;
        require_real_directory(&generation_path, "cleanup context generation")?;
        let snapshot =
            self.read_historical_snapshot_at(&context.projection_store, &generation_path)?;
        if snapshot.manifest.database_instance_id != self.database_instance_id
            || snapshot.manifest.store_name != context.projection_store
            || snapshot.manifest.generation != context.generation_id
            || snapshot.manifest.delivery_digest != context.delivery_digest
        {
            return Err(VectorProjectionBackendError::Delivery(
                "cleanup context does not match its persisted generation snapshot".to_owned(),
            ));
        }
        match fs::symlink_metadata(generation_path.join(EVIDENCE_FILE)) {
            Ok(_) => {
                let descriptor = self.require_store(&context.projection_store)?;
                let evidence = self.read_evidence_at(descriptor, &generation_path)?;
                if evidence.manifest != snapshot.manifest
                    || evidence.manifest.delivery_digest != context.delivery_digest
                {
                    return Err(VectorProjectionBackendError::Delivery(
                        "cleanup context does not match its persisted generation evidence"
                            .to_owned(),
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(backend_io(error)),
        }
        Ok(())
    }

    fn inventory(
        &self,
        store_name: &str,
    ) -> Result<Vec<VectorProjectionGenerationInventoryEntry>, VectorProjectionBackendError> {
        let published = self.published_generations(store_name)?;
        let active_id = published
            .last()
            .map(|item| item.manifest.generation.as_str());
        let previous_id = published
            .iter()
            .rev()
            .nth(1)
            .map(|item| item.manifest.generation.as_str());
        let generations = self.generations_root(store_name, false)?;
        if !path_is_directory_or_missing(&generations)? {
            return Ok(Vec::new());
        }
        let descriptor = self.require_store(store_name)?;
        let mut result = Vec::new();
        for entry in sorted_entries(&generations)? {
            let generation_id = entry.file_name().to_string_lossy().into_owned();
            let file_type = entry.file_type().map_err(backend_io)?;
            if generation_id.starts_with("gen_") && file_type.is_dir() {
                let evidence = self.read_evidence_at(descriptor, &entry.path()).ok();
                let state = if active_id == Some(generation_id.as_str()) {
                    VectorProjectionGenerationState::Active
                } else if previous_id == Some(generation_id.as_str()) {
                    VectorProjectionGenerationState::Previous
                } else if evidence.is_some() {
                    VectorProjectionGenerationState::Prepared
                } else {
                    VectorProjectionGenerationState::Building
                };
                result.push(VectorProjectionGenerationInventoryEntry {
                    generation_id,
                    state,
                    evidence,
                });
            } else if generation_id.starts_with(".gen_") && generation_id.contains(".quarantine.") {
                result.push(VectorProjectionGenerationInventoryEntry {
                    generation_id,
                    state: VectorProjectionGenerationState::Quarantined,
                    evidence: None,
                });
            } else {
                result.push(VectorProjectionGenerationInventoryEntry {
                    generation_id,
                    state: VectorProjectionGenerationState::Orphaned,
                    evidence: None,
                });
            }
        }
        Ok(result)
    }

    fn inspect_active(
        &self,
        store_name: &str,
    ) -> Result<Option<ProjectionArtifactEvidence>, VectorProjectionBackendError> {
        Ok(self.published_generations(store_name)?.pop())
    }

    fn inspect_generation(
        &self,
        store_name: &str,
        generation_id: &str,
    ) -> Result<Option<ProjectionArtifactEvidence>, VectorProjectionBackendError> {
        let descriptor = self.require_store(store_name)?;
        let generations = self.generations_root(store_name, false)?;
        let path = checked_generation_path(&generations, generation_id)?;
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() => self.read_evidence_at(descriptor, &path).map(Some),
            Ok(_) => Err(VectorProjectionBackendError::Backend(format!(
                "generation path is not a directory: {}",
                path.display()
            ))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(backend_io(error)),
        }
    }

    fn published_generations(
        &self,
        store_name: &str,
    ) -> Result<Vec<ProjectionArtifactEvidence>, VectorProjectionBackendError> {
        self.scan_published_generations(store_name, None)
    }

    fn published_generations_for_repair(
        &self,
        expected: &ProjectionArtifactEvidence,
    ) -> Result<Vec<ProjectionArtifactEvidence>, VectorProjectionBackendError> {
        self.scan_published_generations(&expected.manifest.store_name, Some(expected))
    }

    fn scan_published_generations(
        &self,
        store_name: &str,
        repair_candidate: Option<&ProjectionArtifactEvidence>,
    ) -> Result<Vec<ProjectionArtifactEvidence>, VectorProjectionBackendError> {
        let descriptor = self.require_store(store_name)?;
        let generations = self.generations_root(store_name, false)?;
        if !path_is_directory_or_missing(&generations)? {
            return Ok(Vec::new());
        }
        let mut published = Vec::new();
        let mut fences = BTreeSet::new();
        for entry in sorted_entries(&generations)? {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with("gen_") || !entry.file_type().map_err(backend_io)?.is_dir() {
                continue;
            }
            if !marker_exists(&entry.path())? {
                continue;
            }
            let evidence = self.read_evidence_at(descriptor, &entry.path())?;
            if !self.marker_is_valid(&evidence)? {
                if repair_candidate == Some(&evidence) {
                    continue;
                }
                return Err(VectorProjectionBackendError::Backend(format!(
                    "published marker is corrupt for generation {}",
                    evidence.manifest.generation
                )));
            }
            if !fences.insert(evidence.manifest.fence_epoch) {
                return Err(VectorProjectionBackendError::Backend(
                    "multiple published generations have the same fence epoch".to_owned(),
                ));
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

    fn read_evidence_at(
        &self,
        descriptor: &ProjectionStoreDescriptor,
        generation_path: &Path,
    ) -> Result<ProjectionArtifactEvidence, VectorProjectionBackendError> {
        require_regular_file(&generation_path.join(SNAPSHOT_FILE))?;
        require_regular_file(&generation_path.join(EVIDENCE_FILE))?;
        let snapshot: ProjectionSnapshot = serde_json::from_slice(
            &fs::read(generation_path.join(SNAPSHOT_FILE)).map_err(backend_io)?,
        )
        .map_err(backend_json)?;
        let evidence: ProjectionArtifactEvidence = serde_json::from_slice(
            &fs::read(generation_path.join(EVIDENCE_FILE)).map_err(backend_io)?,
        )
        .map_err(backend_json)?;
        self.validate_persisted_snapshot_historical(&snapshot, &descriptor.store_name)?;
        self.validate_snapshot_path_identity(&snapshot, generation_path)?;
        self.validate_evidence_historical(&evidence)?;
        if snapshot.manifest != evidence.manifest
            || snapshot_fingerprint(&snapshot) != evidence.fingerprint
            || evidence.manifest.fingerprint.as_deref() != Some(evidence.fingerprint.as_str())
        {
            return Err(VectorProjectionBackendError::Backend(
                "generation snapshot and evidence fingerprint do not match".to_owned(),
            ));
        }
        Ok(evidence)
    }

    fn validate_snapshot(
        &self,
        snapshot: &ProjectionSnapshot,
        descriptor: &ProjectionStoreDescriptor,
    ) -> Result<(), VectorProjectionBackendError> {
        if snapshot.manifest.fingerprint.is_some() {
            return Err(VectorProjectionBackendError::Delivery(
                "prepare snapshot manifest must not contain a physical fingerprint".to_owned(),
            ));
        }
        self.validate_manifest_current(&snapshot.manifest, descriptor)?;
        validate_record_coverage(snapshot)
    }

    fn validate_persisted_snapshot(
        &self,
        snapshot: &ProjectionSnapshot,
        descriptor: &ProjectionStoreDescriptor,
    ) -> Result<(), VectorProjectionBackendError> {
        self.validate_manifest_current(&snapshot.manifest, descriptor)?;
        let Some(fingerprint) = snapshot.manifest.fingerprint.as_deref() else {
            return Err(VectorProjectionBackendError::Backend(
                "persisted snapshot fingerprint is missing".to_owned(),
            ));
        };
        if fingerprint.is_empty() || snapshot_fingerprint(snapshot) != fingerprint {
            return Err(VectorProjectionBackendError::Backend(
                "persisted snapshot fingerprint does not match its manifest and records".to_owned(),
            ));
        }
        validate_record_coverage(snapshot)
    }

    fn validate_persisted_snapshot_historical(
        &self,
        snapshot: &ProjectionSnapshot,
        store_name: &str,
    ) -> Result<(), VectorProjectionBackendError> {
        self.validate_manifest_historical(&snapshot.manifest, store_name)?;
        let Some(fingerprint) = snapshot.manifest.fingerprint.as_deref() else {
            return Err(VectorProjectionBackendError::Backend(
                "persisted snapshot fingerprint is missing".to_owned(),
            ));
        };
        if fingerprint.is_empty() || snapshot_fingerprint(snapshot) != fingerprint {
            return Err(VectorProjectionBackendError::Backend(
                "persisted snapshot fingerprint does not match its manifest and records".to_owned(),
            ));
        }
        validate_record_coverage(snapshot)
    }

    fn validate_evidence_current(
        &self,
        evidence: &ProjectionArtifactEvidence,
    ) -> Result<(), VectorProjectionBackendError> {
        let descriptor = self.require_store(&evidence.manifest.store_name)?;
        self.validate_manifest_current(&evidence.manifest, descriptor)?;
        self.validate_evidence_fingerprint(evidence)
    }

    fn validate_evidence_historical(
        &self,
        evidence: &ProjectionArtifactEvidence,
    ) -> Result<(), VectorProjectionBackendError> {
        self.validate_manifest_historical(&evidence.manifest, &evidence.manifest.store_name)?;
        self.validate_evidence_fingerprint(evidence)
    }

    fn validate_evidence_fingerprint(
        &self,
        evidence: &ProjectionArtifactEvidence,
    ) -> Result<(), VectorProjectionBackendError> {
        if evidence.fingerprint.trim().is_empty()
            || evidence.manifest.fingerprint.as_deref() != Some(evidence.fingerprint.as_str())
        {
            return Err(VectorProjectionBackendError::Delivery(
                "projection evidence fingerprint is incomplete".to_owned(),
            ));
        }
        Ok(())
    }

    fn validate_manifest_current(
        &self,
        manifest: &ProjectionArtifactManifest,
        descriptor: &ProjectionStoreDescriptor,
    ) -> Result<(), VectorProjectionBackendError> {
        self.validate_manifest_historical(manifest, &descriptor.store_name)?;
        if manifest.provider != descriptor.provider
            || manifest.provider_fingerprint != descriptor.provider_fingerprint
            || manifest.corpus != descriptor.corpus
        {
            return Err(VectorProjectionBackendError::Delivery(
                "projection manifest does not match database/store/provider/corpus binding"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    fn validate_manifest_historical(
        &self,
        manifest: &ProjectionArtifactManifest,
        store_name: &str,
    ) -> Result<(), VectorProjectionBackendError> {
        self.require_store(store_name)?;
        let expected_corpus_schema = match store_name {
            LANCEDB_CHUNKS_STORE => TASK_CHUNKS_CORPUS_SCHEMA,
            LANCEDB_LABEL_ATOMS_STORE => LABEL_ATOMS_CORPUS_SCHEMA,
            _ => {
                return Err(VectorProjectionBackendError::Protocol(format!(
                    "unsupported LanceDB projection store: {store_name}"
                )));
            }
        };
        let corpus = manifest.corpus.as_ref();
        if manifest.store_name != store_name
            || manifest.database_instance_id != self.database_instance_id
            || manifest.protocol_version != VECTOR_PROJECTION_PROTOCOL_VERSION
            || manifest.schema_version != DERIVED_STORE_SCHEMA_VERSION
            || !manifest.generation.starts_with("gen_")
            || manifest.fence_epoch < 0
            || manifest.snapshot_cursor < 0
            || manifest.provider.trim().is_empty()
            || manifest.provider_fingerprint.trim().is_empty()
            || corpus.is_none_or(|corpus| {
                corpus.corpus_schema != expected_corpus_schema
                    || corpus.corpus_fingerprint
                        != corpus_provider_fingerprint(
                            expected_corpus_schema,
                            &manifest.provider_fingerprint,
                        )
                    || corpus.embedding_model.trim().is_empty()
                    || corpus.embedding_dimensions == 0
            })
            || manifest.canonical_item_count < 0
            || manifest.canonical_digest.trim().is_empty()
            || manifest.delivery_item_count < 0
            || manifest.delivery_digest.trim().is_empty()
        {
            return Err(VectorProjectionBackendError::Delivery(
                "historical projection manifest has an invalid self-contained database/store/provider/corpus binding"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    fn validate_context(
        &self,
        context: &VectorProjectionMutationContext,
    ) -> Result<(), VectorProjectionBackendError> {
        require_non_empty(&context.request_id, "request_id")?;
        self.require_store(&context.projection_store)?;
        require_non_empty(&context.generation_id, "generation_id")?;
        require_non_empty(&context.delivery_digest, "delivery_digest")?;
        if !context.generation_id.starts_with("gen_") {
            return Err(VectorProjectionBackendError::Protocol(
                "generation_id must start with gen_".to_owned(),
            ));
        }
        Ok(())
    }

    /// Re-check SQLite only after the cross-process helper lock is held.
    ///
    /// Prepare, publish and repair requests intentionally do not carry lease
    /// capabilities on the wire. For those operations this proves that an
    /// unexpired SQLite lease exists and that the request's fenced generation
    /// evidence is still the exact canonical authority; it does not claim that
    /// the request possesses the opaque lease token. Apply requests carry both
    /// lease and claim capabilities and are checked more strongly below.
    fn validate_prepare_authority(
        &self,
        request: &VectorProjectionPrepareSnapshotRequest,
    ) -> Result<(), VectorProjectionBackendError> {
        self.validate_context(&request.context)?;
        let descriptor = self.require_store(&request.context.projection_store)?;
        self.validate_snapshot(&request.snapshot, descriptor)?;
        let manifest = &request.snapshot.manifest;
        if request.context.generation_id != manifest.generation
            || request.context.delivery_digest != manifest.delivery_digest
            || request.metadata
                != manifest.corpus.clone().ok_or_else(|| {
                    VectorProjectionBackendError::Delivery(
                        "prepare snapshot has no corpus authority".to_owned(),
                    )
                })?
        {
            return Err(stale_sqlite_authority(
                "prepare",
                &request.context.projection_store,
            ));
        }
        let authority = self.load_sqlite_mutation_authority(&request.context.projection_store)?;
        self.require_current_sqlite_lease(&authority, manifest.fence_epoch)?;
        if authority.building.phase.as_deref() != Some("snapshotting")
            || !authority.building.matches_manifest(manifest, None)
        {
            return Err(stale_sqlite_authority(
                "prepare",
                &request.context.projection_store,
            ));
        }
        Ok(())
    }

    fn validate_apply_authority(
        &self,
        request: &VectorProjectionApplyBatchRequest,
    ) -> Result<(), VectorProjectionBackendError> {
        self.validate_context(&request.context)?;
        let descriptor = self.require_store(&request.context.projection_store)?;
        let authority = self.load_sqlite_mutation_authority(&request.context.projection_store)?;
        self.require_sqlite_identity(&authority)?;
        let now = current_time_ms()?;
        if authority.fence_epoch != request.batch.fence_epoch
            || authority.lease_owner.as_deref() != Some(request.batch.owner.as_str())
            || authority.lease_token.as_deref() != Some(request.batch.lease_token.as_str())
            || authority
                .lease_expires_at
                .is_none_or(|expires_at| expires_at <= now)
            || request.batch.claim_expires_at <= now
            || authority
                .lease_expires_at
                .is_none_or(|expires_at| request.batch.claim_expires_at > expires_at)
        {
            return Err(stale_sqlite_authority(
                "apply",
                &request.context.projection_store,
            ));
        }

        let target = if authority.building.generation.is_some() {
            if !matches!(
                authority.building.phase.as_deref(),
                Some("prepared" | "store_published")
            ) || authority.building.generation.as_deref()
                != Some(request.batch.target_generation.as_str())
            {
                return Err(stale_sqlite_authority(
                    "apply",
                    &request.context.projection_store,
                ));
            }
            &authority.building
        } else {
            &authority.active
        };
        let evidence = self
            .inspect_generation(
                &request.context.projection_store,
                &request.context.generation_id,
            )?
            .ok_or_else(|| stale_sqlite_authority("apply", &request.context.projection_store))?;
        self.validate_batch_binding(&request.context, &request.batch, descriptor, &evidence)?;
        if !target.matches_manifest(&evidence.manifest, Some(evidence.fingerprint.as_str()))
            || request.batch.provider != target.provider.as_deref().unwrap_or_default()
            || request.batch.provider_fingerprint
                != target.provider_fingerprint.as_deref().unwrap_or_default()
        {
            return Err(stale_sqlite_authority(
                "apply",
                &request.context.projection_store,
            ));
        }

        let conn = open_readonly_database(&self.db_path)?;
        for delivery in &request.batch.items {
            let actual = sqlite_delivery_claim(&conn, delivery.id)?.ok_or_else(|| {
                stale_sqlite_authority("apply", &request.context.projection_store)
            })?;
            let expected = SqliteDeliveryClaim {
                id: delivery.id,
                outbox_id: delivery.outbox_id,
                store_name: delivery.store_name.clone(),
                board_id: delivery.board_id.clone(),
                source_event_id: delivery.source_event_id,
                cursor: delivery.cursor,
                action: delivery_action_name(delivery.action).to_owned(),
                entity_uri: delivery.entity_uri.clone(),
                payload_json: delivery.payload_json.clone(),
                status: "running".to_owned(),
                attempts: delivery.attempts,
                claim_owner: Some(request.batch.owner.clone()),
                claim_token: Some(request.batch.claim_token.clone()),
                claim_lease_token: Some(request.batch.lease_token.clone()),
                claim_fence_epoch: Some(request.batch.fence_epoch),
                claim_generation: Some(request.batch.target_generation.clone()),
                claim_expires_at: Some(request.batch.claim_expires_at),
            };
            if actual != expected {
                return Err(stale_sqlite_authority(
                    "apply",
                    &request.context.projection_store,
                ));
            }
        }
        let claimed_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM projection_deliveries
                 WHERE store_name=?1 AND status='running'
                   AND claim_owner=?2 AND claim_token=?3
                   AND claim_lease_token=?4 AND claim_fence_epoch=?5
                   AND claim_generation=?6 AND claim_expires_at=?7",
                params![
                    request.batch.store_name,
                    request.batch.owner,
                    request.batch.claim_token,
                    request.batch.lease_token,
                    request.batch.fence_epoch,
                    request.batch.target_generation,
                    request.batch.claim_expires_at,
                ],
                |row| row.get(0),
            )
            .map_err(backend_sql)?;
        if usize::try_from(claimed_count).ok() != Some(request.batch.items.len()) {
            return Err(stale_sqlite_authority(
                "apply",
                &request.context.projection_store,
            ));
        }
        Ok(())
    }

    fn validate_publish_authority(
        &self,
        request: &VectorProjectionPublishRequest,
    ) -> Result<(), VectorProjectionBackendError> {
        self.validate_context(&request.context)?;
        self.validate_evidence_current(&request.prepared)?;
        let manifest = &request.prepared.manifest;
        if request.context.generation_id != manifest.generation
            || request.context.delivery_digest != manifest.delivery_digest
        {
            return Err(stale_sqlite_authority(
                "publish",
                &request.context.projection_store,
            ));
        }
        let authority = self.load_sqlite_mutation_authority(&request.context.projection_store)?;
        self.require_current_sqlite_lease(&authority, manifest.fence_epoch)?;
        if !matches!(
            authority.building.phase.as_deref(),
            Some("prepared" | "store_published")
        ) || !authority
            .building
            .matches_manifest(manifest, Some(request.prepared.fingerprint.as_str()))
        {
            return Err(stale_sqlite_authority(
                "publish",
                &request.context.projection_store,
            ));
        }
        match &request.expected_active {
            Some(expected) => {
                self.validate_evidence_historical(expected)?;
                if !authority
                    .active
                    .matches_manifest(&expected.manifest, Some(expected.fingerprint.as_str()))
                {
                    return Err(stale_sqlite_authority(
                        "publish",
                        &request.context.projection_store,
                    ));
                }
            }
            None if !authority.active.is_absent() => {
                return Err(stale_sqlite_authority(
                    "publish",
                    &request.context.projection_store,
                ));
            }
            None => {}
        }
        Ok(())
    }

    fn validate_repair_authority(
        &self,
        request: &VectorProjectionRepairPublicationRequest,
    ) -> Result<(), VectorProjectionBackendError> {
        self.validate_context(&request.context)?;
        self.validate_evidence_current(&request.expected)?;
        if request.context.generation_id != request.expected.manifest.generation
            || request.context.delivery_digest != request.expected.manifest.delivery_digest
        {
            return Err(stale_sqlite_authority(
                "repair",
                &request.context.projection_store,
            ));
        }
        let authority = self.load_sqlite_mutation_authority(&request.context.projection_store)?;
        self.require_sqlite_identity(&authority)?;
        self.require_unexpired_sqlite_lease(&authority)?;
        let matches_active = authority.active.matches_manifest(
            &request.expected.manifest,
            Some(request.expected.fingerprint.as_str()),
        );
        let matches_building = matches!(
            authority.building.phase.as_deref(),
            Some("prepared" | "store_published")
        ) && authority.building.matches_manifest(
            &request.expected.manifest,
            Some(request.expected.fingerprint.as_str()),
        );
        if !matches_active && !matches_building {
            return Err(stale_sqlite_authority(
                "repair",
                &request.context.projection_store,
            ));
        }
        if matches_building && authority.fence_epoch != request.expected.manifest.fence_epoch {
            return Err(stale_sqlite_authority(
                "repair",
                &request.context.projection_store,
            ));
        }
        if matches_active
            && authority.fence_epoch != request.expected.manifest.fence_epoch
            && (authority.building.fence_epoch != Some(authority.fence_epoch)
                || !matches!(
                    authority.building.phase.as_deref(),
                    Some("snapshotting" | "prepared" | "store_published")
                ))
        {
            return Err(stale_sqlite_authority(
                "repair",
                &request.context.projection_store,
            ));
        }
        Ok(())
    }

    fn load_sqlite_mutation_authority(
        &self,
        store_name: &str,
    ) -> Result<SqliteMutationAuthority, VectorProjectionBackendError> {
        let conn = open_readonly_database(&self.db_path)?;
        self.load_sqlite_mutation_authority_from(&conn, store_name)
    }

    fn load_sqlite_mutation_authority_from(
        &self,
        conn: &Connection,
        store_name: &str,
    ) -> Result<SqliteMutationAuthority, VectorProjectionBackendError> {
        conn.query_row(
            "SELECT database_instance_id,protocol_version,schema_version,control_plane,
                    fence_epoch,lease_owner,lease_token,lease_expires_at,
                    building_generation,building_fingerprint,building_fence_epoch,
                    building_provider,building_provider_fingerprint,
                    building_canonical_count,building_canonical_digest,
                    building_delivery_count,building_delivery_digest,
                    building_corpus_schema,building_corpus_fingerprint,
                    building_embedding_model,building_embedding_dimensions,
                    building_phase,snapshot_cursor,
                    active_generation,active_fingerprint,active_fence_epoch,
                    active_snapshot_cursor,active_provider,active_provider_fingerprint,
                    active_canonical_count,active_canonical_digest,
                    active_delivery_count,active_delivery_digest,
                    active_corpus_schema,active_corpus_fingerprint,
                    active_embedding_model,active_embedding_dimensions
             FROM projection_store_state WHERE store_name=?1",
            [store_name],
            |row| {
                Ok(SqliteMutationAuthority {
                    database_instance_id: row.get(0)?,
                    protocol_version: row.get(1)?,
                    schema_version: row.get(2)?,
                    control_plane: row.get(3)?,
                    fence_epoch: row.get(4)?,
                    lease_owner: row.get(5)?,
                    lease_token: row.get(6)?,
                    lease_expires_at: row.get(7)?,
                    building: SqliteGenerationAuthority {
                        generation: row.get(8)?,
                        fingerprint: row.get(9)?,
                        fence_epoch: row.get(10)?,
                        snapshot_cursor: row.get(22)?,
                        provider: row.get(11)?,
                        provider_fingerprint: row.get(12)?,
                        canonical_item_count: row.get(13)?,
                        canonical_digest: row.get(14)?,
                        delivery_item_count: row.get(15)?,
                        delivery_digest: row.get(16)?,
                        corpus_schema: row.get(17)?,
                        corpus_fingerprint: row.get(18)?,
                        embedding_model: row.get(19)?,
                        embedding_dimensions: row.get(20)?,
                        phase: row.get(21)?,
                    },
                    active: SqliteGenerationAuthority {
                        generation: row.get(23)?,
                        fingerprint: row.get(24)?,
                        fence_epoch: row.get(25)?,
                        snapshot_cursor: row.get(26)?,
                        provider: row.get(27)?,
                        provider_fingerprint: row.get(28)?,
                        canonical_item_count: row.get(29)?,
                        canonical_digest: row.get(30)?,
                        delivery_item_count: row.get(31)?,
                        delivery_digest: row.get(32)?,
                        corpus_schema: row.get(33)?,
                        corpus_fingerprint: row.get(34)?,
                        embedding_model: row.get(35)?,
                        embedding_dimensions: row.get(36)?,
                        phase: None,
                    },
                })
            },
        )
        .optional()
        .map_err(backend_sql)?
        .ok_or_else(|| stale_sqlite_authority("mutation", store_name))
    }

    fn require_sqlite_identity(
        &self,
        authority: &SqliteMutationAuthority,
    ) -> Result<(), VectorProjectionBackendError> {
        if authority.database_instance_id != self.database_instance_id
            || authority.protocol_version != VECTOR_PROJECTION_PROTOCOL_VERSION
            || authority.schema_version != DERIVED_STORE_SCHEMA_VERSION
            || authority.control_plane != "v2"
        {
            return Err(stale_sqlite_authority("mutation", "configured store"));
        }
        Ok(())
    }

    fn require_sqlite_read_identity(
        &self,
        authority: &SqliteMutationAuthority,
        store_name: &str,
    ) -> Result<(), VectorProjectionBackendError> {
        if authority.database_instance_id != self.database_instance_id
            || authority.protocol_version != VECTOR_PROJECTION_PROTOCOL_VERSION
            || authority.schema_version != DERIVED_STORE_SCHEMA_VERSION
            || authority.control_plane != "v2"
        {
            return Err(VectorProjectionBackendError::Delivery(format!(
                "SQLite Projection v2 read authority rejected database/store/version/control-plane mismatch for {store_name}"
            )));
        }
        Ok(())
    }

    fn require_unexpired_sqlite_lease(
        &self,
        authority: &SqliteMutationAuthority,
    ) -> Result<(), VectorProjectionBackendError> {
        self.require_sqlite_identity(authority)?;
        let now = current_time_ms()?;
        if authority
            .lease_owner
            .as_deref()
            .is_none_or(|owner| owner.trim().is_empty())
            || authority
                .lease_token
                .as_deref()
                .is_none_or(|token| token.trim().is_empty())
            || authority
                .lease_expires_at
                .is_none_or(|expires_at| expires_at <= now)
        {
            return Err(stale_sqlite_authority("mutation", "configured store"));
        }
        Ok(())
    }

    fn require_current_sqlite_lease(
        &self,
        authority: &SqliteMutationAuthority,
        fence_epoch: i64,
    ) -> Result<(), VectorProjectionBackendError> {
        self.require_unexpired_sqlite_lease(authority)?;
        if authority.fence_epoch != fence_epoch {
            return Err(stale_sqlite_authority("mutation", "configured store"));
        }
        Ok(())
    }

    fn require_store(
        &self,
        store_name: &str,
    ) -> Result<&ProjectionStoreDescriptor, VectorProjectionBackendError> {
        self.stores
            .iter()
            .find(|store| store.store_name == store_name)
            .ok_or_else(|| {
                VectorProjectionBackendError::Protocol(format!(
                    "unsupported LanceDB projection store: {store_name}"
                ))
            })
    }

    fn generations_root(
        &self,
        store_name: &str,
        create: bool,
    ) -> Result<PathBuf, VectorProjectionBackendError> {
        self.require_store(store_name)?;
        if create {
            ensure_projection_store_generations_path(
                &self.db_path,
                &self.database_instance_id,
                store_name,
            )
            .map_err(backend_io)
        } else {
            checked_projection_store_generations_path(
                &self.db_path,
                &self.database_instance_id,
                store_name,
            )
            .map_err(backend_io)
        }
    }

    fn acquire_mutation_guard(
        &self,
        store_name: &str,
    ) -> Result<DerivedStoreWriteGuard, VectorProjectionBackendError> {
        self.require_store(store_name)?;
        let lock_name = format!("{store_name}-projection-helper");
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match DerivedStoreWriteGuard::acquire(&self.db_path, &lock_name) {
                Ok(guard) => return Ok(guard),
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    return Err(VectorProjectionBackendError::Busy(format!(
                        "timed out waiting for the {store_name} helper mutation lock"
                    )));
                }
                Err(error) => return Err(backend_io(error)),
            }
        }
    }

    fn repair_marker(
        &self,
        evidence: &ProjectionArtifactEvidence,
    ) -> Result<(), VectorProjectionBackendError> {
        let generations = self.generations_root(&evidence.manifest.store_name, false)?;
        let generation_path = checked_generation_path(&generations, &evidence.manifest.generation)?;
        let stored = self
            .inspect_generation(&evidence.manifest.store_name, &evidence.manifest.generation)?
            .ok_or_else(|| {
                VectorProjectionBackendError::Delivery(
                    "generation is missing during marker repair".to_owned(),
                )
            })?;
        if stored != *evidence {
            return Err(VectorProjectionBackendError::Delivery(
                "generation evidence mismatch during marker repair".to_owned(),
            ));
        }
        self.validate_historical_materialization(evidence, &generation_path)?;
        self.validate_content_metadata(&generation_path, evidence)?;
        self.validate_historical_auxiliary_state(&generation_path, evidence)?;
        let marker = generation_path.join(PUBLISHED_MARKER);
        let expected = marker_contents(evidence);
        match fs::symlink_metadata(&marker) {
            Ok(metadata) if metadata.is_file() => {
                if fs::read(&marker).map_err(backend_io)? == expected {
                    return Ok(());
                }
                durable_quarantine_entry(&marker).map_err(backend_io)?;
            }
            Ok(_) => {
                durable_quarantine_entry(&marker).map_err(backend_io)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(backend_io(error)),
        }
        durable_create_new_file(&marker, &expected).map_err(backend_io)?;
        if !self.marker_is_valid(evidence)? {
            return Err(VectorProjectionBackendError::Backend(
                "publication marker read-back failed".to_owned(),
            ));
        }
        Ok(())
    }

    fn marker_is_valid(
        &self,
        evidence: &ProjectionArtifactEvidence,
    ) -> Result<bool, VectorProjectionBackendError> {
        let generations = self.generations_root(&evidence.manifest.store_name, false)?;
        let generation_path = checked_generation_path(&generations, &evidence.manifest.generation)?;
        let marker = generation_path.join(PUBLISHED_MARKER);
        match fs::symlink_metadata(&marker) {
            Ok(metadata) if metadata.is_file() => {
                Ok(fs::read(marker).map_err(backend_io)? == marker_contents(evidence))
            }
            Ok(_) => Ok(false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(backend_io(error)),
        }
    }

    fn error_response(
        &self,
        request: &VectorProjectionHelperRequest,
        error: VectorProjectionBackendError,
    ) -> VectorProjectionHelperResponse {
        let (request_id, projection_store, generation_id, delivery_digest) =
            request_correlation(request);
        let retryable = matches!(
            &error,
            VectorProjectionBackendError::Provider {
                retryable: true,
                ..
            } | VectorProjectionBackendError::Busy(_)
        );
        VectorProjectionHelperResponse::Error(VectorProjectionHelperError {
            kind: error.kind(),
            code: error.code().to_owned(),
            provider: Some(self.provider.provider_name().to_owned()),
            backend: Some("lancedb".to_owned()),
            retryable,
            message: error.to_string(),
            request_id: Some(request_id.to_owned()),
            delivery_digest: delivery_digest.map(str::to_owned),
            projection_store: projection_store.map(str::to_owned),
            generation_id: generation_id.map(str::to_owned),
        })
    }
}

const TASK_SOURCE_SELECT: &str = r#"
    SELECT 'kb://task/' || t.id,t.board_id,t.id,t.title,t.description,
           COALESCE((
             SELECT group_concat(ordered.body, char(10))
             FROM (
               SELECT c.body
               FROM task_comments c
               WHERE c.board_id=t.board_id AND c.task_id=t.id
               ORDER BY c.created_at,c.id
             ) ordered
           ),''),
           COALESCE((
             SELECT group_concat(ordered.text, char(10))
             FROM (
               SELECT COALESCE(r.summary,'') || ' ' || COALESCE(r.error,'') AS text
               FROM task_runs r
               WHERE r.board_id=t.board_id AND r.task_id=t.id
               ORDER BY r.started_at,r.id
             ) ordered
           ),''),
           COALESCE((
             SELECT group_concat(ordered.text, char(10))
             FROM (
               SELECT e.kind || ' ' || e.payload_json AS text
               FROM task_events e
               WHERE e.board_id=t.board_id AND e.task_id=t.id
               ORDER BY e.id
             ) ordered
           ),''),
           (SELECT MAX(e.id) FROM task_events e
            WHERE e.board_id=t.board_id AND e.task_id=t.id),
           t.created_at,t.updated_at
    FROM tasks t
"#;

fn acquire_helper_read_guard(
    db_path: &Path,
    store_name: &str,
) -> Result<DerivedStoreReadGuard, VectorProjectionBackendError> {
    if !matches!(store_name, LANCEDB_CHUNKS_STORE | LANCEDB_LABEL_ATOMS_STORE) {
        return Err(VectorProjectionBackendError::Protocol(format!(
            "unsupported LanceDB projection store: {store_name}"
        )));
    }
    let lock_name = format!("{store_name}-projection-helper");
    DerivedStoreReadGuard::acquire(db_path, &lock_name).map_err(|error| {
        if error.kind() == std::io::ErrorKind::WouldBlock {
            VectorProjectionBackendError::Busy(format!(
                "the {store_name} helper has an active physical writer"
            ))
        } else {
            backend_io(error)
        }
    })
}

fn open_readonly_database(path: &Path) -> Result<DatabaseConnection, VectorProjectionBackendError> {
    connect_existing_read_only(path)
        .map_err(|error| VectorProjectionBackendError::Backend(error.to_string()))
}

fn sqlite_delivery_claim(
    conn: &Connection,
    delivery_id: i64,
) -> Result<Option<SqliteDeliveryClaim>, VectorProjectionBackendError> {
    conn.query_row(
        "SELECT id,outbox_id,store_name,board_id,source_event_id,cursor,action,
                entity_uri,payload_json,status,attempts,claim_owner,claim_token,
                claim_lease_token,claim_fence_epoch,claim_generation,claim_expires_at
         FROM projection_deliveries WHERE id=?1",
        [delivery_id],
        |row| {
            Ok(SqliteDeliveryClaim {
                id: row.get(0)?,
                outbox_id: row.get(1)?,
                store_name: row.get(2)?,
                board_id: row.get(3)?,
                source_event_id: row.get(4)?,
                cursor: row.get(5)?,
                action: row.get(6)?,
                entity_uri: row.get(7)?,
                payload_json: row.get(8)?,
                status: row.get(9)?,
                attempts: row.get(10)?,
                claim_owner: row.get(11)?,
                claim_token: row.get(12)?,
                claim_lease_token: row.get(13)?,
                claim_fence_epoch: row.get(14)?,
                claim_generation: row.get(15)?,
                claim_expires_at: row.get(16)?,
            })
        },
    )
    .optional()
    .map_err(backend_sql)
}

fn delivery_action_name(action: ProjectionDeliveryAction) -> &'static str {
    match action {
        ProjectionDeliveryAction::Upsert => "upsert",
        ProjectionDeliveryAction::Delete => "delete",
        ProjectionDeliveryAction::Rebuild => "rebuild",
    }
}

fn stale_sqlite_authority(action: &str, store_name: &str) -> VectorProjectionBackendError {
    VectorProjectionBackendError::Delivery(format!(
        "SQLite Projection v2 authority rejected stale {action} mutation for {store_name}"
    ))
}

fn require_delivery_source_event_board(
    conn: &Connection,
    delivery: &ProjectionDelivery,
) -> Result<(), VectorProjectionBackendError> {
    let Some(source_event_id) = delivery.source_event_id else {
        return Ok(());
    };
    let exists = conn
        .query_row(
            "SELECT 1 FROM task_events WHERE id=?1 AND board_id=?2",
            params![source_event_id, delivery.board_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(backend_sql)?
        .is_some();
    if !exists {
        return Err(VectorProjectionBackendError::Delivery(format!(
            "delivery {} source event is missing or belongs to another board",
            delivery.id
        )));
    }
    Ok(())
}

fn require_taskless_board_upsert_source(
    conn: &Connection,
    delivery: &ProjectionDelivery,
) -> Result<(), VectorProjectionBackendError> {
    let Some(source_event_id) = delivery.source_event_id else {
        return Err(VectorProjectionBackendError::Delivery(format!(
            "taskless board upsert delivery {} requires canonical source event evidence",
            delivery.id
        )));
    };
    let source = conn
        .query_row(
            "SELECT task_id,run_id FROM task_events WHERE id=?1 AND board_id=?2",
            params![source_event_id, delivery.board_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .optional()
        .map_err(backend_sql)?;
    match source {
        Some((None, None)) => Ok(()),
        Some(_) => Err(VectorProjectionBackendError::Delivery(format!(
            "taskless board upsert delivery {} source event is task or run scoped",
            delivery.id
        ))),
        None => Err(VectorProjectionBackendError::Delivery(format!(
            "taskless board upsert delivery {} source event is missing or belongs to another board",
            delivery.id
        ))),
    }
}

fn task_sources_for_board(
    conn: &Connection,
    board_id: &str,
) -> Result<Vec<TaskChunkSource>, VectorProjectionBackendError> {
    let sql = format!(
        "{TASK_SOURCE_SELECT}
         WHERE t.board_id=?1 AND t.status!='archived'
         ORDER BY t.seq,t.id"
    );
    let mut statement = conn.prepare(&sql).map_err(backend_sql)?;
    let rows = statement
        .query_map([board_id], task_chunk_source_from_row)
        .map_err(backend_sql)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(backend_sql)
}

fn all_task_sources(
    conn: &Connection,
) -> Result<Vec<TaskChunkSource>, VectorProjectionBackendError> {
    let sql = format!(
        "{TASK_SOURCE_SELECT}
         WHERE t.status!='archived'
         ORDER BY t.board_id,t.seq,t.id"
    );
    let mut statement = conn.prepare(&sql).map_err(backend_sql)?;
    let rows = statement
        .query_map([], task_chunk_source_from_row)
        .map_err(backend_sql)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(backend_sql)
}

fn task_source(
    conn: &Connection,
    board_id: &str,
    task_id: &str,
) -> Result<Option<TaskChunkSource>, VectorProjectionBackendError> {
    let sql = format!(
        "{TASK_SOURCE_SELECT}
         WHERE t.board_id=?1 AND t.id=?2
           AND t.status!='archived'"
    );
    conn.query_row(&sql, params![board_id, task_id], task_chunk_source_from_row)
        .optional()
        .map_err(backend_sql)
}

fn task_chunk_source_from_row(row: &Row<'_>) -> rusqlite::Result<TaskChunkSource> {
    Ok(TaskChunkSource {
        task_uri: row.get(0)?,
        project_id: None,
        board_id: Some(row.get(1)?),
        task_id: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        comments: row.get(5)?,
        run_text: row.get(6)?,
        event_text: row.get(7)?,
        // Projection v2 delivery ids are correlation/invalidation only. They
        // must never perturb the stable task chunk corpus.
        source_event_id: None,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn label_atoms_for_board(
    conn: &Connection,
    board_id: &str,
    embedding_model: &str,
) -> Result<Vec<LabelAtomVector>, VectorProjectionBackendError> {
    let mut statement = conn
        .prepare(
            "SELECT a.id,a.label_id,l.name,a.polarity,a.kind,a.text,a.ordinal,
                    a.content_hash,a.created_at,a.updated_at
             FROM label_atoms a
             JOIN labels l ON l.id=a.label_id AND l.board_id=a.board_id
             WHERE a.board_id=?1
             ORDER BY l.name,a.ordinal,a.id",
        )
        .map_err(backend_sql)?;
    let rows = statement
        .query_map([board_id], |row| {
            Ok(LabelAtomVector {
                atom_id: row.get(0)?,
                label_id: row.get(1)?,
                label_name: row.get(2)?,
                board_id: board_id.to_owned(),
                polarity: row.get(3)?,
                kind: row.get(4)?,
                text: row.get(5)?,
                ordinal: row.get(6)?,
                content_hash: row.get(7)?,
                embedding_model: embedding_model.to_owned(),
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .map_err(backend_sql)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(backend_sql)
}

fn all_label_atoms(
    conn: &Connection,
    embedding_model: &str,
) -> Result<Vec<LabelAtomVector>, VectorProjectionBackendError> {
    let mut statement = conn
        .prepare(
            "SELECT a.id,a.label_id,l.name,a.board_id,a.polarity,a.kind,a.text,a.ordinal,
                    a.content_hash,a.created_at,a.updated_at
             FROM label_atoms a
             JOIN labels l ON l.id=a.label_id AND l.board_id=a.board_id
             ORDER BY a.board_id,l.name,a.ordinal,a.id",
        )
        .map_err(backend_sql)?;
    let rows = statement
        .query_map([], |row| {
            Ok(LabelAtomVector {
                atom_id: row.get(0)?,
                label_id: row.get(1)?,
                label_name: row.get(2)?,
                board_id: row.get(3)?,
                polarity: row.get(4)?,
                kind: row.get(5)?,
                text: row.get(6)?,
                ordinal: row.get(7)?,
                content_hash: row.get(8)?,
                embedding_model: embedding_model.to_owned(),
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })
        .map_err(backend_sql)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(backend_sql)
}

fn require_embedding_cache_binding(
    stored: &EmbeddingCacheFile,
    expected: &EmbeddingCacheFile,
) -> Result<(), VectorProjectionBackendError> {
    if stored.format_version != expected.format_version
        || stored.database_instance_id != expected.database_instance_id
        || stored.store_name != expected.store_name
        || stored.generation_id != expected.generation_id
        || stored.provider_fingerprint != expected.provider_fingerprint
        || stored.corpus_fingerprint != expected.corpus_fingerprint
        || stored.embedding_model != expected.embedding_model
        || stored.embedding_dimensions != expected.embedding_dimensions
    {
        return Err(VectorProjectionBackendError::Delivery(
            "persistent embedding cache binding does not match the generation corpus/provider"
                .to_owned(),
        ));
    }
    let prefix = embedding_cache_key_prefix(stored);
    for (cache_key, cached) in &stored.entries {
        if cached.normalized_text.is_empty()
            || normalize_semantic_text(&cached.normalized_text) != cached.normalized_text
            || semantic_content_hash(&cached.normalized_text) != cached.content_hash
            || embedding_cache_key(&prefix, &cached.content_hash) != *cache_key
            || cached.vector.len() != stored.embedding_dimensions
        {
            return Err(VectorProjectionBackendError::Backend(
                "persistent embedding cache contains an invalid entry".to_owned(),
            ));
        }
    }
    Ok(())
}

fn embedding_cache_key_prefix(binding: &EmbeddingCacheFile) -> String {
    [
        ("database", binding.database_instance_id.as_str()),
        ("store", binding.store_name.as_str()),
        ("generation", binding.generation_id.as_str()),
        ("provider", binding.provider_fingerprint.as_str()),
        ("corpus", binding.corpus_fingerprint.as_str()),
        ("model", binding.embedding_model.as_str()),
    ]
    .into_iter()
    .map(|(name, value)| format!("{name}:{}:{value}", value.len()))
    .chain(std::iter::once(format!(
        "dimensions:{}",
        binding.embedding_dimensions
    )))
    .collect::<Vec<_>>()
    .join("|")
}

fn embedding_cache_key(prefix: &str, content_hash: &str) -> String {
    format!(
        "{prefix}|content_hash:{}:{content_hash}",
        content_hash.len()
    )
}

fn require_delivery_state_binding(
    stored: &DeliveryStateFile,
    expected: &DeliveryStateFile,
) -> Result<(), VectorProjectionBackendError> {
    if stored.format_version != expected.format_version
        || stored.database_instance_id != expected.database_instance_id
        || stored.store_name != expected.store_name
        || stored.generation_id != expected.generation_id
        || stored.provider_fingerprint != expected.provider_fingerprint
        || stored.corpus_fingerprint != expected.corpus_fingerprint
        || stored.evidence_fingerprint != expected.evidence_fingerprint
    {
        return Err(VectorProjectionBackendError::Delivery(
            "delivery replay state does not match the generation corpus/provider/evidence"
                .to_owned(),
        ));
    }
    if stored.applied.iter().any(|(id, signature)| {
        id.parse::<i64>().ok().is_none_or(|id| id <= 0)
            || !signature.strip_prefix("fnv64:").is_some_and(|digest| {
                digest.len() == 16 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
    }) {
        return Err(VectorProjectionBackendError::Backend(
            "delivery replay state contains an invalid entry".to_owned(),
        ));
    }
    Ok(())
}

fn delivery_signature(delivery: &ProjectionDelivery) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    hash_bytes(&mut hash, &delivery.id.to_le_bytes());
    hash_bytes(&mut hash, &delivery.outbox_id.to_le_bytes());
    hash_bytes(&mut hash, delivery.store_name.as_bytes());
    hash_bytes(&mut hash, delivery.generation_id.as_bytes());
    hash_bytes(&mut hash, delivery.board_id.as_bytes());
    match delivery.source_event_id {
        Some(source_event_id) => {
            hash_bytes(&mut hash, &[1]);
            hash_bytes(&mut hash, &source_event_id.to_le_bytes());
        }
        None => hash_bytes(&mut hash, &[0]),
    }
    hash_bytes(&mut hash, &delivery.cursor.to_le_bytes());
    hash_bytes(
        &mut hash,
        match delivery.action {
            ProjectionDeliveryAction::Upsert => b"upsert",
            ProjectionDeliveryAction::Delete => b"delete",
            ProjectionDeliveryAction::Rebuild => b"rebuild",
        },
    );
    hash_bytes(&mut hash, delivery.entity_uri.as_bytes());
    hash_bytes(&mut hash, delivery.payload_json.as_bytes());
    format!("fnv64:{hash:016x}")
}

fn projection_content_fingerprint(rows: &[ProjectionContentRow]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for row in rows {
        hash_bytes(&mut hash, row.key.as_bytes());
        hash_bytes(&mut hash, row.content_json.as_bytes());
        match &row.vector_bits {
            Some(vector_bits) => {
                hash_bytes(&mut hash, &[1]);
                hash_bytes(&mut hash, &vector_bits.len().to_le_bytes());
                for bits in vector_bits {
                    hash_bytes(&mut hash, &bits.to_le_bytes());
                }
            }
            None => hash_bytes(&mut hash, &[0]),
        }
    }
    format!("fnv64:{hash:016x}")
}

fn same_projection_content_rows(
    left: &[ProjectionContentRow],
    right: &[ProjectionContentRow],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.key == right.key
                && left.content_json == right.content_json
                && left.vector_bits == right.vector_bits
        })
}

fn current_time_ms() -> Result<i64, VectorProjectionBackendError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| VectorProjectionBackendError::Backend(error.to_string()))?
        .as_millis();
    i64::try_from(millis).map_err(|_| {
        VectorProjectionBackendError::Backend("system clock milliseconds overflowed i64".to_owned())
    })
}

fn persist_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let contents = serde_json::to_vec(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    durable_replace_file_contents(path, &contents)
}

fn lance_table_directory_name(
    store_name: &str,
) -> Result<&'static str, VectorProjectionBackendError> {
    match store_name {
        LANCEDB_CHUNKS_STORE => Ok("kb_chunks.lance"),
        LANCEDB_LABEL_ATOMS_STORE => Ok("kb_label_atoms.lance"),
        _ => Err(VectorProjectionBackendError::Protocol(format!(
            "unsupported LanceDB projection store: {store_name}"
        ))),
    }
}

fn require_real_directory(
    path: &Path,
    description: &str,
) -> Result<(), VectorProjectionBackendError> {
    let metadata = fs::symlink_metadata(path).map_err(backend_io)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(VectorProjectionBackendError::Backend(format!(
            "{description} is not a real directory: {}",
            path.display()
        )));
    }
    Ok(())
}

fn path_exists(path: &Path) -> Result<bool, VectorProjectionBackendError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(backend_io(error)),
    }
}

fn map_vector_error(error: VectorError) -> VectorProjectionBackendError {
    match error {
        VectorError::Provider { message, retryable } => {
            VectorProjectionBackendError::Provider { message, retryable }
        }
        VectorError::DimensionMismatch { expected, actual } => {
            VectorProjectionBackendError::Provider {
                message: format!("embedding dimension mismatch: expected {expected}, got {actual}"),
                retryable: false,
            }
        }
        VectorError::MissingEmbeddingProvider => VectorProjectionBackendError::Provider {
            message: "embedding provider is not configured".to_owned(),
            retryable: false,
        },
        VectorError::EmbeddingModelMismatch { expected, actual } => {
            VectorProjectionBackendError::Delivery(format!(
                "embedding model mismatch: expected {expected}, got {actual}"
            ))
        }
        VectorError::Chunk(message) => VectorProjectionBackendError::Delivery(message),
        VectorError::Disabled => {
            VectorProjectionBackendError::Backend("vector store is disabled".to_owned())
        }
        VectorError::ProjectionHelper(error) => {
            VectorProjectionBackendError::Backend(error.to_string())
        }
        VectorError::Store(message) => VectorProjectionBackendError::Backend(message),
    }
}

fn vector_backend_io(error: std::io::Error) -> VectorError {
    VectorError::Store(error.to_string())
}

fn delivery_json(error: serde_json::Error) -> VectorProjectionBackendError {
    VectorProjectionBackendError::Delivery(format!(
        "canonical snapshot payload is invalid: {error}"
    ))
}

fn backend_sql(error: rusqlite::Error) -> VectorProjectionBackendError {
    VectorProjectionBackendError::Backend(error.to_string())
}

fn derived_backend(error: impl std::fmt::Display) -> VectorProjectionBackendError {
    VectorProjectionBackendError::Backend(error.to_string())
}

fn projection_status_from_base(
    conn: &Connection,
    store_name: &str,
    board_id: &str,
    generation: &str,
    mut status: VectorStoreStatus,
) -> Result<VectorStoreStatus, VectorProjectionBackendError> {
    status
        .diagnostics
        .push(format!("projection_generation={generation}"));
    push_status_diagnostic(&mut status, "projection_v2_active");
    status.message = format!("{}; active_generation={generation}", status.message);
    match store_name {
        LANCEDB_CHUNKS_STORE => {
            let state =
                derived_status_by_name(conn, LANCEDB_CHUNKS_STORE).map_err(derived_backend)?;
            let current_last_event_id =
                current_last_event_id(conn, board_id).map_err(derived_backend)?;
            let board_dirty =
                has_pending_vector_outbox_for_board(conn, board_id, current_last_event_id)
                    .map_err(derived_backend)?;
            status.dirty = Some(state.dirty);
            status.board_dirty = Some(board_dirty);
            if !status.enabled {
                push_status_diagnostic(&mut status, "vector_store_disabled");
            }
            if state.dirty {
                push_status_diagnostic(&mut status, "vector_dirty");
            }
            if board_dirty {
                push_status_diagnostic(&mut status, "vector_board_dirty");
            }
            if state.last_error.is_some() {
                push_status_diagnostic(&mut status, "vector_error");
            }
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
        LANCEDB_LABEL_ATOMS_STORE => {
            push_status_diagnostic(&mut status, "label_atom_helper");
            label_atom_index_status_from_base(conn, board_id, status).map_err(derived_backend)
        }
        _ => Err(VectorProjectionBackendError::Protocol(format!(
            "unsupported LanceDB projection store: {store_name}"
        ))),
    }
}

fn push_status_diagnostic(status: &mut VectorStoreStatus, code: &str) {
    if !status.diagnostics.iter().any(|value| value == code) {
        status.diagnostics.push(code.to_owned());
    }
}

fn read_database_instance_id(conn: &Connection) -> Result<String, VectorProjectionBackendError> {
    let (database_instance_id, protocol_version) = conn
        .query_row(
            "SELECT database_instance_id,protocol_version
             FROM projection_database
             WHERE singleton=1
               AND (SELECT COUNT(*) FROM projection_database)=1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(|error| VectorProjectionBackendError::Backend(error.to_string()))?;
    if !database_instance_id.starts_with("db_")
        || protocol_version != VECTOR_PROJECTION_PROTOCOL_VERSION
    {
        return Err(VectorProjectionBackendError::Protocol(
            "projection database identity or protocol version is invalid".to_owned(),
        ));
    }
    Ok(database_instance_id)
}

fn validate_record_coverage(
    snapshot: &ProjectionSnapshot,
) -> Result<(), VectorProjectionBackendError> {
    let mut previous: Option<(&str, &str)> = None;
    for record in &snapshot.records {
        if record.board_id.trim().is_empty()
            || !record.identity.starts_with("kb://")
            || serde_json::from_str::<serde_json::Value>(&record.payload_json).is_err()
            || record.content_hash != stable_bytes_hash(record.payload_json.as_bytes())
        {
            return Err(VectorProjectionBackendError::Delivery(
                "projection snapshot contains an invalid canonical record".to_owned(),
            ));
        }
        let key = (record.board_id.as_str(), record.identity.as_str());
        if previous.is_some_and(|previous| previous >= key) {
            return Err(VectorProjectionBackendError::Delivery(
                "projection snapshot records must be uniquely sorted by board and identity"
                    .to_owned(),
            ));
        }
        previous = Some(key);
    }
    let (count, digest) = snapshot_coverage(snapshot);
    if snapshot.manifest.canonical_item_count != count
        || snapshot.manifest.canonical_digest != digest
    {
        return Err(VectorProjectionBackendError::Delivery(
            "projection snapshot canonical coverage does not match its records".to_owned(),
        ));
    }
    Ok(())
}

fn snapshot_coverage(snapshot: &ProjectionSnapshot) -> (i64, String) {
    let mut hash = 0xcbf29ce484222325_u64;
    for record in &snapshot.records {
        hash_bytes(&mut hash, record.board_id.as_bytes());
        hash_bytes(&mut hash, record.identity.as_bytes());
        hash_bytes(&mut hash, record.payload_json.as_bytes());
        hash_bytes(&mut hash, record.content_hash.as_bytes());
    }
    (snapshot.records.len() as i64, format!("fnv64:{hash:016x}"))
}

fn stable_bytes_hash(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    hash_bytes(&mut hash, bytes);
    format!("fnv64:{hash:016x}")
}

fn snapshot_fingerprint(snapshot: &ProjectionSnapshot) -> String {
    let manifest = &snapshot.manifest;
    let mut hash = 0xcbf29ce484222325_u64;
    for bytes in [
        manifest.store_name.as_bytes(),
        manifest.database_instance_id.as_bytes(),
        manifest.generation.as_bytes(),
        manifest.provider.as_bytes(),
        manifest.provider_fingerprint.as_bytes(),
        manifest.canonical_digest.as_bytes(),
        manifest.delivery_digest.as_bytes(),
    ] {
        hash_bytes(&mut hash, bytes);
    }
    hash_bytes(&mut hash, &manifest.protocol_version.to_le_bytes());
    hash_bytes(&mut hash, &manifest.schema_version.to_le_bytes());
    hash_bytes(&mut hash, &manifest.fence_epoch.to_le_bytes());
    hash_bytes(&mut hash, &manifest.snapshot_cursor.to_le_bytes());
    hash_bytes(&mut hash, &manifest.canonical_item_count.to_le_bytes());
    hash_bytes(&mut hash, &manifest.delivery_item_count.to_le_bytes());
    if let Some(corpus) = &manifest.corpus {
        hash_bytes(&mut hash, corpus.corpus_schema.as_bytes());
        hash_bytes(&mut hash, corpus.corpus_fingerprint.as_bytes());
        hash_bytes(&mut hash, corpus.embedding_model.as_bytes());
        hash_bytes(
            &mut hash,
            &(corpus.embedding_dimensions as u64).to_le_bytes(),
        );
    }
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

fn marker_contents(evidence: &ProjectionArtifactEvidence) -> Vec<u8> {
    format!(
        "database_instance_id={}\nstore_name={}\ngeneration={}\nfence_epoch={}\nfingerprint={}\n",
        evidence.manifest.database_instance_id,
        evidence.manifest.store_name,
        evidence.manifest.generation,
        evidence.manifest.fence_epoch,
        evidence.fingerprint
    )
    .into_bytes()
}

fn checked_generation_path(
    generations: &Path,
    generation_id: &str,
) -> Result<PathBuf, VectorProjectionBackendError> {
    projection_generation_path(generations, generation_id).map_err(backend_io)
}

fn generation_path_id(generation_path: &Path) -> Result<&str, VectorProjectionBackendError> {
    generation_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            VectorProjectionBackendError::Backend(
                "generation path has no UTF-8 directory name".to_owned(),
            )
        })
}

fn require_regular_file(path: &Path) -> Result<(), VectorProjectionBackendError> {
    let metadata = fs::symlink_metadata(path).map_err(backend_io)?;
    if !metadata.is_file() {
        return Err(VectorProjectionBackendError::Backend(format!(
            "projection metadata path is not a regular file: {}",
            path.display()
        )));
    }
    Ok(())
}

fn marker_exists(generation_path: &Path) -> Result<bool, VectorProjectionBackendError> {
    match fs::symlink_metadata(generation_path.join(PUBLISHED_MARKER)) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(backend_io(error)),
    }
}

fn path_is_directory_or_missing(path: &Path) -> Result<bool, VectorProjectionBackendError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(true),
        Ok(_) => Err(VectorProjectionBackendError::Backend(format!(
            "projection generations root is not a directory: {}",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(backend_io(error)),
    }
}

fn sorted_entries(path: &Path) -> Result<Vec<fs::DirEntry>, VectorProjectionBackendError> {
    let mut entries = fs::read_dir(path)
        .map_err(backend_io)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(backend_io)?;
    entries.sort_by_key(fs::DirEntry::file_name);
    Ok(entries)
}

fn validate_cleanup_protection(
    protection: &VectorProjectionCleanupProtection,
) -> Result<(), VectorProjectionBackendError> {
    let mut generations = BTreeSet::new();
    for generation in protection
        .active_generation
        .iter()
        .chain(protection.previous_generation.iter())
        .chain(protection.building_generation.iter())
        .chain(protection.additional_generations.iter())
    {
        if !generation.starts_with("gen_") || !generations.insert(generation.as_str()) {
            return Err(VectorProjectionBackendError::Protocol(
                "cleanup protection contains an invalid or duplicate generation".to_owned(),
            ));
        }
    }
    Ok(())
}

fn protect_optional(
    protected: &mut BTreeMap<String, VectorProjectionProtectionReason>,
    generation: Option<&str>,
    reason: VectorProjectionProtectionReason,
) {
    if let Some(generation) = generation {
        protected.entry(generation.to_owned()).or_insert(reason);
    }
}

fn protect_evidence(
    protected: &mut BTreeMap<String, VectorProjectionProtectionReason>,
    evidence: Option<&ProjectionArtifactEvidence>,
    reason: VectorProjectionProtectionReason,
) {
    if let Some(evidence) = evidence {
        protected
            .entry(evidence.manifest.generation.clone())
            .or_insert(reason);
    }
}

fn ack(context: &VectorProjectionMutationContext) -> VectorProjectionMutationAck {
    VectorProjectionMutationAck {
        request_id: context.request_id.clone(),
        projection_store: context.projection_store.clone(),
        generation_id: context.generation_id.clone(),
        delivery_digest: context.delivery_digest.clone(),
    }
}

fn require_non_empty(value: &str, field: &str) -> Result<(), VectorProjectionBackendError> {
    if value.trim().is_empty() {
        return Err(VectorProjectionBackendError::Protocol(format!(
            "{field} must not be empty"
        )));
    }
    Ok(())
}

fn backend_io(error: std::io::Error) -> VectorProjectionBackendError {
    VectorProjectionBackendError::Backend(error.to_string())
}

fn backend_json(error: serde_json::Error) -> VectorProjectionBackendError {
    VectorProjectionBackendError::Backend(error.to_string())
}

fn request_correlation(
    request: &VectorProjectionHelperRequest,
) -> (&str, Option<&str>, Option<&str>, Option<&str>) {
    match request {
        VectorProjectionHelperRequest::Descriptor(request) => {
            (&request.request_id, None, None, None)
        }
        VectorProjectionHelperRequest::PrepareSnapshot(request) => correlation(&request.context),
        VectorProjectionHelperRequest::ApplyBatch(request) => correlation(&request.context),
        VectorProjectionHelperRequest::Publish(request) => correlation(&request.context),
        VectorProjectionHelperRequest::InspectActive(request) => (
            &request.request_id,
            Some(&request.projection_store),
            None,
            None,
        ),
        VectorProjectionHelperRequest::InspectGeneration(request) => (
            &request.request_id,
            Some(&request.projection_store),
            Some(&request.generation_id),
            None,
        ),
        VectorProjectionHelperRequest::ValidateGenerationPublication(request) => (
            &request.request_id,
            Some(&request.projection_store),
            Some(&request.expected.manifest.generation),
            Some(&request.expected.manifest.delivery_digest),
        ),
        VectorProjectionHelperRequest::ValidateActiveContents(request) => (
            &request.request_id,
            Some(&request.projection_store),
            Some(&request.active.manifest.generation),
            Some(&request.active.manifest.delivery_digest),
        ),
        VectorProjectionHelperRequest::RepairPublication(request) => correlation(&request.context),
        VectorProjectionHelperRequest::Quarantine(request)
        | VectorProjectionHelperRequest::Abort(request) => correlation(&request.context),
        VectorProjectionHelperRequest::Inventory(request) => (
            &request.request_id,
            Some(&request.projection_store),
            None,
            None,
        ),
        VectorProjectionHelperRequest::Cleanup(request) => correlation(&request.context),
    }
}

fn correlation(
    context: &VectorProjectionMutationContext,
) -> (&str, Option<&str>, Option<&str>, Option<&str>) {
    (
        &context.request_id,
        Some(&context.projection_store),
        Some(&context.generation_id),
        Some(&context.delivery_digest),
    )
}

#[cfg(test)]
mod tests {
    use super::{projection_content_fingerprint, same_projection_content_rows};
    use crate::lancedb_store::ProjectionContentRow;

    #[test]
    fn physical_content_fingerprint_includes_vector_bits() {
        let first = vec![ProjectionContentRow {
            key: "kb://chunk/task/task-1/0".to_owned(),
            content_json: r#"{"text":"same"}"#.to_owned(),
            vector_bits: Some(vec![1.0_f32.to_bits(), 2.0_f32.to_bits()]),
        }];
        let changed = vec![ProjectionContentRow {
            vector_bits: Some(vec![1.0_f32.to_bits(), 3.0_f32.to_bits()]),
            ..first[0].clone()
        }];
        let expected_without_vector = vec![ProjectionContentRow {
            vector_bits: None,
            ..first[0].clone()
        }];

        assert!(!same_projection_content_rows(
            &first,
            &expected_without_vector
        ));
        assert_ne!(
            projection_content_fingerprint(&first),
            projection_content_fingerprint(&changed)
        );
    }
}
