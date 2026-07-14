use std::{path::PathBuf, sync::Arc};

use kanban_contract::{
    VectorHelperCheckProviderResponse, VectorHelperChunkHit, VectorHelperChunkRef,
    VectorHelperEmbedQueryResponse, VectorHelperErrorResponse, VectorHelperHandshakeResponse,
    VectorHelperLabelAtomHit, VectorHelperLabelAtomVectorHit, VectorHelperQueryChunksResponse,
    VectorHelperQueryLabelAtomsItem, VectorHelperQueryLabelAtomsResponse,
    VectorHelperStatusResponse,
};
use kanban_vector::EmbeddingProvider;
use kanban_vector::{LabelAtomHit, LabelAtomVectorHit, VectorHit, VectorStoreStatus};

mod lancedb_store;
mod ollama;

pub use lancedb_store::LanceDbStore;
pub use ollama::OllamaEmbeddingProvider;

pub fn vector_helper_handshake_response(version: &str) -> VectorHelperHandshakeResponse {
    VectorHelperHandshakeResponse {
        helper: "kanban-vector-lancedb".to_owned(),
        protocol: kanban_helper_protocol::HelperEnvelope::PROTOCOL.to_owned(),
        version: version.to_owned(),
    }
}

pub fn vector_helper_error_response(message: impl Into<String>) -> VectorHelperErrorResponse {
    VectorHelperErrorResponse {
        code: "helper_error".to_owned(),
        message: message.into(),
    }
}

pub fn vector_helper_check_provider_response() -> VectorHelperCheckProviderResponse {
    VectorHelperCheckProviderResponse { ok: true }
}

pub fn vector_helper_status_response(status: VectorStoreStatus) -> VectorHelperStatusResponse {
    VectorHelperStatusResponse {
        backend: status.backend,
        enabled: status.enabled,
        message: status.message,
        diagnostics: status.diagnostics,
        dirty: status.dirty,
        board_dirty: status.board_dirty,
        generation: status.generation,
    }
}

pub fn vector_helper_query_chunks_response(
    hits: Vec<VectorHit>,
) -> VectorHelperQueryChunksResponse {
    hits.into_iter()
        .map(|hit| VectorHelperChunkHit {
            chunk: VectorHelperChunkRef {
                uri: hit.chunk.uri.to_string(),
                entity_uri: hit.chunk.entity_uri.to_string(),
                ordinal: hit.chunk.ordinal,
                content_hash: hit.chunk.content_hash,
            },
            score: hit.score,
            text: hit.text,
            summary: hit.summary,
        })
        .collect()
}

fn vector_helper_label_atom_hit(hit: LabelAtomHit) -> VectorHelperLabelAtomHit {
    VectorHelperLabelAtomHit {
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
    }
}

pub fn vector_helper_query_label_atoms_response(
    hits: Vec<LabelAtomHit>,
) -> VectorHelperQueryLabelAtomsResponse {
    hits.into_iter()
        .map(vector_helper_label_atom_hit)
        .map(VectorHelperQueryLabelAtomsItem::Hit)
        .collect()
}

pub fn vector_helper_query_label_atom_vectors_response(
    hits: Vec<LabelAtomVectorHit>,
) -> VectorHelperQueryLabelAtomsResponse {
    hits.into_iter()
        .map(|hit| {
            VectorHelperQueryLabelAtomsItem::WithVector(VectorHelperLabelAtomVectorHit {
                hit: vector_helper_label_atom_hit(hit.hit),
                vector: hit.vector,
            })
        })
        .collect()
}

pub fn vector_helper_embed_query_response(vector: Vec<f32>) -> VectorHelperEmbedQueryResponse {
    vector
}

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
