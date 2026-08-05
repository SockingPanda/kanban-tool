use crate::AddDependencyInput as StoreAddDependency;
use crate::{
    AddDependencyRecord as ApplicationAddDependency,
    AddDependencyResult as ApplicationAddDependencyResult, DependencyCreate,
    TaskRecord as ApplicationTask,
};
use kanban_core::Result;

use crate::adapter::{
    TursoApplicationStore, application_dependency_snapshot, application_task, store_error,
};

impl DependencyCreate for TursoApplicationStore {
    async fn get_task(&self, task_id: &str) -> Result<ApplicationTask> {
        self.store
            .get_task_global(task_id)
            .await
            .map_err(store_error)
            .and_then(application_task)
    }

    async fn add_dependency(
        &self,
        child_task_id: &str,
        parent_task_id: &str,
        input: ApplicationAddDependency,
    ) -> Result<ApplicationAddDependencyResult> {
        let result = self
            .store
            .add_dependency(
                child_task_id,
                parent_task_id,
                StoreAddDependency {
                    expected_child_lock_version: input.expected_child_lock_version,
                    target_child_status: input.target_child_status.as_str().to_owned(),
                    actor: input.actor,
                    event_id: input.event_id,
                    recompute_event_id: input.recompute_event_id,
                    now: input.now,
                },
            )
            .await
            .map_err(store_error)?;
        Ok(ApplicationAddDependencyResult {
            added: result.added,
            dependencies: application_dependency_snapshot(result.dependencies)?,
        })
    }
}
