use kanban_protocol::{ApiTask, CompleteTaskRequest};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn complete_task(
        &self,
        task_id: &str,
        request: &CompleteTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: kanban_protocol::CompleteTaskResponse = rpc!(
            self,
            complete_task,
            CompleteTaskRequest,
            kanban_protocol::CompleteTaskPath {
                task_id: task_id.trim().to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }

    pub async fn complete_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &CompleteTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.complete_task(&task_id, request).await
    }
}
