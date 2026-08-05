use kanban_application::{
    DependencyRemove, RemoveDependencyResult as ApplicationRemoveDependencyResult,
};
use kanban_core::Result;
use kanban_store_turso::RemoveDependencyInput as StoreRemoveDependency;

use crate::adapter::{TursoApplicationStore, application_dependency_snapshot, store_error};

impl DependencyRemove for TursoApplicationStore {
    async fn remove_dependency(
        &self,
        child_task_id: &str,
        parent_task_id: &str,
        actor: String,
        event_id: String,
        now: i64,
    ) -> Result<ApplicationRemoveDependencyResult> {
        let result = self
            .store
            .remove_dependency(
                child_task_id,
                parent_task_id,
                StoreRemoveDependency {
                    actor,
                    event_id,
                    now,
                },
            )
            .await
            .map_err(store_error)?;
        Ok(ApplicationRemoveDependencyResult {
            removed: result.removed,
            dependencies: application_dependency_snapshot(result.dependencies)?,
        })
    }
}
