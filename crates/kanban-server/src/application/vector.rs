//! Host-owned vector HTTP endpoints。
//!
//! provider 调用和 projection worker 由 `kanban-service` 持有；CLI、MCP 和 Desktop
//! 只能通过 localhost API 读取结果。
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{
    DataEnvelope, VectorChunkResult, VectorConfigureRequest, VectorConfigureResponse,
    VectorLabelAtomResult, VectorProjectionRequest, VectorProjectionResponse, VectorQuery,
    VectorQueryChunksResponse, VectorQueryLabelAtomsResponse, VectorStatus as VectorStatusProtocol,
    VectorStatusQuery, VectorStatusResponse,
};
use kanban_service::{
    VectorChunkQueryCommand, VectorConfigureCommand, VectorLabelAtomQueryCommand,
    VectorStatus as ServiceVectorStatus,
};
pub(crate) async fn status(
    state: AppState,
    query: VectorStatusQuery,
) -> Result<VectorStatusResponse, ApiError> {
    let value = state.application().vector_status(&query.board).await?;
    Ok(DataEnvelope::new(vector_status(value)))
}
pub(crate) async fn configure(
    state: AppState,
    body: VectorConfigureRequest,
) -> Result<VectorConfigureResponse, ApiError> {
    let command = VectorConfigureCommand {
        provider: body.provider.clone(),
        endpoint: body.endpoint.clone(),
        model: body.model.clone(),
        dimensions: body.dimensions,
    };
    state.application().configure_vector(command).await?;
    Ok(DataEnvelope::new(body))
}
pub(crate) async fn rebuild(
    state: AppState,
    body: VectorProjectionRequest,
) -> Result<VectorProjectionResponse, ApiError> {
    let value = state.application().rebuild_vector(&body.board).await?;
    Ok(DataEnvelope::new(vector_status(value)))
}
pub(crate) async fn sync(
    state: AppState,
    body: VectorProjectionRequest,
) -> Result<VectorProjectionResponse, ApiError> {
    let value = state.application().sync_vector(&body.board).await?;
    Ok(DataEnvelope::new(vector_status(value)))
}
pub(crate) async fn query_chunks(
    state: AppState,
    query: VectorQuery,
) -> Result<VectorQueryChunksResponse, ApiError> {
    let hits = state
        .application()
        .query_vector_chunks(VectorChunkQueryCommand {
            board: query.board,
            q: query.q,
            embedding_model: query.embedding_model,
            limit: query.limit,
        })
        .await?;
    Ok(DataEnvelope::new(
        hits.into_iter()
            .map(|hit| VectorChunkResult {
                id: hit.id,
                entity_uri: hit.entity_uri,
                source_kind: hit.source_kind,
                content: hit.content,
                content_hash: hit.content_hash,
                embedding_model: hit.embedding_model,
                distance: hit.distance,
                score: hit.score,
            })
            .collect(),
    ))
}
pub(crate) async fn query_label_atoms(
    state: AppState,
    query: VectorQuery,
) -> Result<VectorQueryLabelAtomsResponse, ApiError> {
    let hits = state
        .application()
        .query_vector_label_atoms(VectorLabelAtomQueryCommand {
            board: query.board,
            q: query.q,
            embedding_model: query.embedding_model,
            polarity: query.polarity,
            limit: query.limit,
            include_vector: query.include_vector,
        })
        .await?;
    Ok(DataEnvelope::new(
        hits.into_iter()
            .map(|hit| VectorLabelAtomResult {
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
                vector: hit.vector,
            })
            .collect(),
    ))
}
fn vector_status(value: ServiceVectorStatus) -> VectorStatusProtocol {
    VectorStatusProtocol {
        backend: value.backend,
        enabled: value.enabled,
        message: value.message,
        diagnostics: value.diagnostics,
        dirty: value.dirty,
        board_dirty: value.board_dirty,
        generation: value.generation,
    }
}
