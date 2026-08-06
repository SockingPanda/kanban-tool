use crate::{
    CompleteStepInput as StoreCompleteStep, ReopenStepInput as StoreReopenStep,
    SkipStepInput as StoreSkipStep,
};
use crate::{
    StepRecord as ApplicationStep, TaskStepsRecord as ApplicationTaskSteps,
    operations::{
        CompleteStepRecord as ApplicationCompleteStep, ReopenStepRecord as ApplicationReopenStep,
        SkipStepRecord as ApplicationSkipStep, StepComplete, StepReopen, StepSkip,
    },
};
use kanban_core::Result;

use crate::adapter::{
    TursoApplicationStore, application_execution_plan, application_step, store_error,
};
use crate::operations::application_task;

fn application_steps(steps: crate::domain::TaskStepsRecord) -> Result<ApplicationTaskSteps> {
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

impl StepComplete for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<crate::TaskRecord> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn complete_step(
        &self,
        task_id: &str,
        step_id: &str,
        input: ApplicationCompleteStep,
    ) -> Result<ApplicationStep> {
        self.store
            .complete_step(
                task_id,
                step_id,
                StoreCompleteStep {
                    note: input.note,
                    actor: input.actor,
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
        application_steps(self.store.list_steps(task_id).await.map_err(store_error)?)
    }
}

impl StepSkip for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<crate::TaskRecord> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn skip_step(
        &self,
        task_id: &str,
        step_id: &str,
        input: ApplicationSkipStep,
    ) -> Result<ApplicationStep> {
        self.store
            .skip_step(
                task_id,
                step_id,
                StoreSkipStep {
                    reason: input.reason,
                    actor: input.actor,
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
        application_steps(self.store.list_steps(task_id).await.map_err(store_error)?)
    }
}

impl StepReopen for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<crate::TaskRecord> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn reopen_step(
        &self,
        task_id: &str,
        step_id: &str,
        input: ApplicationReopenStep,
    ) -> Result<ApplicationStep> {
        self.store
            .reopen_step(
                task_id,
                step_id,
                StoreReopenStep {
                    reason: input.reason,
                    actor: input.actor,
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
        application_steps(self.store.list_steps(task_id).await.map_err(store_error)?)
    }
}
