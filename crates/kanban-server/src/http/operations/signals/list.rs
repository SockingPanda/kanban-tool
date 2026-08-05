use super::support::api_signal;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, Query, State, rejection::QueryRejection},
    routing::get,
};
use kanban_application::SignalListOptions as ApplicationSignalListOptions;
use kanban_core::KanbanError;
use kanban_protocol::{BoardLabelPath, MetadataEnvelope, SignalFilterMeta, SignalQuery};

pub(crate) async fn list_signals(
    State(state): State<AppState>,
    Path(BoardLabelPath { board }): Path<BoardLabelPath>,
    query: Result<Query<SignalQuery>, QueryRejection>,
) -> Result<Json<kanban_protocol::ListSignalsResponse>, ApiError> {
    let Query(query) = query
        .map_err(|error| KanbanError::InvalidInput(format!("invalid signal query: {error}")))?;
    let signals = state
        .application()
        .list_signals(&board, signal_options(&query)?)
        .await?
        .into_iter()
        .map(api_signal)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(MetadataEnvelope {
        data: signals,
        meta: SignalFilterMeta {
            include_all: query.include_all,
            limit: query.limit,
        },
    }))
}

pub(crate) async fn review_signals(
    State(state): State<AppState>,
    Path(BoardLabelPath { board }): Path<BoardLabelPath>,
    query: Result<Query<SignalQuery>, QueryRejection>,
) -> Result<Json<kanban_protocol::ReviewSignalsResponse>, ApiError> {
    let Query(mut query) = query
        .map_err(|error| KanbanError::InvalidInput(format!("invalid signal query: {error}")))?;
    query.include_all = false;
    if query.status.is_empty() {
        query.status = vec!["open".to_owned(), "confirmed".to_owned()];
    }
    let signals = state
        .application()
        .list_signals(&board, signal_options(&query)?)
        .await?
        .into_iter()
        .map(api_signal)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(MetadataEnvelope {
        data: signals,
        meta: SignalFilterMeta {
            include_all: query.include_all,
            limit: query.limit,
        },
    }))
}

fn signal_options(query: &SignalQuery) -> Result<ApplicationSignalListOptions, ApiError> {
    Ok(ApplicationSignalListOptions {
        statuses: query
            .status
            .iter()
            .map(|value| super::support::parse_status(value))
            .collect::<Result<Vec<_>, _>>()?,
        kinds: query.kind.clone(),
        task_ref: query.task_ref.clone(),
        include_all: query.include_all,
        limit: query.limit,
    })
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/boards/:board/signals", get(list_signals))
        .route("/api/v1/boards/:board/signals/review", get(review_signals))
}
