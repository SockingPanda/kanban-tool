use kanban_entity::ChunkRef;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorStoreStatus {
    pub backend: String,
    pub enabled: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingChunk {
    pub chunk: ChunkRef,
    pub text: String,
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
}

pub trait EmbeddingProvider {
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

#[derive(Debug, Error)]
pub enum VectorError {
    #[error("vector store is disabled")]
    Disabled,
    #[error("vector store error: {0}")]
    Store(String),
}

#[cfg(test)]
mod tests {
    use super::{DisabledVectorStore, VectorError, VectorStore};

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
}
