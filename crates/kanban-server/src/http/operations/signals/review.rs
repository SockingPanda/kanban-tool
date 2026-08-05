use super::support::api_signal;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_application::{SignalLifecycle, SignalReviewCommand};
use kanban_contract::{BoardLabelPath, DataEnvelope, ReviewSignalsRequest, SignalWire};
use kanban_core::KanbanError;

use super::super::support::request_actor;

pub(crate) async fn confirm_signals(
    state: State<AppState>,
    path: Path<BoardLabelPath>,
    headers: HeaderMap,
    body: Result<Json<ReviewSignalsRequest>, JsonRejection>,
) -> Result<Json<DataEnvelope<Vec<SignalWire>>>, ApiError> {
    review(state, path, headers, body, SignalLifecycle::Confirm).await
}

pub(crate) async fn reject_signals(
    state: State<AppState>,
    path: Path<BoardLabelPath>,
    headers: HeaderMap,
    body: Result<Json<ReviewSignalsRequest>, JsonRejection>,
) -> Result<Json<DataEnvelope<Vec<SignalWire>>>, ApiError> {
    review(state, path, headers, body, SignalLifecycle::Reject).await
}

pub(crate) async fn resolve_signals(
    state: State<AppState>,
    path: Path<BoardLabelPath>,
    headers: HeaderMap,
    body: Result<Json<ReviewSignalsRequest>, JsonRejection>,
) -> Result<Json<DataEnvelope<Vec<SignalWire>>>, ApiError> {
    review(state, path, headers, body, SignalLifecycle::Resolve).await
}

pub(crate) async fn supersede_signals(
    state: State<AppState>,
    path: Path<BoardLabelPath>,
    headers: HeaderMap,
    body: Result<Json<ReviewSignalsRequest>, JsonRejection>,
) -> Result<Json<DataEnvelope<Vec<SignalWire>>>, ApiError> {
    review(state, path, headers, body, SignalLifecycle::Supersede).await
}

async fn review(
    State(state): State<AppState>,
    Path(BoardLabelPath { board }): Path<BoardLabelPath>,
    headers: HeaderMap,
    body: Result<Json<ReviewSignalsRequest>, JsonRejection>,
    lifecycle: SignalLifecycle,
) -> Result<Json<DataEnvelope<Vec<SignalWire>>>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let result = state
        .application()
        .review_signals(SignalReviewCommand {
            board: Some(board),
            signal_ids: body.signal_ids,
            lifecycle,
            replacement_signal_id: body.replacement_signal_id,
            actor,
            reason: body.reason,
        })
        .await?
        .into_iter()
        .map(api_signal)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(DataEnvelope { data: result }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/boards/:board/signals/confirm",
            post(confirm_signals),
        )
        .route("/api/v1/boards/:board/signals/reject", post(reject_signals))
        .route(
            "/api/v1/boards/:board/signals/resolve",
            post(resolve_signals),
        )
        .route(
            "/api/v1/boards/:board/signals/supersede",
            post(supersede_signals),
        )
}
