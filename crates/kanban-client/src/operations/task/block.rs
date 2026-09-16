use kanban_protocol::{ApiTask, BlockTaskRequest};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn block_task(
        &self,
        task_id: &str,
        request: &BlockTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: kanban_protocol::BlockTaskResponse = rpc!(
            self,
            block_task,
            BlockTaskRequest,
            kanban_protocol::BlockTaskPath {
                task_id: task_id.trim().to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }

    pub async fn block_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &BlockTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.block_task(&task_id, request).await
    }
}
