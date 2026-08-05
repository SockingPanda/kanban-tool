use std::{future::Future, path::Path};

use kanban_core::{
    Clock, KanbanError, Result, is_claimable_task, new_event_id, new_run_id, new_typed_id,
};

use crate::{ApplicationService, ApplicationStore, ClaimRecord, ExecutionPlanState, TaskRecord};

#[derive(Debug, Clone, PartialEq)]
pub struct ClaimTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub ttl_ms: i64,
    pub worker_profile: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub claim_token: String,
    pub run_id: String,
    pub event_id: String,
    pub worker_profile: String,
    pub metadata_json: String,
    pub log_path: Option<String>,
    pub now: i64,
    pub claim_expires_at: i64,
}

pub trait TaskClaim: ApplicationStore {
    fn get_task(&self, task_id: &str) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn claim_task(
        &self,
        task_id: &str,
        input: ClaimTaskRecord,
    ) -> impl Future<Output = Result<ClaimRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: TaskClaim,
    C: Clock,
{
    pub async fn claim_task(&self, command: ClaimTaskCommand) -> Result<ClaimRecord> {
        self.claim_task_inner(command, None).await
    }

    /// Claim a task for the in-process dispatcher and atomically attach its run log path.
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
        let task = self.store.get_task(task_id).await?;
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
        self.store
            .claim_task(
                task_id,
                ClaimTaskRecord {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
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
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::{Arc, atomic::AtomicUsize};

    use kanban_core::{KanbanError, Result, TaskStatus};

    use crate::operations::test_support::{FixedClock, StubStore, task_for_id};
    use crate::*;

    impl TaskClaim for StubStore {
        async fn get_task(&self, task_id: &str) -> Result<TaskRecord> {
            Ok(task_for_id(task_id))
        }

        async fn claim_task(&self, task_id: &str, input: ClaimTaskRecord) -> Result<ClaimRecord> {
            assert_eq!(task_id, "t_claim");
            assert_eq!(input.expected_lock_version, 0);
            assert_eq!(input.actor, "worker");
            assert!(input.claim_token.starts_with("claim_"));
            assert!(input.run_id.starts_with("r_"));
            assert!(input.event_id.starts_with("e_"));
            match input.worker_profile.as_str() {
                "manual" => {
                    assert_eq!(input.metadata_json, r#"{"source":"test"}"#);
                    assert_eq!(input.log_path, None);
                }
                "dispatcher" => {
                    assert_eq!(input.metadata_json, "{}");
                    assert_eq!(
                        input.log_path.as_deref(),
                        Some(format!("dispatcher-logs/{}.log", input.run_id).as_str())
                    );
                }
                profile => panic!("unexpected worker profile: {profile}"),
            }
            assert_eq!(input.now, 100);
            assert_eq!(input.claim_expires_at, 400);
            let claim_expires_at = input.claim_expires_at;
            let mut task = task_for_id(task_id);
            task.status = TaskStatus::Running;
            task.has_claim_token = true;
            task.claim_owner = Some(input.actor.clone());
            task.claim_expires_at = Some(claim_expires_at);
            task.last_heartbeat_at = Some(input.now);
            task.current_run_id = Some(input.run_id.clone());
            task.started_at = Some(input.now);
            task.updated_at = input.now;
            task.lock_version += 1;
            Ok(ClaimRecord {
                task,
                run: RunRecord {
                    id: input.run_id,
                    board_id: "b_default".into(),
                    task_id: task_id.to_owned(),
                    status: RunStatus::Running,
                    worker_profile: Some(input.worker_profile),
                    worker_pid: None,
                    claim_owner: input.actor,
                    claim_expires_at,
                    started_at: input.now,
                    last_heartbeat_at: Some(input.now),
                    finished_at: None,
                    exit_code: None,
                    summary: None,
                    error: None,
                    log_path: input.log_path,
                    metadata_json: input.metadata_json,
                },
                claim_token: input.claim_token,
                claim_expires_at,
            })
        }
    }
    #[tokio::test]
    async fn claim_task_uses_core_guard_and_canonicalizes_lease_input() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let claim = service
            .claim_task(ClaimTaskCommand {
                task_id: " t_claim ".into(),
                actor: " worker ".into(),
                ttl_ms: 300,
                worker_profile: None,
                metadata: serde_json::json!({"source": "test"}),
            })
            .await
            .unwrap();
        assert_eq!(claim.task.status, TaskStatus::Running);
        assert_eq!(claim.run.status, RunStatus::Running);
        assert!(claim.claim_token.starts_with("claim_"));
        assert_eq!(claim.claim_expires_at, 400);

        let dispatcher_claim = service
            .claim_task_with_run_log_dir(
                ClaimTaskCommand {
                    task_id: "t_claim".into(),
                    actor: "worker".into(),
                    ttl_ms: 300,
                    worker_profile: Some("dispatcher".into()),
                    metadata: serde_json::json!({}),
                },
                Path::new("dispatcher-logs"),
            )
            .await
            .unwrap();
        assert_eq!(
            dispatcher_claim.run.log_path.as_deref(),
            Some(format!("dispatcher-logs/{}.log", dispatcher_claim.run.id).as_str())
        );

        let claimed = service
            .claim_task(ClaimTaskCommand {
                task_id: "t_claimed".into(),
                actor: "worker".into(),
                ttl_ms: 300,
                worker_profile: None,
                metadata: serde_json::json!({}),
            })
            .await
            .unwrap_err();
        assert!(matches!(
            claimed,
            KanbanError::InvalidTransition(message) if message.contains("claim conflict")
        ));
        let dependency = service
            .claim_task(ClaimTaskCommand {
                task_id: "t_claim_dependency".into(),
                actor: "worker".into(),
                ttl_ms: 300,
                worker_profile: None,
                metadata: serde_json::json!({}),
            })
            .await
            .unwrap_err();
        assert!(matches!(
            dependency,
            KanbanError::InvalidTransition(message) if message.contains("dependency blocked")
        ));
        let unplanned = service
            .claim_task(ClaimTaskCommand {
                task_id: "t_claim_unplanned".into(),
                actor: "worker".into(),
                ttl_ms: 300,
                worker_profile: None,
                metadata: serde_json::json!({}),
            })
            .await
            .unwrap_err();
        assert!(matches!(unplanned, KanbanError::ExecutionPlanRequired(_)));
    }

    #[tokio::test]
    async fn claim_task_rejects_invalid_identity_and_lease() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(i64::MAX - 10),
        );
        for command in [
            ClaimTaskCommand {
                task_id: "default#1".into(),
                actor: "worker".into(),
                ttl_ms: 300,
                worker_profile: None,
                metadata: serde_json::json!({}),
            },
            ClaimTaskCommand {
                task_id: "t_claim".into(),
                actor: " ".into(),
                ttl_ms: 300,
                worker_profile: None,
                metadata: serde_json::json!({}),
            },
            ClaimTaskCommand {
                task_id: "t_claim".into(),
                actor: "worker".into(),
                ttl_ms: 0,
                worker_profile: None,
                metadata: serde_json::json!({}),
            },
            ClaimTaskCommand {
                task_id: "t_claim".into(),
                actor: "worker".into(),
                ttl_ms: 20,
                worker_profile: None,
                metadata: serde_json::json!({}),
            },
        ] {
            assert!(matches!(
                service.claim_task(command).await,
                Err(KanbanError::InvalidInput(_))
            ));
        }
    }
}
