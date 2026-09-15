use crate::{KanbanClient, error::ClientError, transport::rpc};
use kanban_protocol::{ApiTask, ReclaimTaskRequest};

impl KanbanClient {
    pub async fn reclaim_task(
        &self,
        task_id: &str,
        request: &ReclaimTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: kanban_protocol::ReclaimTaskResponse = rpc!(
            self,
            reclaim_task,
            ReclaimTaskRequest,
            kanban_protocol::ReclaimTaskPath {
                task_id: task_id.trim().to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }
    pub async fn reclaim_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &ReclaimTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.reclaim_task(&task_id, request).await
    }
}
