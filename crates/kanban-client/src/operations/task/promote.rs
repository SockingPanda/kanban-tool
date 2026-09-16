use kanban_protocol::{ApiTask, PromoteTaskRequest};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn promote_task(
        &self,
        task_id: &str,
        request: &PromoteTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: kanban_protocol::PromoteTaskResponse = rpc!(
            self,
            promote_task,
            PromoteTaskRequest,
            kanban_protocol::PromoteTaskPath {
                task_id: task_id.trim().to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }

    pub async fn promote_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &PromoteTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.promote_task(&task_id, request).await
    }
}
