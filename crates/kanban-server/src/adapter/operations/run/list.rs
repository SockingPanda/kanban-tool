use kanban_application::{RunList, RunRecord as ApplicationRun};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, application_run, store_error};

impl RunList for TursoApplicationStore {
    async fn list_runs(&self, task_id: &str) -> Result<Vec<ApplicationRun>> {
        self.store
            .list_runs(task_id)
            .await
            .map_err(store_error)?
            .into_iter()
            .map(application_run)
            .collect()
    }
}
