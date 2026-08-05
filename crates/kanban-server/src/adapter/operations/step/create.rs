use kanban_application::{
    CreateStepRecord as ApplicationCreateStep, ExecutionPlanState, StepCreate,
    StepRecord as ApplicationStep, TaskStepsRecord as ApplicationTaskSteps,
};
use kanban_core::Result;
use kanban_store_turso::CreateStepInput as StoreCreateStep;

use crate::adapter::{
    TursoApplicationStore, application_execution_plan, application_step, application_task,
    store_error,
};

impl StepCreate for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<kanban_application::TaskRecord> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn create_step(
        &self,
        task_id: &str,
        input: ApplicationCreateStep,
    ) -> Result<ApplicationStep> {
        self.store
            .create_step(
                task_id,
                StoreCreateStep {
                    id: input.id,
                    idempotency_key: input.idempotency_key,
                    title: input.title,
                    body: input.body,
                    linked_task_id: input.linked_task_id,
                    position: input.position,
                    required: input.required,
                    created_by: input.created_by,
                    event_id: input.event_id,
                    plan_event_id: input.plan_event_id,
                    recompute_event_id: input.recompute_event_id,
                    created_at: input.created_at,
                    expected_lock_version: input.expected_lock_version,
                    expected_plan_state: match input.expected_plan_state {
                        ExecutionPlanState::Unplanned => "unplanned",
                        ExecutionPlanState::Planned => "planned",
                        ExecutionPlanState::NotRequired => "not_required",
                    }
                    .to_owned(),
                    target_status: input.target_status.as_str().to_owned(),
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
