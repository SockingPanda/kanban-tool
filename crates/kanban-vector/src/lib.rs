use kanban_entity::ChunkRef;
use serde::{Deserialize, Serialize};
#[cfg(feature = "vector-lancedb")]
use std::{path::PathBuf, sync::Arc};
use thiserror::Error;

pub const DEFAULT_EMBEDDING_MODEL: &str = "kb-local-default";

#[cfg(feature = "vector-lancedb")]
mod lancedb_store;
#[cfg(feature = "vector-lancedb")]
mod ollama;
#[cfg(feature = "vector-lancedb")]
pub use lancedb_store::LanceDbStore;
#[cfg(feature = "vector-lancedb")]
pub use ollama::OllamaEmbeddingProvider;

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

#[cfg(feature = "vector-lancedb")]
#[derive(Clone)]
pub struct LanceDbConfig {
    pub path: PathBuf,
    pub table_name: String,
    pub label_atom_table_name: String,
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
            label_atom_table_name: "kb_label_atoms".to_owned(),
            provider: Some(provider),
        }
    }

    pub fn degraded(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            table_name: "kb_chunks".to_owned(),
            label_atom_table_name: "kb_label_atoms".to_owned(),
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

    #[cfg(feature = "vector-lancedb")]
    mod ollama_provider {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        use crate::{EmbeddingProvider, OllamaEmbeddingProvider, VectorError};

        fn mock_ollama(response: &'static str) -> String {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let endpoint = format!("http://{}", listener.local_addr().unwrap());
            thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_http_request(&mut stream);
                assert!(request.starts_with("POST /api/embed "));
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    response.len(),
                    response
                )
                .unwrap();
            });
            endpoint
        }

        fn mock_ollama_with_request(
            response: &'static str,
        ) -> (String, thread::JoinHandle<String>) {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let endpoint = format!("http://{}", listener.local_addr().unwrap());
            let handle = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_http_request(&mut stream);
                assert!(request.starts_with("POST /api/embed "));
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    response.len(),
                    response
                )
                .unwrap();
                request
            });
            (endpoint, handle)
        }

        fn read_http_request(stream: &mut std::net::TcpStream) -> String {
            let mut buffer = Vec::new();
            let mut chunk = [0; 1024];
            let header_end = loop {
                let read = stream.read(&mut chunk).unwrap();
                assert!(read > 0);
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
                    break position + 4;
                }
            };
            let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            while buffer.len() < header_end + content_length {
                let read = stream.read(&mut chunk).unwrap();
                assert!(read > 0);
                buffer.extend_from_slice(&chunk[..read]);
            }
            String::from_utf8_lossy(&buffer).to_string()
        }

        #[test]
        fn ollama_provider_reads_first_embedding() {
            let endpoint = mock_ollama(r#"{"embeddings":[[0.1,0.2,0.3]]}"#);
            let provider = OllamaEmbeddingProvider::new(endpoint, "test-model", 3).unwrap();

            assert_eq!(provider.embedding_model(), "test-model");
            assert_eq!(provider.embed("short text").unwrap(), vec![0.1, 0.2, 0.3]);
        }

        #[test]
        fn ollama_provider_sends_dimensions_in_embed_request() {
            let (endpoint, request) = mock_ollama_with_request(r#"{"embeddings":[[0.1,0.2,0.3]]}"#);
            let provider = OllamaEmbeddingProvider::new(endpoint, "test-model", 3).unwrap();

            assert_eq!(provider.embed("short text").unwrap(), vec![0.1, 0.2, 0.3]);
            let request = request.join().unwrap();
            let body = request.split("\r\n\r\n").nth(1).unwrap();
            let body: serde_json::Value = serde_json::from_str(body).unwrap();
            assert_eq!(body["model"], "test-model");
            assert_eq!(body["input"], "short text");
            assert_eq!(body["dimensions"], 3);
        }

        #[test]
        fn ollama_provider_rejects_dimension_mismatch() {
            let endpoint = mock_ollama(r#"{"embeddings":[[0.1,0.2]]}"#);
            let provider = OllamaEmbeddingProvider::new(endpoint, "test-model", 3).unwrap();

            assert!(matches!(
                provider.embed("short text"),
                Err(VectorError::DimensionMismatch {
                    expected: 3,
                    actual: 2
                })
            ));
        }

        #[test]
        fn ollama_provider_maps_error_responses_to_store_errors() {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let endpoint = format!("http://{}", listener.local_addr().unwrap());
            thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 2048];
                let _ = stream.read(&mut request).unwrap();
                let response = r#"{"error":"model not found"}"#;
                write!(
                    stream,
                    "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    response.len(),
                    response
                )
                .unwrap();
            });
            let provider = OllamaEmbeddingProvider::new(endpoint, "test-model", 3).unwrap();

            assert!(matches!(
                provider.embed("short text"),
                Err(VectorError::Store(_))
            ));
        }
    }
}
