use crate::{
    ExecutionPlanRecord as ApplicationExecutionPlan,
    MarkExecutionPlanNotRequiredRecord as ApplicationMarkExecutionPlanNotRequired,
    TaskPlanNotRequired,
};
use kanban_core::Result;
use crate::MarkExecutionPlanNotRequiredInput as StoreMarkExecutionPlanNotRequired;

use crate::adapter::{TursoApplicationStore, application_execution_plan, store_error};

impl TaskPlanNotRequired for TursoApplicationStore {
    async fn mark_execution_plan_not_required(
        &self,
        task_id: &str,
        input: ApplicationMarkExecutionPlanNotRequired,
    ) -> Result<ApplicationExecutionPlan> {
        self.store
            .mark_execution_plan_not_required(
                task_id,
                StoreMarkExecutionPlanNotRequired {
                    reason: input.reason,
                    actor: input.actor,
                    event_id: input.event_id,
                    updated_at: input.updated_at,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_execution_plan)
    }
}
