use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};
use kanban_contract::{
    BoardQuery, CheckpointReport, CheckpointResponse, DataEnvelope, DoctorResponse, StatsResponse,
};

use crate::error::{ApiError, extractor_error};
use crate::state::AppState;

pub(crate) async fn get_stats(
    State(state): State<AppState>,
    query: Result<Query<BoardQuery>, QueryRejection>,
) -> Result<Json<StatsResponse>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let stats = kanban_sqlite::api::queue_stats(state.db_path(), &query.board)?;
    Ok(Json(DataEnvelope::new(crate::queue_stats_from_record(
        stats,
    )?)))
}

fn checkpoint_report(value: kanban_sqlite::api::CheckpointResult) -> CheckpointReport {
    CheckpointReport {
        busy: value.busy,
        log_frames: value.log_frames,
        checkpointed_frames: value.checkpointed_frames,
    }
}

pub(crate) async fn doctor(
    State(state): State<AppState>,
) -> Result<Json<DoctorResponse>, ApiError> {
    Ok(Json(DoctorResponse::new(crate::doctor_report_from_record(
        kanban_sqlite::api::doctor_database(state.db_path())?,
    ))))
}

pub(crate) async fn checkpoint(
    State(state): State<AppState>,
) -> Result<Json<CheckpointResponse>, ApiError> {
    Ok(Json(CheckpointResponse::new(checkpoint_report(
        kanban_sqlite::api::checkpoint_database(state.db_path())?,
    ))))
}
