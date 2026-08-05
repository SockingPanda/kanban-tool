use crate::UpdateStepInput as StoreUpdateStep;
use crate::{
    StepRecord as ApplicationStep, StepUpdate, TaskStepsRecord as ApplicationTaskSteps,
    UpdateStepRecord as ApplicationUpdateStep,
};
use kanban_core::Result;

use crate::adapter::{
    TursoApplicationStore, application_execution_plan, application_step, application_task,
    store_error,
};

impl StepUpdate for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<crate::TaskRecord> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn update_step(
        &self,
        task_id: &str,
        step_id: &str,
        input: ApplicationUpdateStep,
    ) -> Result<ApplicationStep> {
        self.store
            .update_step(
                task_id,
                step_id,
                StoreUpdateStep {
                    title: input.title,
                    body: input.body,
                    linked_task_id: input.linked_task_id,
                    unlink_task: input.unlink_task,
                    position: input.position,
                    required: input.required,
                    updated_by: input.updated_by,
                    event_id: input.event_id,
                    updated_at: input.updated_at,
                    expected_lock_version: input.expected_lock_version,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_step)
    }

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
