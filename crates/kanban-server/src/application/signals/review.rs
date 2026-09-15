use super::super::support::request_actor;
use super::support::api_signal;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{BoardLabelPath, DataEnvelope, ReviewSignalsRequest, SignalWire};
use kanban_service::{SignalLifecycle, SignalReviewCommand};
pub(crate) async fn confirm_signals(
    state: AppState,
    path: BoardLabelPath,
    headers: CallContext,
    body: ReviewSignalsRequest,
) -> Result<DataEnvelope<Vec<SignalWire>>, ApiError> {
    review(state, path, headers, body, SignalLifecycle::Confirm).await
}
pub(crate) async fn reject_signals(
    state: AppState,
    path: BoardLabelPath,
    headers: CallContext,
    body: ReviewSignalsRequest,
) -> Result<DataEnvelope<Vec<SignalWire>>, ApiError> {
    review(state, path, headers, body, SignalLifecycle::Reject).await
}
pub(crate) async fn resolve_signals(
    state: AppState,
    path: BoardLabelPath,
    headers: CallContext,
    body: ReviewSignalsRequest,
) -> Result<DataEnvelope<Vec<SignalWire>>, ApiError> {
    review(state, path, headers, body, SignalLifecycle::Resolve).await
}
pub(crate) async fn supersede_signals(
    state: AppState,
    path: BoardLabelPath,
    headers: CallContext,
    body: ReviewSignalsRequest,
) -> Result<DataEnvelope<Vec<SignalWire>>, ApiError> {
    review(state, path, headers, body, SignalLifecycle::Supersede).await
}
async fn review(
    state: AppState,
    BoardLabelPath { board }: BoardLabelPath,
    headers: CallContext,
    body: ReviewSignalsRequest,
    lifecycle: SignalLifecycle,
) -> Result<DataEnvelope<Vec<SignalWire>>, ApiError> {
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
    Ok(DataEnvelope { data: result })
}
