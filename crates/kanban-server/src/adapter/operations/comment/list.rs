use kanban_application::{CommentList, CommentRecord as ApplicationComment};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, application_comment, store_error};

impl CommentList for TursoApplicationStore {
    async fn list_comments(&self, task_id: &str) -> Result<Vec<ApplicationComment>> {
        self.store
            .list_comments(task_id)
            .await
            .map_err(store_error)?
            .into_iter()
            .map(application_comment)
            .collect()
    }
}
