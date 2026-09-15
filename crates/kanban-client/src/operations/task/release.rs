use kanban_protocol::{ApiTask, ReleaseTaskRequest};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn release_task(
        &self,
        task_id: &str,
        request: &ReleaseTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: kanban_protocol::ReleaseTaskResponse = rpc!(
            self,
            release_task,
            ReleaseTaskRequest,
            kanban_protocol::ReleaseTaskPath {
                task_id: task_id.trim().to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }

    pub async fn release_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &ReleaseTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.release_task(&task_id, request).await
    }
}
