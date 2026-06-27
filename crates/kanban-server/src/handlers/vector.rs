use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};

use crate::dto::{BoardQuery, Envelope};
use crate::error::{ApiError, extractor_error};
use crate::helper::{HelperKind, helper_degraded_message, run_helper_json};
use crate::state::AppState;

pub(crate) async fn vector_status(
    State(state): State<AppState>,
    query: Result<Query<BoardQuery>, QueryRejection>,
) -> Result<Json<Envelope<kanban_vector::VectorStoreStatus>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let args = vector_helper_args(&state, &query.board, &["status".to_owned()]);
    let status =
        match run_helper_json::<kanban_vector::VectorStoreStatus>(state, HelperKind::Vector, args)
            .await
        {
            Ok(status) => status,
            Err(error) if error.is_status_degraded() => degraded_vector_status(&error),
            Err(error) => return Err(error.into()),
        };
    Ok(Json(Envelope {
        data: status,
        meta: None,
    }))
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
) -> kanban_vector::VectorStoreStatus {
    let mut status = kanban_vector::VectorStoreStatus::new(
        error.degraded_backend(),
        false,
        helper_degraded_message(HelperKind::Vector, error),
    );
    status
        .diagnostics
        .push(error.degraded_diagnostic().to_owned());
    status
}
