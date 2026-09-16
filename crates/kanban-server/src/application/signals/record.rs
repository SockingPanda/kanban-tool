use super::super::support::request_actor;
use super::support::api_signal;
use crate::{application::comments::support::api_comment, error::ApiError, state::AppState};
use kanban_protocol::{
    BoardLabelPath, DataEnvelope, RecordSignalRequest, RecordSignalResponse, SignalRecordResult,
};
use kanban_service::SignalRecordCommand;
pub(crate) async fn record_signal(
    state: AppState,
    BoardLabelPath { board }: BoardLabelPath,
    headers: crate::application::support::CallContext,
    body: RecordSignalRequest,
) -> Result<RecordSignalResponse, ApiError> {
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
    Ok(DataEnvelope { data })
}
