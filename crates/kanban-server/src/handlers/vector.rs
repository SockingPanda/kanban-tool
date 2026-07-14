use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};
use kanban_contract::{
    BoardQuery, DataEnvelope, VectorHelperStatusResponse, VectorStatus, VectorStatusResponse,
};

use crate::error::{ApiError, extractor_error};
use crate::helper::{HelperKind, helper_degraded_message, run_helper_json};
use crate::state::AppState;

pub(crate) async fn vector_status(
    State(state): State<AppState>,
    query: Result<Query<BoardQuery>, QueryRejection>,
) -> Result<Json<VectorStatusResponse>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let args = vector_helper_args(&state, &query.board, &["status".to_owned()]);
    let status = match run_helper_json::<VectorHelperStatusResponse>(
        state,
        HelperKind::Vector,
        args,
    )
    .await
    {
        Ok(status) => status,
        Err(error) if error.is_status_degraded() => degraded_vector_status(&error),
        Err(error) => return Err(error.into()),
    };
    Ok(Json(DataEnvelope::new(VectorStatus {
        backend: status.backend,
        enabled: status.enabled,
        message: status.message,
        diagnostics: status.diagnostics,
        dirty: status.dirty,
        board_dirty: status.board_dirty,
        generation: status.generation,
    })))
}

pub(crate) fn vector_helper_args(
    state: &AppState,
    board: &str,
    command_args: &[String],
) -> Vec<String> {
    let mut args = command_args.to_vec();
    args.push("--db".to_owned());
    args.push(state.db_path().display().to_string());
    args.push("--board".to_owned());
    args.push(board.to_owned());
    if let Some(path) = state.vector_config_path() {
        args.push("--vector-config".to_owned());
        args.push(path.display().to_string());
    }
    args
}

pub(crate) fn degraded_vector_status(
    error: &crate::helper::HelperRunError,
) -> VectorHelperStatusResponse {
    VectorHelperStatusResponse {
        backend: error.degraded_backend().to_owned(),
        enabled: false,
        message: helper_degraded_message(HelperKind::Vector, error),
        diagnostics: vec![error.degraded_diagnostic().to_owned()],
        dirty: None,
        board_dirty: None,
        generation: None,
    }
}

pub(crate) fn vector_store_status_from_helper(
    status: VectorHelperStatusResponse,
) -> kanban_vector::VectorStoreStatus {
    kanban_vector::VectorStoreStatus {
        backend: status.backend,
        enabled: status.enabled,
        message: status.message,
        diagnostics: status.diagnostics,
        dirty: status.dirty,
        board_dirty: status.board_dirty,
        generation: status.generation,
    }
}
