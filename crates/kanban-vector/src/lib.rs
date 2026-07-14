use kanban_contract::{
    VectorHelperEmbedQueryResponse, VectorHelperErrorResponse, VectorHelperLabelAtomHit,
    VectorHelperQueryChunksResponse, VectorHelperQueryLabelAtomsItem,
    VectorHelperQueryLabelAtomsResponse, VectorHelperStatusResponse,
};
use kanban_entity::{ChunkRef, EntityUri};
use kanban_helper_protocol::HelperEnvelope;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use thiserror::Error;

pub const DEFAULT_EMBEDDING_MODEL: &str = "kb-local-default";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorStoreStatus {
    pub backend: String,
    pub enabled: bool,
    pub message: String,
    #[serde(default)]
    pub diagnostics: Vec<String>,
    #[serde(default)]
    pub dirty: Option<bool>,
    #[serde(default)]
    pub board_dirty: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<i64>,
}

impl VectorStoreStatus {
    pub fn new(backend: impl Into<String>, enabled: bool, message: impl Into<String>) -> Self {
        Self {
            backend: backend.into(),
            enabled,
            message: message.into(),
            diagnostics: Vec::new(),
            dirty: None,
            board_dirty: None,
            generation: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingChunk {
    pub chunk: ChunkRef,
    pub kind: String,
    pub project_id: Option<String>,
    pub board_id: Option<String>,
    pub task_id: Option<String>,
    pub source_table: String,
    pub source_id: String,
    pub text: String,
    pub summary: Option<String>,
    pub embedding_model: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub source_event_id: Option<i64>,
    pub metadata_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelAtomVector {
    pub atom_id: String,
    pub label_id: String,
    pub label_name: String,
    pub board_id: String,
    pub polarity: String,
    pub kind: String,
    pub text: String,
    pub ordinal: i64,
    pub content_hash: String,
    pub embedding_model: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl LabelAtomVector {
    pub fn atom_key(&self) -> String {
        atom_key(&self.atom_id, &self.embedding_model)
    }
}

pub fn atom_key(atom_id: &str, embedding_model: &str) -> String {
    format!("{atom_id}#{embedding_model}")
}

impl EmbeddingChunk {
    pub fn chunk_key(&self) -> String {
        chunk_key(self.chunk.uri.as_str(), &self.embedding_model)
    }
}

pub fn chunk_key(chunk_uri: &str, embedding_model: &str) -> String {
    format!("{chunk_uri}#{embedding_model}")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskChunkSource {
    pub task_uri: String,
    pub project_id: Option<String>,
    pub board_id: Option<String>,
    pub task_id: String,
    pub title: String,
    pub description: Option<String>,
    pub source_event_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkBuilder {
    embedding_model: String,
}

impl ChunkBuilder {
    pub fn new(embedding_model: impl Into<String>) -> Self {
        Self {
            embedding_model: embedding_model.into(),
        }
    }

    pub fn build_task_chunks(
        &self,
        task: &TaskChunkSource,
    ) -> Result<Vec<EmbeddingChunk>, VectorError> {
        let task_uri = kanban_entity::EntityUri::new(task.task_uri.clone())
            .map_err(|err| VectorError::Chunk(err.to_string()))?;
        let text = match task.description.as_deref().map(str::trim) {
            Some(description) if !description.is_empty() => {
                format!("{}\n\n{}", task.title.trim(), description)
            }
            _ => task.title.trim().to_owned(),
        };
        if text.is_empty() {
            return Ok(Vec::new());
        }
        let chunk_uri =
            kanban_entity::EntityUri::new(format!("kb://chunk/task/{}/0", task.task_id))
                .map_err(|err| VectorError::Chunk(err.to_string()))?;
        Ok(vec![EmbeddingChunk {
            chunk: ChunkRef {
                uri: chunk_uri,
                entity_uri: task_uri,
                ordinal: 0,
                content_hash: Some(stable_hash(&text)),
            },
            kind: "task".to_owned(),
            project_id: task.project_id.clone(),
            board_id: task.board_id.clone(),
            task_id: Some(task.task_id.clone()),
            source_table: "tasks".to_owned(),
            source_id: task.task_id.clone(),
            text,
            summary: Some(task.title.trim().to_owned()),
            embedding_model: self.embedding_model.clone(),
            created_at: task.created_at,
            updated_at: task.updated_at,
            source_event_id: task.source_event_id,
            metadata_json: serde_json::json!({ "updated_at": task.updated_at }).to_string(),
        }])
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorQuery {
    pub text: String,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomQuery {
    pub text: String,
    pub limit: usize,
    pub board_id: Option<String>,
    pub embedding_model: Option<String>,
    pub polarity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomVectorQuery {
    pub vector: Vec<f32>,
    pub limit: usize,
    pub board_id: Option<String>,
    pub embedding_model: Option<String>,
    pub polarity: Option<String>,
    pub include_vector: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorHit {
    pub chunk: ChunkRef,
    pub score: f32,
    pub text: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomHit {
    pub atom_id: String,
    pub label_id: String,
    pub label_name: String,
    pub board_id: String,
    pub polarity: String,
    pub kind: String,
    pub text: String,
    pub ordinal: i64,
    pub content_hash: String,
    pub embedding_model: String,
    pub distance: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomVectorHit {
    pub hit: LabelAtomHit,
    pub vector: Option<Vec<f32>>,
}

pub trait EmbeddingProvider {
    fn embedding_model(&self) -> &str;
    fn dimensions(&self) -> usize;
    fn embed(&self, text: &str) -> Result<Vec<f32>, VectorError>;
}

pub trait VectorStoreBackend {
    fn embedding_model(&self) -> &str {
        DEFAULT_EMBEDDING_MODEL
    }
    fn status(&self) -> VectorStoreStatus;
}

pub trait QueryEmbeddingProvider: VectorStoreBackend {
    fn embed_query_text(&self, _text: &str) -> Result<Vec<f32>, VectorError> {
        Err(VectorError::Disabled)
    }
}

pub trait ChunkVectorStore: VectorStoreBackend {
    fn chunk_embedding_model(&self) -> &str {
        self.embedding_model()
    }
    fn delete_board(&self, board_id: &str) -> Result<(), VectorError>;
    fn delete_entities(&self, entity_uris: &[String]) -> Result<(), VectorError>;
    fn upsert(&self, chunks: &[EmbeddingChunk]) -> Result<(), VectorError>;
    fn query(&self, query: &VectorQuery) -> Result<Vec<VectorHit>, VectorError>;
}

pub trait LabelAtomVectorStore: QueryEmbeddingProvider {
    fn delete_label_atoms_for_board(&self, _board_id: &str) -> Result<(), VectorError> {
        Err(VectorError::Disabled)
    }
    fn upsert_label_atoms(&self, _atoms: &[LabelAtomVector]) -> Result<(), VectorError> {
        Err(VectorError::Disabled)
    }
    fn query_label_atoms(&self, _query: &LabelAtomQuery) -> Result<Vec<LabelAtomHit>, VectorError> {
        Ok(Vec::new())
    }
    fn query_label_atoms_by_vector(
        &self,
        _query: &LabelAtomVectorQuery,
    ) -> Result<Vec<LabelAtomVectorHit>, VectorError> {
        Ok(Vec::new())
    }
}

pub trait VectorStore: ChunkVectorStore + LabelAtomVectorStore {}

impl<T> VectorStore for T where T: ChunkVectorStore + LabelAtomVectorStore + ?Sized {}

#[derive(Debug, Clone, Default)]
pub struct DisabledVectorStore;

impl VectorStoreBackend for DisabledVectorStore {
    fn status(&self) -> VectorStoreStatus {
        VectorStoreStatus::new(
            "disabled",
            false,
            "Vector store is disabled; context retrieval uses lexical fallback",
        )
    }
}

impl QueryEmbeddingProvider for DisabledVectorStore {}

impl ChunkVectorStore for DisabledVectorStore {
    fn upsert(&self, _chunks: &[EmbeddingChunk]) -> Result<(), VectorError> {
        Err(VectorError::Disabled)
    }

    fn delete_board(&self, _board_id: &str) -> Result<(), VectorError> {
        Err(VectorError::Disabled)
    }

    fn delete_entities(&self, _entity_uris: &[String]) -> Result<(), VectorError> {
        Err(VectorError::Disabled)
    }

    fn query(&self, _query: &VectorQuery) -> Result<Vec<VectorHit>, VectorError> {
        Ok(Vec::new())
    }
}

impl LabelAtomVectorStore for DisabledVectorStore {}

#[derive(Debug, Clone)]
pub struct SubprocessVectorStore {
    helper_path: PathBuf,
    db_path: PathBuf,
    board: String,
    vector_config_path: Option<PathBuf>,
    embedding_model: Option<String>,
}

impl SubprocessVectorStore {
    pub fn new(
        helper_path: impl Into<PathBuf>,
        db_path: impl Into<PathBuf>,
        board: impl Into<String>,
        vector_config_path: Option<PathBuf>,
    ) -> Self {
        Self {
            helper_path: helper_path.into(),
            db_path: db_path.into(),
            board: board.into(),
            vector_config_path,
            embedding_model: None,
        }
    }

    pub fn with_embedding_model(mut self, embedding_model: impl Into<String>) -> Self {
        self.embedding_model = Some(embedding_model.into());
        self
    }

    fn helper_args(&self, command_args: &[String]) -> Vec<String> {
        let mut args = command_args.to_vec();
        args.push("--db".to_owned());
        args.push(self.db_path.display().to_string());
        args.push("--board".to_owned());
        args.push(self.board.clone());
        if let Some(path) = &self.vector_config_path {
            args.push("--vector-config".to_owned());
            args.push(path.display().to_string());
        }
        args
    }

    fn run_helper<T>(&self, command_args: &[String]) -> Result<T, VectorError>
    where
        T: DeserializeOwned,
    {
        let args = self.helper_args(command_args);
        let output = Command::new(&self.helper_path)
            .args(&args)
            .output()
            .map_err(|error| {
                VectorError::Store(format!(
                    "vector helper unavailable: failed to run {}: {}",
                    self.helper_path.display(),
                    error
                ))
            })?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() {
            if let Ok(envelope) = HelperEnvelope::from_json(stdout.trim())
                && let Ok(error) = envelope.decode::<VectorHelperErrorResponse>()
            {
                return Err(VectorError::Store(format!(
                    "vector helper failed: {} ({})",
                    error.message, error.code
                )));
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VectorError::Store(format!(
                "vector helper {} exited with status {:?}: {}",
                self.helper_path.display(),
                output.status.code(),
                bounded_helper_message(stderr.trim())
            )));
        }
        let envelope = HelperEnvelope::from_json(stdout.trim()).map_err(|error| {
            VectorError::Store(format!(
                "vector helper {} returned invalid JSON envelope: {}",
                self.helper_path.display(),
                error
            ))
        })?;
        envelope.decode::<T>().map_err(|error| {
            VectorError::Store(format!(
                "vector helper {} returned an invalid payload: {}",
                self.helper_path.display(),
                error
            ))
        })
    }

    pub fn label_atom_status(&self) -> VectorStoreStatus {
        match self
            .run_helper::<VectorHelperStatusResponse>(&["label-atoms-status".to_owned()])
            .map(vector_store_status)
        {
            Ok(status) => status,
            Err(error) => {
                let mut status = VectorStoreStatus::new(
                    "helper-missing",
                    false,
                    format!("vector helper unavailable: {error}"),
                );
                status.diagnostics.push("helper_missing".to_owned());
                status
                    .diagnostics
                    .push("label_atom_helper_missing".to_owned());
                status
            }
        }
    }

    pub fn rebuild_label_atoms(&self) -> Result<VectorStoreStatus, VectorError> {
        self.run_helper::<VectorHelperStatusResponse>(&["rebuild-label-atoms".to_owned()])
            .map(vector_store_status)
    }

    pub fn sync_label_atoms(&self) -> Result<VectorStoreStatus, VectorError> {
        self.run_helper::<VectorHelperStatusResponse>(&["sync-label-atoms".to_owned()])
            .map(vector_store_status)
    }
}

fn bounded_helper_message(value: &str) -> String {
    const MAX: usize = 240;
    let mut value = value.replace(['\r', '\n'], " ");
    if value.len() > MAX {
        value.truncate(MAX);
        value.push_str("...");
    }
    value
}

impl VectorStoreBackend for SubprocessVectorStore {
    fn embedding_model(&self) -> &str {
        self.embedding_model
            .as_deref()
            .unwrap_or(DEFAULT_EMBEDDING_MODEL)
    }

    fn status(&self) -> VectorStoreStatus {
        match self
            .run_helper::<VectorHelperStatusResponse>(&["status".to_owned()])
            .map(vector_store_status)
        {
            Ok(status) => status,
            Err(error) => {
                let mut status = VectorStoreStatus::new(
                    "helper-missing",
                    false,
                    format!("vector helper unavailable: {error}"),
                );
                status.diagnostics.push("helper_missing".to_owned());
                status
            }
        }
    }
}

impl QueryEmbeddingProvider for SubprocessVectorStore {
    fn embed_query_text(&self, text: &str) -> Result<Vec<f32>, VectorError> {
        self.run_helper::<VectorHelperEmbedQueryResponse>(&[
            "embed-query".to_owned(),
            "--text".to_owned(),
            text.to_owned(),
        ])
    }
}

impl ChunkVectorStore for SubprocessVectorStore {
    fn delete_board(&self, _board_id: &str) -> Result<(), VectorError> {
        Err(VectorError::Disabled)
    }

    fn delete_entities(&self, _entity_uris: &[String]) -> Result<(), VectorError> {
        Err(VectorError::Disabled)
    }

    fn upsert(&self, _chunks: &[EmbeddingChunk]) -> Result<(), VectorError> {
        Err(VectorError::Disabled)
    }

    fn query(&self, query: &VectorQuery) -> Result<Vec<VectorHit>, VectorError> {
        self.run_helper::<VectorHelperQueryChunksResponse>(&[
            "query-chunks".to_owned(),
            "--text".to_owned(),
            query.text.clone(),
            "--limit".to_owned(),
            query.limit.to_string(),
        ])?
        .into_iter()
        .map(vector_hit)
        .collect()
    }
}

impl LabelAtomVectorStore for SubprocessVectorStore {
    fn query_label_atoms(&self, query: &LabelAtomQuery) -> Result<Vec<LabelAtomHit>, VectorError> {
        let mut args = vec![
            "query-label-atoms".to_owned(),
            "--text".to_owned(),
            query.text.clone(),
            "--limit".to_owned(),
            query.limit.to_string(),
        ];
        push_label_atom_filters(
            &mut args,
            query.board_id.as_deref(),
            query.embedding_model.as_deref(),
            query.polarity.as_deref(),
        );
        self.run_helper::<VectorHelperQueryLabelAtomsResponse>(&args)?
            .into_iter()
            .map(|item| match item {
                VectorHelperQueryLabelAtomsItem::Hit(hit) => label_atom_hit(hit),
                VectorHelperQueryLabelAtomsItem::WithVector(hit) => label_atom_hit(hit.hit),
            })
            .collect()
    }

    fn query_label_atoms_by_vector(
        &self,
        query: &LabelAtomVectorQuery,
    ) -> Result<Vec<LabelAtomVectorHit>, VectorError> {
        let vector_json = serde_json::to_string(&query.vector)
            .map_err(|error| VectorError::Store(error.to_string()))?;
        let mut args = vec![
            "query-label-atoms".to_owned(),
            "--vector-json".to_owned(),
            vector_json,
            "--limit".to_owned(),
            query.limit.to_string(),
        ];
        push_label_atom_filters(
            &mut args,
            query.board_id.as_deref(),
            query.embedding_model.as_deref(),
            query.polarity.as_deref(),
        );
        if query.include_vector {
            args.push("--include-vector".to_owned());
        }
        self.run_helper::<VectorHelperQueryLabelAtomsResponse>(&args)?
            .into_iter()
            .map(|item| match item {
                VectorHelperQueryLabelAtomsItem::Hit(hit) => Ok(LabelAtomVectorHit {
                    hit: label_atom_hit(hit)?,
                    vector: None,
                }),
                VectorHelperQueryLabelAtomsItem::WithVector(hit) => Ok(LabelAtomVectorHit {
                    hit: label_atom_hit(hit.hit)?,
                    vector: hit.vector,
                }),
            })
            .collect()
    }
}

fn vector_store_status(status: VectorHelperStatusResponse) -> VectorStoreStatus {
    VectorStoreStatus {
        backend: status.backend,
        enabled: status.enabled,
        message: status.message,
        diagnostics: status.diagnostics,
        dirty: status.dirty,
        board_dirty: status.board_dirty,
        generation: status.generation,
    }
}

fn vector_hit(hit: kanban_contract::VectorHelperChunkHit) -> Result<VectorHit, VectorError> {
    Ok(VectorHit {
        chunk: ChunkRef {
            uri: EntityUri::new(hit.chunk.uri)
                .map_err(|error| VectorError::Store(error.to_string()))?,
            entity_uri: EntityUri::new(hit.chunk.entity_uri)
                .map_err(|error| VectorError::Store(error.to_string()))?,
            ordinal: hit.chunk.ordinal,
            content_hash: hit.chunk.content_hash,
        },
        score: hit.score,
        text: hit.text,
        summary: hit.summary,
    })
}

fn label_atom_hit(hit: VectorHelperLabelAtomHit) -> Result<LabelAtomHit, VectorError> {
    Ok(LabelAtomHit {
        atom_id: hit.atom_id,
        label_id: hit.label_id,
        label_name: hit.label_name,
        board_id: hit.board_id,
        polarity: hit.polarity,
        kind: hit.kind,
        text: hit.text,
        ordinal: hit.ordinal,
        content_hash: hit.content_hash,
        embedding_model: hit.embedding_model,
        distance: hit.distance,
    })
}

fn push_label_atom_filters(
    args: &mut Vec<String>,
    board_id: Option<&str>,
    embedding_model: Option<&str>,
    polarity: Option<&str>,
) {
    if let Some(board_id) = board_id {
        args.push("--board-id".to_owned());
        args.push(board_id.to_owned());
    }
    if let Some(embedding_model) = embedding_model {
        args.push("--embedding-model".to_owned());
        args.push(embedding_model.to_owned());
    }
    if let Some(polarity) = polarity {
        args.push("--polarity".to_owned());
        args.push(polarity.to_owned());
    }
}
pub fn ensure_dimensions(vector: &[f32], expected: usize) -> Result<(), VectorError> {
    if vector.len() == expected {
        Ok(())
    } else {
        Err(VectorError::DimensionMismatch {
            expected,
            actual: vector.len(),
        })
    }
}

fn stable_hash(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[derive(Debug, Error)]
pub enum VectorError {
    #[error("vector store is disabled")]
    Disabled,
    #[error("embedding provider is not configured")]
    MissingEmbeddingProvider,
    #[error("embedding dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
    #[error("embedding model mismatch: expected {expected}, got {actual}")]
    EmbeddingModelMismatch { expected: String, actual: String },
    #[error("chunk build error: {0}")]
    Chunk(String),
    #[error("vector store error: {0}")]
    Store(String),
}

#[cfg(test)]
mod tests {
    use super::{
        ChunkBuilder, ChunkVectorStore, DisabledVectorStore, LabelAtomVectorStore, TaskChunkSource,
        VectorError, VectorStore, VectorStoreBackend, ensure_dimensions,
    };

    fn assert_chunk_store<T: ChunkVectorStore>(_store: &T) {}
    fn assert_label_atom_store<T: LabelAtomVectorStore>(_store: &T) {}
    fn assert_vector_store<T: VectorStore>(_store: &T) {}

    #[test]
    fn disabled_store_implements_split_vector_traits() {
        let store = DisabledVectorStore;

        assert_chunk_store(&store);
        assert_label_atom_store(&store);
        assert_vector_store(&store);
    }

    #[test]
    fn disabled_vector_store_rejects_writes() {
        let store = DisabledVectorStore;

        assert!(matches!(store.upsert(&[]), Err(VectorError::Disabled)));
        assert!(matches!(
            store.delete_entities(&["kb://task/t_1".to_owned()]),
            Err(VectorError::Disabled)
        ));
        assert_eq!(
            store
                .query(&super::VectorQuery {
                    text: "x".into(),
                    limit: 1
                })
                .unwrap(),
            Vec::new()
        );
        assert!(!store.status().enabled);
    }

    #[test]
    fn chunk_builder_uses_chunk_uri_and_model_as_stable_key() {
        let builder = ChunkBuilder::new("test-model");
        let chunks = builder
            .build_task_chunks(&TaskChunkSource {
                task_uri: "kb://task/t_1".to_owned(),
                project_id: Some("project-a".to_owned()),
                board_id: Some("b_1".to_owned()),
                task_id: "t_1".to_owned(),
                title: "Title".to_owned(),
                description: Some("Spec body".to_owned()),
                source_event_id: Some(7),
                created_at: 41,
                updated_at: 42,
            })
            .unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk.uri.as_str(), "kb://chunk/task/t_1/0");
        assert_eq!(chunks[0].chunk.entity_uri.as_str(), "kb://task/t_1");
        assert_eq!(chunks[0].chunk_key(), "kb://chunk/task/t_1/0#test-model");
        assert_eq!(chunks[0].text, "Title\n\nSpec body");
        assert_eq!(chunks[0].source_table, "tasks");
        assert_eq!(chunks[0].source_id, "t_1");
        assert_eq!(chunks[0].source_event_id, Some(7));
    }

    #[test]
    fn dimension_mismatch_is_explicit_error() {
        let err = ensure_dimensions(&[1.0, 2.0], 3).unwrap_err();
        assert!(matches!(
            err,
            VectorError::DimensionMismatch {
                expected: 3,
                actual: 2
            }
        ));
    }
}
