use super::super::support::request_actor;
use super::support::api_signal;
use crate::{error::ApiError, http::operations::comments::support::api_comment, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    routing::post,
};
use kanban_application::SignalRecordCommand;
use kanban_core::KanbanError;
use kanban_protocol::{
    BoardLabelPath, DataEnvelope, RecordSignalRequest, RecordSignalResponse, SignalRecordResult,
};

pub(crate) async fn record_signal(
    State(state): State<AppState>,
    Path(BoardLabelPath { board }): Path<BoardLabelPath>,
    headers: axum::http::HeaderMap,
    body: Result<Json<RecordSignalRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<RecordSignalResponse>), ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let evidence = body.evidence.unwrap_or_default().0;
    let result = state
        .application()
        .record_signal(SignalRecordCommand {
            board,
            kind: body.kind,
            title: body.title,
            summary: body.summary,
            severity: body.severity,
            task_ref: body.task_ref,
            task_id: body.task_id,
            run_id: body.run_id,
            comment_id: body.comment_id,
            actor,
            agent_type: body.agent_type,
            dedupe_key: body.dedupe_key,
            source: body.source,
            evidence,
            comment_body: body.comment.and_then(|comment| comment.body),
        })
        .await?;
    let data = SignalRecordResult {
        signal: api_signal(result.signal)?,
        backlink_comment: result.backlink_comment.map(api_comment).transpose()?,
    };
    Ok((StatusCode::CREATED, Json(DataEnvelope { data })))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/boards/:board/signals", post(record_signal))
}
