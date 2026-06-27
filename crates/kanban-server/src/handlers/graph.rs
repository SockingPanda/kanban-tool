use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};
use kanban_entity::EntityUri;
use serde_json::json;

use crate::dto::{BoardQuery, Envelope, GraphNeighborsQuery};
use crate::error::{ApiError, extractor_error, invalid_input, validate_page_bounds};
use crate::helper::{HelperKind, helper_degraded_message, run_helper_json};
use crate::state::AppState;

use super::shared::parse_predicate;

pub(crate) async fn graph_status(
    State(state): State<AppState>,
    query: Result<Query<BoardQuery>, QueryRejection>,
) -> Result<Json<Envelope<kanban_graph::GraphStoreStatus>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let args = graph_helper_args(&state, &query.board, &["status".to_owned()]);
    let status =
        match run_helper_json::<kanban_graph::GraphStoreStatus>(state, HelperKind::Graph, args)
            .await
        {
            Ok(status) => status,
            Err(error) if error.is_status_degraded() => kanban_graph::GraphStoreStatus {
                backend: error.degraded_backend().to_owned(),
                enabled: false,
                message: helper_degraded_message(HelperKind::Graph, &error),
            },
            Err(error) => return Err(error.into()),
        };
    Ok(Json(Envelope {
        data: status,
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
    let mut args = vec![
        "neighbors".to_owned(),
        "--entity-uri".to_owned(),
        entity_uri.to_string(),
        "--limit".to_owned(),
        query.limit.to_string(),
    ];
    if let Some(predicate) = predicate {
        args.push("--predicate".to_owned());
        args.push(predicate.as_str().to_owned());
    }
    let args = graph_helper_args(&state, &query.board, &args);
    let data = match run_helper_json::<Vec<kanban_entity::Relation>>(state, HelperKind::Graph, args)
        .await
    {
        Ok(data) => data,
        Err(error) if error.is_helper_missing() => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    Ok(Json(Envelope {
        data,
        meta: Some(json!({ "limit": query.limit })),
    }))
}

fn graph_helper_args(state: &AppState, board: &str, command_args: &[String]) -> Vec<String> {
    let mut args = command_args.to_vec();
    args.push("--db".to_owned());
    args.push(state.db_path().display().to_string());
    args.push("--board".to_owned());
    args.push(board.to_owned());
    args
}
