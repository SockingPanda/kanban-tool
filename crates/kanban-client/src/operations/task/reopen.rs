use crate::{KanbanClient, error::ClientError, transport::rpc};
use kanban_protocol::{ApiTask, ReopenTaskRequest};

impl KanbanClient {
    pub async fn reopen_task(
        &self,
        task_id: &str,
        request: &ReopenTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: kanban_protocol::ReopenTaskResponse = rpc!(
            self,
            reopen_task,
            ReopenTaskRequest,
            kanban_protocol::ReopenTaskPath {
                task_id: task_id.trim().to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }
    pub async fn reopen_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &ReopenTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.reopen_task(&task_id, request).await
    }
}
