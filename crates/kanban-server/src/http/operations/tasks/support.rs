use crate::error::ApiError;
use kanban_service::{ExecutionPlanRecord, ExecutionPlanState, LabelRecord, TaskRecord};
use kanban_service::{KanbanError, TaskStatus};
use kanban_protocol::{
    ApiExecutionPlan, ApiExecutionPlanState, ApiStepStatus, ApiTask, ApiTaskPriority,
    ApiTaskStatus, ApiTaskStep,
};

pub(crate) fn api_task_step(step: kanban_service::StepRecord) -> Result<ApiTaskStep, ApiError> {
    let status = match step.status.as_str() {
        "todo" => ApiStepStatus::Todo,
        "done" => ApiStepStatus::Done,
        "skipped" => ApiStepStatus::Skipped,
        other => {
            return Err(
                KanbanError::Storage(format!("stored step status is invalid: {other}")).into(),
            );
        }
    };
    Ok(ApiTaskStep {
        id: step.id,
        parent_task_id: step.parent_task_id,
        title: step.title,
        body: step.body,
        linked_task: step.linked_task.map(api_task).transpose()?,
        position: step.position,
        required: step.required,
        status,
        resolution_note: step.resolution_note,
        resolved_by: step.resolved_by,
        resolved_at: step.resolved_at,
        created_by: step.created_by,
        created_at: step.created_at,
        updated_by: step.updated_by,
        updated_at: step.updated_at,
    })
}

pub(crate) fn api_task(task: TaskRecord) -> Result<ApiTask, ApiError> {
    let priority = ApiTaskPriority::try_from(task.priority).map_err(|priority| {
        KanbanError::Storage(format!("stored task priority is outside 0..=3: {priority}"))
    })?;
    let metadata = serde_json::from_str(&task.metadata_json).map_err(|error| {
        KanbanError::Storage(format!("stored task metadata is invalid JSON: {error}"))
    })?;
    let result = task
        .result_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|error| {
            KanbanError::Storage(format!("stored task result is invalid JSON: {error}"))
        })?;
    Ok(ApiTask {
        id: task.id,
        board_id: task.board_id,
        board_slug: task.board_slug,
        task_ref: task.task_ref,
        seq: task.seq,
        title: task.title,
        description: task.description,
        status: api_task_status(task.status),
        status_reason: task.status_reason,
        assignee: task.assignee,
        priority,
        position: task.position,
        scheduled_at: task.scheduled_at,
        due_at: task.due_at,
        created_by: task.created_by,
        created_at: task.created_at,
        updated_at: task.updated_at,
        started_at: task.started_at,
        completed_at: task.completed_at,
        archived_at: task.archived_at,
        claim_owner: task.claim_owner,
        claim_expires_at: task.claim_expires_at,
        last_heartbeat_at: task.last_heartbeat_at,
        current_run_id: task.current_run_id,
        retry_count: task.retry_count,
        max_retries: task.max_retries,
        result_summary: task.result_summary,
        result,
        metadata,
        lock_version: task.lock_version,
        dependency_blocked: task.dependency_blocked,
        unfinished_parent_count: task.unfinished_parent_count,
        execution_plan_state: match task.execution_plan_state {
            ExecutionPlanState::Unplanned => ApiExecutionPlanState::Unplanned,
            ExecutionPlanState::Planned => ApiExecutionPlanState::Planned,
            ExecutionPlanState::NotRequired => ApiExecutionPlanState::NotRequired,
        },
        required_step_count: task.required_step_count,
        completed_required_step_count: task.completed_required_step_count,
        optional_step_count: task.optional_step_count,
        labels: task.labels.into_iter().map(api_label).collect(),
    })
}

pub(crate) fn api_label(label: LabelRecord) -> kanban_protocol::ApiLabel {
    kanban_protocol::ApiLabel {
        id: label.id,
        board_id: label.board_id,
        name: label.name,
        color: label.color,
        created_at: label.created_at,
        updated_at: label.updated_at,
    }
}

pub(crate) fn api_execution_plan(plan: ExecutionPlanRecord) -> ApiExecutionPlan {
    ApiExecutionPlan {
        board_id: plan.board_id,
        task_id: plan.task_id,
        state: match plan.state {
            ExecutionPlanState::Unplanned => ApiExecutionPlanState::Unplanned,
            ExecutionPlanState::Planned => ApiExecutionPlanState::Planned,
            ExecutionPlanState::NotRequired => ApiExecutionPlanState::NotRequired,
        },
        reason: plan.reason,
        updated_by: plan.updated_by,
        updated_at: plan.updated_at,
    }
}

pub(crate) fn api_task_status(status: TaskStatus) -> ApiTaskStatus {
    match status {
        TaskStatus::Triage => ApiTaskStatus::Triage,
        TaskStatus::Todo => ApiTaskStatus::Todo,
        TaskStatus::Scheduled => ApiTaskStatus::Scheduled,
        TaskStatus::Ready => ApiTaskStatus::Ready,
        TaskStatus::Running => ApiTaskStatus::Running,
        TaskStatus::Blocked => ApiTaskStatus::Blocked,
        TaskStatus::Review => ApiTaskStatus::Review,
        TaskStatus::Done => ApiTaskStatus::Done,
        TaskStatus::Archived => ApiTaskStatus::Archived,
    }
}
