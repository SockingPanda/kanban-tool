use crate::{KanbanClient, error::ClientError, transport::rpc};
use kanban_protocol::{ApiTask, SpecifyTaskRequest};

impl KanbanClient {
    pub async fn specify_task(
        &self,
        task_id: &str,
        request: &SpecifyTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: kanban_protocol::SpecifyTaskResponse = rpc!(
            self,
            specify_task,
            SpecifyTaskRequest,
            kanban_protocol::SpecifyTaskPath {
                task_id: task_id.trim().to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }
    pub async fn specify_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &SpecifyTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.specify_task(&task_id, request).await
    }
}
