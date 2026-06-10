use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};

use crate::dto::{BoardQuery, Envelope};
use crate::error::{ApiError, extractor_error};
use crate::state::AppState;

pub(crate) async fn vector_status(
    State(state): State<AppState>,
    query: Result<Query<BoardQuery>, QueryRejection>,
) -> Result<Json<Envelope<kanban_vector::VectorStoreStatus>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    Ok(Json(Envelope {
        data: kanban_sqlite::vector_store_status(state.db_path(), &query.board)?,
        meta: None,
    }))
}
