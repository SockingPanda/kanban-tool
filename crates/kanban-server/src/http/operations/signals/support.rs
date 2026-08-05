use crate::error::ApiError;
use kanban_service::{SignalRecord, SignalStatus};
use kanban_core::KanbanError;
use kanban_protocol::{SignalObservationWire, SignalWire};

pub(crate) fn api_signal(signal: SignalRecord) -> Result<SignalWire, ApiError> {
    let evidence: serde_json::Value = serde_json::from_str(&signal.observation.evidence_json)
        .map_err(|error| {
            KanbanError::Storage(format!(
                "stored signal evidence is invalid JSON for {}: {error}",
                signal.id
            ))
        })?;
    let evidence = evidence.as_object().cloned().ok_or_else(|| {
        ApiError(KanbanError::Storage(format!(
            "stored signal evidence is not a JSON object for {}",
            signal.id
        )))
    })?;
    Ok(SignalWire {
        id: signal.id,
        board_id: signal.board_id,
        observation_id: signal.observation_id,
        kind: signal.kind,
        title: signal.title,
        summary: signal.summary,
        severity: signal.severity,
        status: signal.status.as_str().to_owned(),
        dedupe_key: signal.dedupe_key,
        superseded_by_signal_id: signal.superseded_by_signal_id,
        reviewed_by: signal.reviewed_by,
        reviewed_at: signal.reviewed_at,
        review_reason: signal.review_reason,
        created_at: signal.created_at,
        updated_at: signal.updated_at,
        observation: SignalObservationWire {
            id: signal.observation.id,
            board_id: signal.observation.board_id,
            task_id: signal.observation.task_id,
            task_ref_snapshot: signal.observation.task_ref_snapshot,
            run_id: signal.observation.run_id,
            comment_id: signal.observation.comment_id,
            actor: signal.observation.actor,
            agent_type: signal.observation.agent_type,
            source: signal.observation.source,
            evidence: kanban_protocol::structured_metadata::JsonObject(
                evidence.into_iter().collect(),
            ),
            created_at: signal.observation.created_at,
        },
    })
}

pub(crate) fn parse_status(value: &str) -> Result<SignalStatus, ApiError> {
    match value {
        "open" => Ok(SignalStatus::Open),
        "confirmed" => Ok(SignalStatus::Confirmed),
        "rejected" => Ok(SignalStatus::Rejected),
        "superseded" => Ok(SignalStatus::Superseded),
        "resolved" => Ok(SignalStatus::Resolved),
        other => Err(KanbanError::InvalidInput(format!("invalid signal status: {other}")).into()),
    }
}
