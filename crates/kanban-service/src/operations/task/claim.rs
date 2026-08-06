use std::path::Path;

use kanban_core::{
    Clock, KanbanError, Result, is_claimable_task, new_event_id, new_run_id, new_typed_id,
};

use crate::{ClaimRecord, ExecutionPlanState, KanbanService};

#[derive(Debug, Clone, PartialEq)]
pub struct ClaimTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub ttl_ms: i64,
    pub worker_profile: Option<String>,
    pub metadata: serde_json::Value,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn claim_task(&self, command: ClaimTaskCommand) -> Result<ClaimRecord> {
        self.claim_task_inner(command, None).await
    }

    /// 为进程内 dispatcher 认领任务，并原子附加其运行日志路径。
    pub async fn claim_task_with_run_log_dir(
        &self,
        command: ClaimTaskCommand,
        run_log_dir: &Path,
    ) -> Result<ClaimRecord> {
        if run_log_dir.as_os_str().is_empty() {
            return Err(KanbanError::InvalidInput(
                "run_log_dir is required".to_owned(),
            ));
        }
        self.claim_task_inner(command, Some(run_log_dir)).await
    }

    async fn claim_task_inner(
        &self,
        command: ClaimTaskCommand,
        run_log_dir: Option<&Path>,
    ) -> Result<ClaimRecord> {
        let task_id = command.task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        let actor = command.actor.trim();
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("actor is required".to_owned()));
        }
        if command.ttl_ms <= 0 {
            return Err(KanbanError::InvalidInput(
                "ttl_ms must be positive".to_owned(),
            ));
        }
        let worker_profile = command
            .worker_profile
            .unwrap_or_else(|| "manual".to_owned());
        let metadata_json = serde_json::to_string(&command.metadata)
            .map_err(|error| KanbanError::InvalidInput(format!("invalid metadata: {error}")))?;
        let _mutation = self.mutation_gate.lock().await;
        let task = self.get_task(task_id).await?;
        if !is_claimable_task(task.status, task.has_claim_token) {
            let message = if task.has_claim_token {
                "claim conflict: task is already claimed"
            } else {
                "task is not claimable"
            };
            return Err(KanbanError::InvalidTransition(message.to_owned()));
        }
        if task.dependency_blocked {
            return Err(KanbanError::InvalidTransition(
                "dependency blocked".to_owned(),
            ));
        }
        if task.execution_plan_state == ExecutionPlanState::Unplanned {
            return Err(KanbanError::ExecutionPlanRequired(
                "add steps or mark execution plan not_required before claiming task".to_owned(),
            ));
        }
        let now = self.clock.now_ms();
        let claim_expires_at = now.checked_add(command.ttl_ms).ok_or_else(|| {
            KanbanError::InvalidInput("ttl_ms produces an invalid claim expiry".to_owned())
        })?;
        let run_id = new_run_id();
        let log_path = run_log_dir
            .map(|directory| directory.join(format!("{run_id}.log")))
            .map(|path| {
                path.into_os_string().into_string().map_err(|_| {
                    KanbanError::InvalidInput("run log path must be valid UTF-8".to_owned())
                })
            })
            .transpose()?;
        let claim = self
            .store
            .claim_task(
                task_id,
                crate::store_operations::ClaimTaskInput {
                    expected_lock_version: task.lock_version,
                    owner: actor.to_owned(),
                    claim_token: new_typed_id("claim"),
                    run_id,
                    event_id: new_event_id(),
                    worker_profile,
                    metadata_json,
                    log_path,
                    now,
                    claim_expires_at,
                },
            )
            .await
            .map_err(crate::error::store_error)?;
        application_claim(claim)
    }
}

fn application_claim(claim: crate::store_operations::ClaimTaskRecord) -> Result<ClaimRecord> {
    Ok(ClaimRecord {
        task: super::application_task(claim.task)?,
        run: crate::operations::application_run(claim.run)?,
        claim_token: claim.claim_token,
        claim_expires_at: claim.claim_expires_at,
    })
}
