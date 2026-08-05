use kanban_application::{StepList, TaskStepsRecord as ApplicationTaskSteps};
use kanban_core::Result;

use crate::adapter::{
    TursoApplicationStore, application_execution_plan, application_step, store_error,
};

impl StepList for TursoApplicationStore {
    async fn list_steps(&self, task_id: &str) -> Result<ApplicationTaskSteps> {
        let steps = self.store.list_steps(task_id).await.map_err(store_error)?;
        Ok(ApplicationTaskSteps {
            task_id: steps.task_id,
            steps: steps
                .steps
                .into_iter()
                .map(application_step)
                .collect::<Result<Vec<_>>>()?,
            execution_plan: application_execution_plan(steps.execution_plan)?,
        })
    }
}
