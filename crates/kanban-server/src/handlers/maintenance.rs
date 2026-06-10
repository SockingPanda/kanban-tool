use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};

use crate::dto::Envelope;
use crate::error::{ApiError, extractor_error};
use crate::state::AppState;

use super::shared::StatsQuery;

pub(crate) async fn get_stats(
    State(state): State<AppState>,
    query: Result<Query<StatsQuery>, QueryRejection>,
) -> Result<Json<Envelope<kanban_sqlite::QueueStats>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    Ok(Json(Envelope {
        data: kanban_sqlite::queue_stats(state.db_path(), &query.board)?,
        meta: None,
    }))
}

pub(crate) async fn doctor(
    State(state): State<AppState>,
) -> Result<Json<Envelope<kanban_sqlite::DoctorReport>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::doctor_database(state.db_path())?,
        meta: None,
    }))
}

pub(crate) async fn checkpoint(
    State(state): State<AppState>,
) -> Result<Json<Envelope<kanban_sqlite::CheckpointResult>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::checkpoint_database(state.db_path())?,
        meta: None,
    }))
}
