mod create;
mod list;
mod remove;
mod resolve;
mod update;

pub use create::CreateStepCommand;
pub use remove::RemoveStepCommand;
pub use resolve::{CompleteStepCommand, ReopenStepCommand, SkipStepCommand};
pub use update::UpdateStepCommand;

pub(crate) fn application_steps(
    steps: crate::domain::TaskStepsRecord,
) -> crate::Result<crate::TaskStepsRecord> {
    Ok(crate::TaskStepsRecord {
        task_id: steps.task_id,
        steps: steps
            .steps
            .into_iter()
            .map(application_step)
            .collect::<crate::Result<Vec<_>>>()?,
        execution_plan: application_execution_plan(steps.execution_plan)?,
    })
}

pub(crate) fn application_execution_plan(
    plan: crate::domain::TaskExecutionPlanRecord,
) -> crate::Result<crate::ExecutionPlanRecord> {
    let state = match plan.state.as_str() {
        "unplanned" => crate::ExecutionPlanState::Unplanned,
        "planned" => crate::ExecutionPlanState::Planned,
        "not_required" => crate::ExecutionPlanState::NotRequired,
        other => {
            return Err(crate::KanbanError::Storage(format!(
                "stored execution plan state is invalid: {other}"
            )));
        }
    };
    Ok(crate::ExecutionPlanRecord {
        board_id: plan.board_id,
        task_id: plan.task_id,
        state,
        reason: plan.reason,
        updated_by: plan.updated_by,
        updated_at: plan.updated_at,
    })
}

pub(crate) fn application_step(
    step: crate::domain::TaskStepRecord,
) -> crate::Result<crate::StepRecord> {
    Ok(crate::StepRecord {
        id: step.id,
        parent_task_id: step.parent_task_id,
        title: step.title,
        body: step.body,
        linked_task: step.linked_task.map(super::application_task).transpose()?,
        position: step.position,
        required: step.required,
        status: step.status,
        resolution_note: step.resolution_note,
        resolved_by: step.resolved_by,
        resolved_at: step.resolved_at,
        created_by: step.created_by,
        created_at: step.created_at,
        updated_by: step.updated_by,
        updated_at: step.updated_at,
    })
}
