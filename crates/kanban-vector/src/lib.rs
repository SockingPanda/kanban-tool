use kanban_entity::ChunkRef;
use serde::{Deserialize, Serialize};
#[cfg(feature = "vector-lancedb")]
use std::{path::PathBuf, sync::Arc};
use thiserror::Error;

#[cfg(feature = "vector-lancedb")]
mod lancedb_store;
#[cfg(feature = "vector-lancedb")]
pub use lancedb_store::LanceDbStore;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorStoreStatus {
    pub backend: String,
    pub enabled: bool,
    pub message: String,
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
pub struct VectorHit {
    pub chunk: ChunkRef,
    pub score: f32,
    pub text: Option<String>,
    pub summary: Option<String>,
}

pub trait EmbeddingProvider {
    fn embedding_model(&self) -> &str;
    fn dimensions(&self) -> usize;
    fn embed(&self, text: &str) -> Result<Vec<f32>, VectorError>;
}

pub trait VectorStore {
    fn status(&self) -> VectorStoreStatus;
    fn upsert(&self, chunks: &[EmbeddingChunk]) -> Result<(), VectorError>;
    fn query(&self, query: &VectorQuery) -> Result<Vec<VectorHit>, VectorError>;
}

#[derive(Debug, Clone, Default)]
pub struct DisabledVectorStore;

impl VectorStore for DisabledVectorStore {
    fn status(&self) -> VectorStoreStatus {
        VectorStoreStatus {
            backend: "disabled".to_owned(),
            enabled: false,
            message: "Vector store is disabled; context retrieval uses lexical fallback".to_owned(),
        }
    }

    fn upsert(&self, _chunks: &[EmbeddingChunk]) -> Result<(), VectorError> {
        Err(VectorError::Disabled)
    }

    fn query(&self, _query: &VectorQuery) -> Result<Vec<VectorHit>, VectorError> {
        Ok(Vec::new())
    }
}

#[cfg(feature = "vector-lancedb")]
#[derive(Clone)]
pub struct LanceDbConfig {
    pub path: PathBuf,
    pub table_name: String,
    pub provider: Option<Arc<dyn EmbeddingProvider + Send + Sync>>,
}

#[cfg(feature = "vector-lancedb")]
impl LanceDbConfig {
    pub fn new(
        path: impl Into<PathBuf>,
        provider: Arc<dyn EmbeddingProvider + Send + Sync>,
    ) -> Self {
        Self {
            path: path.into(),
            table_name: "kb_chunks".to_owned(),
            provider: Some(provider),
        }
    }

    pub fn degraded(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            table_name: "kb_chunks".to_owned(),
            provider: None,
        }
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
    #[error("chunk build error: {0}")]
    Chunk(String),
    #[error("vector store error: {0}")]
    Store(String),
}

#[cfg(test)]
mod tests {
    use super::{
        ChunkBuilder, DisabledVectorStore, TaskChunkSource, VectorError, VectorStore,
        ensure_dimensions,
    };

    #[test]
    fn disabled_vector_store_rejects_writes() {
        let store = DisabledVectorStore;

        assert!(matches!(store.upsert(&[]), Err(VectorError::Disabled)));
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
