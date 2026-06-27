use std::{path::PathBuf, sync::Arc};

use kanban_vector::EmbeddingProvider;

mod lancedb_store;
mod ollama;

pub use lancedb_store::LanceDbStore;
pub use ollama::OllamaEmbeddingProvider;

#[derive(Clone)]
pub struct LanceDbConfig {
    pub path: PathBuf,
    pub table_name: String,
    pub label_atom_table_name: String,
    pub provider: Option<Arc<dyn EmbeddingProvider + Send + Sync>>,
}

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
