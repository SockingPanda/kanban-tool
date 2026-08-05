use kanban_application::{
    RemoveStepRecord as ApplicationRemoveStep, StepRecord as ApplicationStep, StepRemove,
};
use kanban_core::Result;
use kanban_store_turso::RemoveStepInput as StoreRemoveStep;

use crate::adapter::{TursoApplicationStore, application_step, store_error};

impl StepRemove for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<kanban_application::TaskRecord> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(crate::adapter::application_task)
    }

    async fn remove_step(
        &self,
        task_id: &str,
        step_id: &str,
        input: ApplicationRemoveStep,
    ) -> Result<ApplicationStep> {
        self.store
            .remove_step(
                task_id,
                step_id,
                StoreRemoveStep {
                    actor: input.actor,
                    event_id: input.event_id,
                    recompute_event_id: input.recompute_event_id,
                    updated_at: input.updated_at,
                    expected_lock_version: input.expected_lock_version,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_step)
    }

    async fn list_steps(&self, task_id: &str) -> Result<kanban_application::TaskStepsRecord> {
        let steps = self.store.list_steps(task_id).await.map_err(store_error)?;
        Ok(kanban_application::TaskStepsRecord {
            task_id: steps.task_id,
            steps: steps
                .steps
                .into_iter()
                .map(application_step)
                .collect::<Result<Vec<_>>>()?,
            execution_plan: crate::adapter::application_execution_plan(steps.execution_plan)?,
        })
    }
}
