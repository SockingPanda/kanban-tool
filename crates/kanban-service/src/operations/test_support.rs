use std::sync::{Arc, atomic::AtomicUsize};

use kanban_core::{Clock, TaskStatus};

use crate::*;

#[derive(Clone)]
pub(crate) struct StubStore {
    pub(crate) calls: Arc<AtomicUsize>,
}

impl ApplicationStore for StubStore {}

#[derive(Clone, Copy)]
pub(crate) struct FixedClock(pub(crate) i64);

impl Clock for FixedClock {
    fn now_ms(&self) -> i64 {
        self.0
    }
}

fn base_task(task_id: &str) -> TaskRecord {
    TaskRecord {
        id: task_id.to_owned(),
        board_id: "b_default".into(),
        board_slug: "default".into(),
        task_ref: "default#1".into(),
        seq: 1,
        title: "Promote".into(),
        description: Some("ready spec".into()),
        status: TaskStatus::Todo,
        status_reason: None,
        assignee: None,
        priority: 1,
        position: 1024,
        scheduled_at: None,
        due_at: None,
        created_by: "tester".into(),
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
        max_retries: None,
        result_summary: None,
        result_json: None,
        metadata_json: "{}".into(),
        lock_version: 0,
        dependency_blocked: false,
        unfinished_parent_count: 0,
        execution_plan_state: ExecutionPlanState::Unplanned,
        required_step_count: 0,
        completed_required_step_count: 0,
        optional_step_count: 0,
        labels: Vec::new(),
    }
}

pub(crate) fn task_for_id(task_id: &str) -> TaskRecord {
    let mut task = base_task(task_id);
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
