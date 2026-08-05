use crate::error::ApiError;
use crate::http::operations::tasks::support::{api_execution_plan, api_task_step};
use kanban_protocol::ApiTaskSteps;

pub(super) fn api_task_steps(
    steps: kanban_service::TaskStepsRecord,
) -> Result<ApiTaskSteps, ApiError> {
    Ok(ApiTaskSteps {
        task_id: steps.task_id,
        steps: steps
            .steps
            .into_iter()
            .map(api_task_step)
            .collect::<Result<Vec<_>, _>>()?,
        execution_plan: api_execution_plan(steps.execution_plan),
    })
}
