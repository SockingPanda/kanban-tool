use crate::{KanbanClient, error::ClientError, transport::rpc};
use kanban_protocol::{ApiTask, UnblockTaskRequest};

impl KanbanClient {
    pub async fn unblock_task(
        &self,
        task_id: &str,
        request: &UnblockTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: kanban_protocol::UnblockTaskResponse = rpc!(
            self,
            unblock_task,
            UnblockTaskRequest,
            kanban_protocol::UnblockTaskPath {
                task_id: task_id.trim().to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }
    pub async fn unblock_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &UnblockTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.unblock_task(&task_id, request).await
    }
}
