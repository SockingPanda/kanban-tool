use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};

use crate::dto::{BoardQuery, Envelope};
use crate::error::{ApiError, extractor_error};
#[cfg(feature = "vector-lancedb")]
use crate::handlers::shared::configured_lancedb_store;
use crate::state::AppState;

pub(crate) async fn vector_status(
    State(state): State<AppState>,
    query: Result<Query<BoardQuery>, QueryRejection>,
) -> Result<Json<Envelope<kanban_vector::VectorStoreStatus>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    #[cfg(feature = "vector-lancedb")]
    if let Some(store) = configured_lancedb_store(&state)? {
        return Ok(Json(Envelope {
            data: kanban_sqlite::vector_store_status_with(state.db_path(), &query.board, &store)?,
            meta: None,
        }));
    }
    Ok(Json(Envelope {
        data: kanban_sqlite::vector_store_status(state.db_path(), &query.board)?,
        meta: None,
    }))
}
