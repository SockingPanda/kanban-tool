use kanban_application::{RunRecord as ApplicationRun, RunShow};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, application_run, store_error};

impl RunShow for TursoApplicationStore {
    async fn get_run(&self, run_id: &str) -> Result<ApplicationRun> {
        self.store
            .get_run(run_id)
            .await
            .map_err(store_error)
            .and_then(application_run)
    }
}
