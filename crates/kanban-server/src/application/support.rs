use crate::error::ApiError;
use kanban_protocol::{ApiRun, ApiRunStatus};
use kanban_service::KanbanError;
use kanban_service::{RunRecord, RunStatus};
pub(crate) fn api_run(run: RunRecord) -> Result<ApiRun, ApiError> {
    let metadata = serde_json::from_str(&run.metadata_json).map_err(|error| {
        KanbanError::Storage(format!("存储的 run metadata 不是有效 JSON：{error}"))
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
/// 所有传输共用的调用者上下文；业务 actor 仍遵循请求体、metadata、Host 默认值的优先级。
#[derive(Clone, Debug, Default)]
pub(crate) struct CallContext {
    pub actor: Option<String>,
}

pub(super) fn request_actor(
    body_actor: Option<&str>,
    context: &CallContext,
    default_actor: &str,
) -> Result<String, ApiError> {
    let actor = body_actor
        .or(context.actor.as_deref())
        .unwrap_or(default_actor)
        .trim();
    if actor.is_empty() {
        return Err(KanbanError::InvalidInput("必须提供 actor".to_owned()).into());
    }
    Ok(actor.to_owned())
}
