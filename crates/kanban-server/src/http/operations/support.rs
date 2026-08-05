use crate::error::ApiError;
use axum::http::HeaderMap;
use kanban_application::{RunRecord, RunStatus};
use kanban_core::KanbanError;
use kanban_protocol::{ApiRun, ApiRunStatus};

pub(crate) fn api_run(run: RunRecord) -> Result<ApiRun, ApiError> {
    let metadata = serde_json::from_str(&run.metadata_json).map_err(|error| {
        KanbanError::Storage(format!("stored run metadata is invalid JSON: {error}"))
    })?;
    Ok(ApiRun {
        id: run.id,
        task_id: run.task_id,
        status: match run.status {
            RunStatus::Running => ApiRunStatus::Running,
            RunStatus::Succeeded => ApiRunStatus::Succeeded,
            RunStatus::Failed => ApiRunStatus::Failed,
            RunStatus::Canceled => ApiRunStatus::Canceled,
            RunStatus::Expired => ApiRunStatus::Expired,
        },
        worker_profile: run.worker_profile,
        worker_pid: run.worker_pid,
        claim_owner: run.claim_owner,
        started_at: run.started_at,
        finished_at: run.finished_at,
        exit_code: run.exit_code,
        summary: run.summary,
        error: run.error,
        has_log: run.log_path.is_some(),
        metadata,
    })
}

pub(super) fn request_actor(
    body_actor: Option<&str>,
    headers: &HeaderMap,
    default_actor: &str,
) -> Result<String, ApiError> {
    let actor = match body_actor {
        Some(actor) => actor,
        None => headers
            .get("x-kb-actor")
            .map(|value| {
                value.to_str().map_err(|_| {
                    KanbanError::InvalidInput("x-kb-actor must contain valid text".to_owned())
                })
            })
            .transpose()?
            .unwrap_or(default_actor),
    };
    let actor = actor.trim();
    if actor.is_empty() {
        return Err(KanbanError::InvalidInput("actor is required".to_owned()).into());
    }
    Ok(actor.to_owned())
}
