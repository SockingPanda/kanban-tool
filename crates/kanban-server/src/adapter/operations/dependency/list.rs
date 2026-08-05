use kanban_application::{
    DependencyList, DependencySnapshotRecord as ApplicationDependencySnapshot,
};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, application_dependency_snapshot, store_error};

impl DependencyList for TursoApplicationStore {
    async fn list_dependencies(&self, task_id: &str) -> Result<ApplicationDependencySnapshot> {
        self.store
            .list_dependencies(task_id)
            .await
            .map_err(store_error)
            .and_then(application_dependency_snapshot)
    }
}
