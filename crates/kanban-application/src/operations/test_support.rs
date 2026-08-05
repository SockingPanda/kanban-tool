use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use kanban_core::{Board, Clock, KanbanError, Result, TaskStatus};

use crate::*;

#[derive(Clone)]
pub(crate) struct StubStore {
    pub(crate) calls: Arc<AtomicUsize>,
}

impl ApplicationStore for StubStore {
    async fn list_boards(&self, include_archived: bool) -> Result<Vec<BoardRecord>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert!(include_archived);
        Ok(vec![Board {
            id: "b_default".into(),
            slug: "default".into(),
            name: "Default".into(),
            description: None,
            created_at: 1,
            updated_at: 1,
            archived_at: None,
        }])
    }

    async fn list_board_columns(&self, board: &str) -> Result<Vec<BoardColumnRecord>> {
        assert_eq!(board, "default");
        Ok(vec![BoardColumnRecord {
            id: "col_default_todo".into(),
            board_id: "b_default".into(),
            status: TaskStatus::Todo,
            title: "Todo".into(),
            position: 20,
            hidden: false,
            wip_limit: None,
            created_at: 1,
            updated_at: 1,
        }])
    }

    async fn create_task(&self, board: &str, input: CreateTaskRecord) -> Result<TaskRecord> {
        assert_eq!(board, "default");
        Ok(task_record(input))
    }

    async fn list_tasks(&self, board: &str, options: TaskListOptions) -> Result<TaskListPage> {
        assert_eq!(board, "default");
        assert_eq!(options.assignee.as_deref(), Some("worker"));
        assert_eq!(options.query.as_deref(), Some("needle"));
        Ok(TaskListPage {
            tasks: Vec::new(),
            total: 0,
        })
    }

    async fn get_task(&self, task_id: &str) -> Result<TaskRecord> {
        Ok(task_for_id(task_id))
    }

    async fn mark_execution_plan_not_required(
        &self,
        task_id: &str,
        input: MarkExecutionPlanNotRequiredRecord,
    ) -> Result<ExecutionPlanRecord> {
        assert_eq!(task_id, "t_show");
        assert_eq!(input.reason, "small task");
        assert_eq!(input.actor, "tester");
        assert!(input.event_id.starts_with("e_"));
        assert_eq!(input.updated_at, 100);
        Ok(ExecutionPlanRecord {
            board_id: "b_default".into(),
            task_id: task_id.to_owned(),
            state: ExecutionPlanState::NotRequired,
            reason: Some(input.reason),
            updated_by: input.actor,
            updated_at: input.updated_at,
        })
    }

    async fn promote_task(&self, task_id: &str, input: PromoteTaskRecord) -> Result<TaskRecord> {
        assert_eq!(task_id, "t_promote");
        assert_eq!(input.expected_lock_version, 0);
        assert_eq!(input.actor, "promoter");
        assert!(input.event_id.starts_with("e_"));
        assert_eq!(input.updated_at, 100);
        let mut task = task_for_id(task_id);
        task.status = TaskStatus::Ready;
        task.lock_version += 1;
        task.updated_at = input.updated_at;
        Ok(task)
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

    async fn heartbeat_task(
        &self,
        task_id: &str,
        input: HeartbeatTaskRecord,
    ) -> Result<TaskRecord> {
        assert_eq!(task_id, "t_heartbeat");
        assert_eq!(input.expected_lock_version, 2);
        assert_eq!(input.actor, "worker");
        assert!(input.event_id.starts_with("e_"));
        assert_eq!(input.now, 100);
        assert_eq!(input.claim_expires_at, 400);
        if input.claim_token != "claim_valid" {
            return Err(KanbanError::InvalidTransition(
                "claim token mismatch".to_owned(),
            ));
        }
        assert_eq!(input.note.as_deref(), Some(" alive "));
        let mut task = task_for_id(task_id);
        task.claim_expires_at = Some(input.claim_expires_at);
        task.last_heartbeat_at = Some(input.now);
        task.updated_at = input.now;
        task.lock_version += 1;
        Ok(task)
    }

    async fn release_task(&self, task_id: &str, input: ReleaseTaskRecord) -> Result<TaskRecord> {
        assert_eq!(task_id, "t_release");
        assert_eq!(input.expected_lock_version, 2);
        assert_eq!(input.actor, "worker");
        assert!(input.event_id.starts_with("e_"));
        assert_eq!(input.now, 100);
        if input.claim_token != "claim_valid" {
            return Err(KanbanError::InvalidTransition(
                "claim token mismatch".to_owned(),
            ));
        }
        let mut task = task_for_id(task_id);
        task.status = TaskStatus::Ready;
        task.has_claim_token = false;
        task.claim_owner = None;
        task.claim_expires_at = None;
        task.last_heartbeat_at = None;
        task.current_run_id = None;
        task.updated_at = input.now;
        task.lock_version += 1;
        Ok(task)
    }

    async fn list_expired_claims(&self, board: &str, now: i64) -> Result<Vec<TaskRecord>> {
        assert_eq!(board, "default");
        assert_eq!(now, 100);
        let mut task = task_for_id("t_expired");
        task.status = TaskStatus::Running;
        task.execution_plan_state = ExecutionPlanState::NotRequired;
        task.has_claim_token = true;
        task.claim_owner = Some("worker".to_owned());
        task.claim_expires_at = Some(90);
        task.last_heartbeat_at = Some(80);
        task.current_run_id = Some("r_expired".to_owned());
        task.lock_version = 2;
        task.max_retries = Some(2);
        let mut planned_without_steps = task.clone();
        planned_without_steps.id = "t_expired_planned".to_owned();
        planned_without_steps.current_run_id = Some("r_expired_planned".to_owned());
        planned_without_steps.execution_plan_state = ExecutionPlanState::Planned;
        Ok(vec![task, planned_without_steps])
    }

    async fn reclaim_expired_task(
        &self,
        task_id: &str,
        input: ReclaimExpiredTaskRecord,
    ) -> Result<Option<TaskRecord>> {
        assert!(matches!(task_id, "t_expired" | "t_expired_planned"));
        assert_eq!(input.expected_lock_version, 2);
        assert_eq!(input.actor, "dispatcher");
        assert!(input.event_id.starts_with("e_"));
        let expected_status = if task_id == "t_expired" {
            TaskStatus::Ready
        } else {
            TaskStatus::Todo
        };
        assert_eq!(input.target_status, expected_status);
        assert_eq!(input.retry_count, 1);
        assert_eq!(input.reason, "claim expired");
        assert_eq!(input.now, 100);
        let mut task = task_for_id(task_id);
        task.status = input.target_status;
        task.retry_count = input.retry_count;
        task.lock_version = input.expected_lock_version + 1;
        Ok(Some(task))
    }

    async fn submit_review_task(
        &self,
        task_id: &str,
        input: SubmitReviewTaskRecord,
    ) -> Result<TaskRecord> {
        assert_eq!(task_id, "t_review");
        assert_eq!(input.expected_lock_version, 2);
        assert_eq!(input.actor, "worker");
        assert!(input.event_id.starts_with("e_"));
        assert_eq!(input.now, 100);
        if !input.force && input.claim_token.as_deref() != Some("claim_valid") {
            return Err(KanbanError::InvalidTransition(
                "claim token mismatch".to_owned(),
            ));
        }
        let mut task = task_for_id(task_id);
        task.status = TaskStatus::Review;
        task.has_claim_token = false;
        task.claim_owner = None;
        task.claim_expires_at = None;
        task.last_heartbeat_at = None;
        task.result_summary = input.summary;
        task.updated_at = input.now;
        task.lock_version += 1;
        Ok(task)
    }

    async fn complete_task(&self, task_id: &str, input: CompleteTaskRecord) -> Result<TaskRecord> {
        let expected_lock_version = if task_id == "t_complete_review" { 3 } else { 2 };
        assert_eq!(input.expected_lock_version, expected_lock_version);
        assert_eq!(input.actor, "worker");
        assert!(input.event_id.starts_with("e_"));
        assert_eq!(input.now, 100);
        let source = task_for_id(task_id);
        if source.status == TaskStatus::Running
            && !input.force
            && input.claim_token.as_deref() != Some("claim_valid")
        {
            return Err(KanbanError::InvalidTransition(
                "claim token mismatch".to_owned(),
            ));
        }
        let mut task = source;
        task.status = TaskStatus::Done;
        task.has_claim_token = false;
        task.claim_owner = None;
        task.claim_expires_at = None;
        task.last_heartbeat_at = None;
        task.result_summary = input.summary;
        task.result_json = input.result_json;
        task.completed_at = Some(input.now);
        task.updated_at = input.now;
        task.lock_version += 1;
        Ok(task)
    }

    async fn block_task(&self, task_id: &str, input: BlockTaskRecord) -> Result<TaskRecord> {
        let source = task_for_id(task_id);
        assert_eq!(input.expected_lock_version, source.lock_version);
        assert!(matches!(input.actor.as_str(), "worker" | "admin"));
        assert_eq!(input.reason.trim(), "waiting");
        assert!(input.event_id.starts_with("e_"));
        assert_eq!(input.now, 100);
        if source.status == TaskStatus::Running
            && !input.force
            && input.claim_token.as_deref() != Some("claim_valid")
        {
            return Err(KanbanError::InvalidTransition(
                "claim token mismatch".to_owned(),
            ));
        }
        let mut task = source;
        task.status = TaskStatus::Blocked;
        task.status_reason = Some(input.reason);
        task.has_claim_token = false;
        task.claim_owner = None;
        task.claim_expires_at = None;
        task.last_heartbeat_at = None;
        task.updated_at = input.now;
        task.lock_version += 1;
        Ok(task)
    }

    async fn create_comment(
        &self,
        task_id: &str,
        input: CreateCommentRecord,
    ) -> Result<CommentRecord> {
        assert_eq!(task_id, "t_comment");
        assert!(input.id.starts_with("c_"));
        assert!(input.event_id.starts_with("e_"));
        assert_eq!(input.created_at, 100);
        Ok(CommentRecord {
            id: input.id,
            board_id: "b_default".into(),
            task_id: task_id.into(),
            author: input.author,
            author_type: input.author_type,
            agent_type: input.agent_type,
            body: input.body,
            kind: input.kind,
            metadata_json: input.metadata_json,
            created_at: input.created_at,
        })
    }

    async fn list_comments(&self, task_id: &str) -> Result<Vec<CommentRecord>> {
        assert_eq!(task_id, "t_comment");
        Ok(vec![CommentRecord {
            id: "c_comment".into(),
            board_id: "b_default".into(),
            task_id: task_id.into(),
            author: "alice".into(),
            author_type: CommentAuthorType::User,
            agent_type: None,
            body: "handoff".into(),
            kind: CommentKind::Note,
            metadata_json: r#"{"source":"test"}"#.into(),
            created_at: 100,
        }])
    }

    async fn add_dependency(
        &self,
        _child_task_id: &str,
        _parent_task_id: &str,
        _input: AddDependencyRecord,
    ) -> Result<AddDependencyResult> {
        Err(KanbanError::FeatureNotAvailable(
            "dependency stub is not configured".to_owned(),
        ))
    }

    async fn list_dependencies(&self, _task_id: &str) -> Result<DependencySnapshotRecord> {
        Err(KanbanError::FeatureNotAvailable(
            "dependency stub is not configured".to_owned(),
        ))
    }

    async fn remove_dependency(
        &self,
        _child_task_id: &str,
        _parent_task_id: &str,
        _actor: String,
        _event_id: String,
        _now: i64,
    ) -> Result<RemoveDependencyResult> {
        Err(KanbanError::FeatureNotAvailable(
            "dependency stub is not configured".to_owned(),
        ))
    }

    async fn create_step(&self, task_id: &str, input: CreateStepRecord) -> Result<StepRecord> {
        assert_eq!(task_id, "t_step");
        assert!(input.id.starts_with("step_"));
        assert_eq!(input.idempotency_key.as_deref(), Some("step-retry"));
        assert_eq!(input.title, "step title");
        assert_eq!(input.body.as_deref(), Some("step body"));
        assert_eq!(input.target_status, TaskStatus::Ready);
        assert!(input.event_id.starts_with("e_"));
        assert!(input.plan_event_id.starts_with("e_"));
        assert!(input.recompute_event_id.starts_with("e_"));
        Ok(StepRecord {
            id: input.id,
            parent_task_id: task_id.to_owned(),
            title: input.title,
            body: input.body,
            linked_task: None,
            position: input.position.unwrap_or(1024),
            required: input.required,
            status: "todo".to_owned(),
            resolution_note: None,
            resolved_by: None,
            resolved_at: None,
            created_by: input.created_by.clone(),
            created_at: input.created_at,
            updated_by: input.created_by,
            updated_at: input.created_at,
        })
    }

    async fn list_steps(&self, task_id: &str) -> Result<TaskStepsRecord> {
        assert_eq!(task_id, "t_step");
        Ok(TaskStepsRecord {
            task_id: task_id.to_owned(),
            steps: vec![],
            execution_plan: ExecutionPlanRecord {
                board_id: "b_default".into(),
                task_id: task_id.to_owned(),
                state: ExecutionPlanState::Planned,
                reason: None,
                updated_by: "tester".into(),
                updated_at: 100,
            },
        })
    }

    async fn update_step(
        &self,
        task_id: &str,
        step_id: &str,
        input: UpdateStepRecord,
    ) -> Result<StepRecord> {
        assert_eq!(task_id, "t_step");
        assert!(step_id.starts_with("step_"));
        assert_eq!(input.title.as_deref(), Some("updated step"));
        assert!(input.body.is_none());
        assert_eq!(input.position, Some(2048));
        assert_eq!(input.required, Some(false));
        assert!(input.event_id.starts_with("e_"));
        assert_eq!(input.updated_by, "tester");
        assert_eq!(input.updated_at, 100);
        assert_eq!(input.expected_lock_version, 0);
        Ok(StepRecord {
            id: step_id.to_owned(),
            parent_task_id: task_id.to_owned(),
            title: input.title.unwrap_or_else(|| "step title".to_owned()),
            body: input.body,
            linked_task: None,
            position: input.position.unwrap_or(1024),
            required: input.required.unwrap_or(true),
            status: "todo".to_owned(),
            resolution_note: None,
            resolved_by: None,
            resolved_at: None,
            created_by: "tester".to_owned(),
            created_at: 100,
            updated_by: input.updated_by,
            updated_at: input.updated_at,
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) struct FixedClock(pub(crate) i64);

impl Clock for FixedClock {
    fn now_ms(&self) -> i64 {
        self.0
    }
}

pub(crate) fn task_record(input: CreateTaskRecord) -> TaskRecord {
    TaskRecord {
        id: input.id,
        board_id: "b_default".into(),
        board_slug: "default".into(),
        task_ref: "default#1".into(),
        seq: 1,
        title: input.title,
        description: input.description,
        status: input.status,
        status_reason: None,
        assignee: input.assignee,
        priority: input.priority,
        position: 1024,
        scheduled_at: input.scheduled_at,
        due_at: input.due_at,
        created_by: input.created_by,
        created_at: 100,
        updated_at: 100,
        started_at: None,
        completed_at: None,
        archived_at: None,
        has_claim_token: false,
        claim_owner: None,
        claim_expires_at: None,
        last_heartbeat_at: None,
        current_run_id: None,
        retry_count: 0,
        max_retries: input.max_retries,
        result_summary: None,
        result_json: None,
        metadata_json: input.metadata_json,
        lock_version: 0,
        dependency_blocked: false,
        unfinished_parent_count: 0,
        execution_plan_state: ExecutionPlanState::Unplanned,
        required_step_count: 0,
        completed_required_step_count: 0,
        optional_step_count: 0,
    }
}

pub(crate) fn task_for_id(task_id: &str) -> TaskRecord {
    let mut task = task_record(CreateTaskRecord {
        id: task_id.to_owned(),
        idempotency_key: None,
        title: "Promote".into(),
        description: Some("ready spec".into()),
        status: TaskStatus::Todo,
        assignee: None,
        priority: 1,
        scheduled_at: None,
        due_at: None,
        max_retries: None,
        metadata_json: "{}".into(),
        created_by: "tester".into(),
    });
    match task_id {
        "t_promote" => task.execution_plan_state = ExecutionPlanState::NotRequired,
        "t_claim" => {
            task.status = TaskStatus::Ready;
            task.execution_plan_state = ExecutionPlanState::NotRequired;
        }
        "t_claimed" => {
            task.status = TaskStatus::Ready;
            task.execution_plan_state = ExecutionPlanState::NotRequired;
            task.has_claim_token = true;
        }
        "t_claim_dependency" => {
            task.status = TaskStatus::Ready;
            task.execution_plan_state = ExecutionPlanState::NotRequired;
            task.dependency_blocked = true;
            task.unfinished_parent_count = 1;
        }
        "t_claim_unplanned" => task.status = TaskStatus::Ready,
        "t_heartbeat" => {
            task.status = TaskStatus::Running;
            task.execution_plan_state = ExecutionPlanState::NotRequired;
            task.has_claim_token = true;
            task.claim_owner = Some("worker".into());
            task.claim_expires_at = Some(200);
            task.last_heartbeat_at = Some(100);
            task.current_run_id = Some("r_heartbeat".into());
            task.started_at = Some(50);
            task.lock_version = 2;
        }
        "t_release"
        | "t_release_unplanned"
        | "t_release_dependency"
        | "t_release_future"
        | "t_review" => {
            task.status = TaskStatus::Running;
            task.execution_plan_state = if task_id == "t_release_unplanned" {
                ExecutionPlanState::Unplanned
            } else {
                ExecutionPlanState::NotRequired
            };
            task.has_claim_token = true;
            task.claim_owner = Some("worker".into());
            task.claim_expires_at = Some(200);
            task.last_heartbeat_at = Some(100);
            task.current_run_id = Some(if task_id == "t_review" {
                "r_review".into()
            } else {
                "r_release".into()
            });
            task.started_at = Some(50);
            task.lock_version = 2;
            if task_id == "t_release_dependency" {
                task.dependency_blocked = true;
                task.unfinished_parent_count = 1;
            }
            if task_id == "t_release_future" {
                task.scheduled_at = Some(200);
            }
        }
        "t_complete_running" | "t_complete_steps" => {
            task.status = TaskStatus::Running;
            task.execution_plan_state = ExecutionPlanState::NotRequired;
            task.has_claim_token = true;
            task.claim_owner = Some("worker".into());
            task.claim_expires_at = Some(200);
            task.last_heartbeat_at = Some(100);
            task.current_run_id = Some("r_complete".into());
            task.started_at = Some(50);
            task.lock_version = 2;
            if task_id == "t_complete_steps" {
                task.required_step_count = 2;
                task.completed_required_step_count = 1;
            }
        }
        "t_complete_review" => {
            task.status = TaskStatus::Review;
            task.execution_plan_state = ExecutionPlanState::NotRequired;
            task.current_run_id = Some("r_complete".into());
            task.started_at = Some(50);
            task.lock_version = 3;
        }
        "t_block_running" => {
            task.status = TaskStatus::Running;
            task.execution_plan_state = ExecutionPlanState::NotRequired;
            task.has_claim_token = true;
            task.claim_owner = Some("worker".into());
            task.claim_expires_at = Some(200);
            task.last_heartbeat_at = Some(100);
            task.current_run_id = Some("r_block".into());
            task.started_at = Some(50);
            task.lock_version = 2;
        }
        "t_block_todo" => {}
        "t_block_done" => task.status = TaskStatus::Done,
        "t_future" => {
            task.status = TaskStatus::Scheduled;
            task.scheduled_at = Some(200);
            task.execution_plan_state = ExecutionPlanState::NotRequired;
        }
        "t_running" => {
            task.status = TaskStatus::Running;
            task.execution_plan_state = ExecutionPlanState::NotRequired;
            task.has_claim_token = true;
            task.claim_owner = Some("worker".into());
            task.claim_expires_at = Some(200);
            task.current_run_id = Some("r_running".into());
        }
        _ => {}
    }
    task
}
