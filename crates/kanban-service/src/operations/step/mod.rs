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
            .map(crate::adapter::application_step)
            .collect::<crate::Result<Vec<_>>>()?,
        execution_plan: crate::adapter::application_execution_plan(steps.execution_plan)?,
    })
}
