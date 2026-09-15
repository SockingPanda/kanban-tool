use crate::{KanbanClient, error::ClientError, transport::rpc};
use kanban_protocol::{ApiTask, UpdateTaskRequest};

impl KanbanClient {
    pub async fn update_task(
        &self,
        task_id: &str,
        request: &UpdateTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: kanban_protocol::UpdateTaskResponse = rpc!(
            self,
            update_task,
            UpdateTaskRequest,
            kanban_protocol::UpdateTaskPath {
                task_id: task_id.trim().to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }
    pub async fn update_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &UpdateTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.update_task(&task_id, request).await
    }
}
