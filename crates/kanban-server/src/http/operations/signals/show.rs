use super::support::api_signal;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use kanban_contract::{DataEnvelope, GetSignalResponse, SignalPath};

pub(crate) async fn get_signal(
    State(state): State<AppState>,
    Path(SignalPath { signal_id }): Path<SignalPath>,
) -> Result<Json<GetSignalResponse>, ApiError> {
    let signal = state.application().get_signal(&signal_id).await?;
    Ok(Json(DataEnvelope {
        data: api_signal(signal)?,
    }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/signals/:signal_id", get(get_signal))
}
