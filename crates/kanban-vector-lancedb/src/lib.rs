use std::{path::PathBuf, sync::Arc, time::Duration};

use kanban_contract::{
    VECTOR_PROJECTION_PROTOCOL_VERSION, VectorHelperCheckProviderResponse, VectorHelperChunkHit,
    VectorHelperChunkRef, VectorHelperEmbedQueryResponse, VectorHelperErrorResponse,
    VectorHelperHandshakeResponse, VectorHelperLabelAtomHit, VectorHelperLabelAtomVectorHit,
    VectorHelperQueryChunksResponse, VectorHelperQueryLabelAtomsItem,
    VectorHelperQueryLabelAtomsResponse, VectorHelperStatusResponse,
    VectorProjectionHelperDescriptor, VectorProjectionHelperError, VectorProjectionHelperErrorKind,
    VectorProjectionHelperOperation, VectorProjectionHelperRequest, VectorProjectionHelperResponse,
};
use kanban_vector::EmbeddingProvider;
use kanban_vector::{LabelAtomHit, LabelAtomVectorHit, VectorHit, VectorStoreStatus};

mod lancedb_store;
mod ollama;

pub use lancedb_store::LanceDbStore;
pub use ollama::OllamaEmbeddingProvider;

pub const VECTOR_HELPER_BUILD_IDENTITY: &str = match option_env!("KANBAN_BUILD_ID") {
    Some(build_id) => build_id,
    None => concat!(
        "dev:",
        env!("CARGO_PKG_NAME"),
        "@",
        env!("CARGO_PKG_VERSION")
    ),
};

pub const fn vector_helper_build_identity() -> &'static str {
    VECTOR_HELPER_BUILD_IDENTITY
}

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

pub fn decode_vector_projection_request(
    input: &[u8],
) -> Result<VectorProjectionHelperRequest, serde_json::Error> {
    serde_json::from_slice(input)
}

pub fn vector_projection_descriptor_response(
    request_id: impl Into<String>,
) -> VectorProjectionHelperResponse {
    VectorProjectionHelperResponse::Descriptor(VectorProjectionHelperDescriptor {
        request_id: request_id.into(),
        protocol_version: VECTOR_PROJECTION_PROTOCOL_VERSION,
        build_identity: vector_helper_build_identity().to_owned(),
        supported_stores: Vec::new(),
        supported_operations: vec![VectorProjectionHelperOperation::Descriptor],
    })
}

pub fn vector_projection_unavailable_response(
    request: &VectorProjectionHelperRequest,
) -> VectorProjectionHelperResponse {
    let (request_id, projection_store, generation_id, delivery_digest) =
        vector_projection_request_correlation(request);
    VectorProjectionHelperResponse::Error(VectorProjectionHelperError {
        kind: VectorProjectionHelperErrorKind::Backend,
        code: "projection_backend_unavailable".to_owned(),
        provider: None,
        backend: Some("lancedb".to_owned()),
        retryable: false,
        message: "the LanceDB Projection v2 generation backend is not enabled".to_owned(),
        request_id: Some(request_id.to_owned()),
        delivery_digest: delivery_digest.map(str::to_owned),
        projection_store: projection_store.map(str::to_owned),
        generation_id: generation_id.map(str::to_owned),
    })
}

pub fn vector_projection_invalid_request_response() -> VectorProjectionHelperResponse {
    VectorProjectionHelperResponse::Error(VectorProjectionHelperError {
        kind: VectorProjectionHelperErrorKind::Protocol,
        code: "invalid_request".to_owned(),
        provider: None,
        backend: Some("lancedb".to_owned()),
        retryable: false,
        message: "vector projection helper stdin is not a valid request".to_owned(),
        request_id: None,
        delivery_digest: None,
        projection_store: None,
        generation_id: None,
    })
}

fn vector_projection_request_correlation(
    request: &VectorProjectionHelperRequest,
) -> (&str, Option<&str>, Option<&str>, Option<&str>) {
    match request {
        VectorProjectionHelperRequest::Descriptor(request) => {
            (&request.request_id, None, None, None)
        }
        VectorProjectionHelperRequest::PrepareSnapshot(request) => (
            &request.context.request_id,
            Some(&request.context.projection_store),
            Some(&request.context.generation_id),
            Some(&request.context.delivery_digest),
        ),
        VectorProjectionHelperRequest::ApplyBatch(request) => (
            &request.context.request_id,
            Some(&request.context.projection_store),
            Some(&request.context.generation_id),
            Some(&request.context.delivery_digest),
        ),
        VectorProjectionHelperRequest::Publish(request) => (
            &request.context.request_id,
            Some(&request.context.projection_store),
            Some(&request.context.generation_id),
            Some(&request.context.delivery_digest),
        ),
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
        VectorProjectionHelperRequest::RepairPublication(request) => (
            &request.context.request_id,
            Some(&request.context.projection_store),
            Some(&request.context.generation_id),
            Some(&request.context.delivery_digest),
        ),
        VectorProjectionHelperRequest::Quarantine(request)
        | VectorProjectionHelperRequest::Abort(request) => (
            &request.context.request_id,
            Some(&request.context.projection_store),
            Some(&request.context.generation_id),
            Some(&request.context.delivery_digest),
        ),
        VectorProjectionHelperRequest::Inventory(request) => (
            &request.request_id,
            Some(&request.projection_store),
            None,
            None,
        ),
        VectorProjectionHelperRequest::Cleanup(request) => (
            &request.context.request_id,
            Some(&request.context.projection_store),
            Some(&request.context.generation_id),
            Some(&request.context.delivery_digest),
        ),
    }
}

#[derive(Clone)]
pub struct LanceDbConfig {
    pub path: PathBuf,
    pub table_name: String,
    pub label_atom_table_name: String,
    pub provider: Option<Arc<dyn EmbeddingProvider + Send + Sync>>,
    pub execution_policy: EmbeddingExecutionPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingExecutionPolicy {
    pub batch_size: usize,
    pub min_batch_interval: Duration,
    pub max_retries: usize,
    pub initial_retry_backoff: Duration,
    pub max_retry_backoff: Duration,
}

impl Default for EmbeddingExecutionPolicy {
    fn default() -> Self {
        Self {
            batch_size: 32,
            min_batch_interval: Duration::from_millis(25),
            max_retries: 4,
            initial_retry_backoff: Duration::from_millis(250),
            max_retry_backoff: Duration::from_secs(5),
        }
    }
}

impl EmbeddingExecutionPolicy {
    fn validate(&self) -> Result<(), kanban_vector::VectorError> {
        if self.batch_size == 0 {
            return Err(kanban_vector::VectorError::Store(
                "embedding batch size must be greater than zero".to_owned(),
            ));
        }
        if self.initial_retry_backoff > self.max_retry_backoff {
            return Err(kanban_vector::VectorError::Store(
                "embedding initial retry backoff cannot exceed maximum backoff".to_owned(),
            ));
        }
        Ok(())
    }
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
            execution_policy: EmbeddingExecutionPolicy::default(),
        }
    }

    pub fn with_execution_policy(mut self, policy: EmbeddingExecutionPolicy) -> Self {
        self.execution_policy = policy;
        self
    }

    pub fn degraded(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            table_name: "kb_chunks".to_owned(),
            label_atom_table_name: "kb_label_atoms".to_owned(),
            provider: None,
            execution_policy: EmbeddingExecutionPolicy::default(),
        }
    }
}
