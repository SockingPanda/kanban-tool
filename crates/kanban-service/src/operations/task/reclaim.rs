use kanban_core::{
    Clock, KanbanError, ReadinessFacts, Result, TaskStatus, new_event_id, recompute_ready_status,
    retry_decision,
};

use crate::{ExecutionPlanState, KanbanService, TaskRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReclaimTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub force: bool,
    pub target_status: Option<TaskStatus>,
    pub reason: Option<String>,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    /// 显式回收一个 running 任务。未过期 claim 只有 force 才能回收。
    pub async fn reclaim_task(&self, command: ReclaimTaskCommand) -> Result<TaskRecord> {
        let task_id = command.task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id 必须是全局 t_... ID".to_owned(),
            ));
        }
        let actor = command.actor.trim();
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("actor 不能为空".to_owned()));
        }
        if let Some(target) = command.target_status
            && !matches!(target, TaskStatus::Ready | TaskStatus::Blocked)
        {
            return Err(KanbanError::InvalidInput(
                "reclaim target status 必须是 ready 或 blocked".to_owned(),
            ));
        }
        let reason = command.reason.unwrap_or_else(|| "claim 已回收".to_owned());
        if reason.trim().is_empty() {
            return Err(KanbanError::InvalidInput(
                "reclaim reason 不能为空".to_owned(),
            ));
        }
        let _mutation = self.mutation_gate.lock().await;
        let task = self.get_task(task_id).await?;
        if task.status != TaskStatus::Running {
            return Err(KanbanError::InvalidTransition(
                "reclaim 只能用于 running 任务".to_owned(),
            ));
        }
        let now = self.clock.now_ms();
        if !command.force && task.claim_expires_at.is_none_or(|expires| expires > now) {
            return Err(KanbanError::InvalidTransition(
                "claim 未过期时必须设置 force 才能 reclaim".to_owned(),
            ));
        }
        let decision = retry_decision(task.retry_count, task.max_retries, TaskStatus::Ready);
        let mut target_status = if decision.max_retries_reached {
            TaskStatus::Blocked
        } else if let Some(target) = command.target_status {
            target
        } else {
            recompute_ready_status(
                ReadinessFacts {
                    title: &task.title,
                    description: task.description.as_deref(),
                    scheduled_at: task.scheduled_at,
                    dependencies_done: !task.dependency_blocked,
                },
                now,
            )
        };
        if target_status == TaskStatus::Ready
            && task.execution_plan_state == ExecutionPlanState::Unplanned
        {
            target_status = TaskStatus::Todo;
        }
        if target_status != TaskStatus::Blocked
            && !matches!(
                target_status,
                TaskStatus::Ready | TaskStatus::Todo | TaskStatus::Scheduled | TaskStatus::Triage
            )
        {
            return Err(KanbanError::InvalidTransition(
                "reclaim target status 不是可重算的 active 状态".to_owned(),
            ));
        }
        self.store
            .reclaim_task(
                task_id,
                crate::store_operations::ReclaimTaskInput {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    force: command.force,
                    target_status: target_status.as_str().to_owned(),
                    retry_count: decision.retry_count,
                    reason: reason.trim().to_owned(),
                    event_id: new_event_id(),
                    now,
                },
            )
            .await
            .map_err(crate::error::store_error)
            .and_then(super::application_task)
    }

    /// 仅为进程内 dispatcher 回收已过期的 running lease。
    pub async fn reclaim_expired(&self, board: &str, actor: &str) -> Result<usize> {
        let board = board.trim();
        if board.is_empty() {
            return Err(KanbanError::InvalidInput("board is required".to_owned()));
        }
        let actor = actor.trim();
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("actor 不能为空".to_owned()));
        }
        let _mutation = self.mutation_gate.lock().await;
        let now = self.clock.now_ms();
        let expired = self
            .store
            .list_expired_claims(board, now)
            .await
            .map_err(crate::error::store_error)?
            .into_iter()
            .map(super::application_task)
            .collect::<Result<Vec<_>>>()?;
        let mut reclaimed = 0;
        for task in expired {
            let decision = retry_decision(task.retry_count, task.max_retries, TaskStatus::Ready);
            let mut target_status = decision.status;
            if target_status == TaskStatus::Ready {
                target_status = recompute_ready_status(
                    ReadinessFacts {
                        title: &task.title,
                        description: task.description.as_deref(),
                        scheduled_at: task.scheduled_at,
                        dependencies_done: !task.dependency_blocked,
                    },
                    now,
                );
                let has_executable_plan = task.execution_plan_state
                    == ExecutionPlanState::NotRequired
                    || task.required_step_count > 0
                    || task.optional_step_count > 0;
                if target_status == TaskStatus::Ready && !has_executable_plan {
                    target_status = TaskStatus::Todo;
                }
            }
            let reason = if decision.max_retries_reached {
                "max retries reached"
            } else {
                "claim expired"
            };
            let result = self
                .store
                .reclaim_expired_task(
                    &task.id,
                    crate::store_operations::ReclaimExpiredTaskInput {
                        expected_lock_version: task.lock_version,
                        actor: actor.to_owned(),
                        event_id: new_event_id(),
                        target_status: target_status.as_str().to_owned(),
                        retry_count: decision.retry_count,
                        reason: reason.to_owned(),
                        now,
                    },
                )
                .await
                .map_err(crate::error::store_error)?;
            if result.is_some() {
                reclaimed += 1;
            }
        }
        Ok(reclaimed)
    }
}
