use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};
use kanban_entity::EntityUri;
use serde_json::json;

use crate::dto::{BoardQuery, Envelope, GraphNeighborsQuery};
use crate::error::{ApiError, extractor_error, invalid_input, validate_page_bounds};
use crate::state::AppState;

use super::shared::parse_predicate;

pub(crate) async fn graph_status(
    State(state): State<AppState>,
    query: Result<Query<BoardQuery>, QueryRejection>,
) -> Result<Json<Envelope<kanban_graph::GraphStoreStatus>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    Ok(Json(Envelope {
        data: kanban_sqlite::graph_store_status(state.db_path(), &query.board)?,
        meta: None,
    }))
}

pub(crate) async fn graph_neighbors(
    State(state): State<AppState>,
    query: Result<Query<GraphNeighborsQuery>, QueryRejection>,
) -> Result<Json<Envelope<Vec<kanban_entity::Relation>>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(query.limit, kanban_sqlite::MAX_TASK_LIST_LIMIT, 0)?;
    let entity_uri =
        EntityUri::new(query.entity_uri).map_err(|error| invalid_input(error.to_string()))?;
    let predicate = query
        .predicate
        .as_deref()
        .map(parse_predicate)
        .transpose()?;
    Ok(Json(Envelope {
        data: kanban_sqlite::graph_neighbors(state.db_path(), &entity_uri, predicate, query.limit)?,
        meta: Some(json!({ "limit": query.limit })),
    }))
}
