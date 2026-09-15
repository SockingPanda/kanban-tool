use crate::{KanbanClient, error::ClientError, transport::rpc};
use kanban_protocol::{ApiTask, ArchiveTaskRequest};

impl KanbanClient {
    pub async fn archive_task(
        &self,
        task_id: &str,
        request: &ArchiveTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: kanban_protocol::ArchiveTaskResponse = rpc!(
            self,
            archive_task,
            ArchiveTaskRequest,
            kanban_protocol::ArchiveTaskPath {
                task_id: task_id.trim().to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }
    pub async fn archive_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &ArchiveTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.archive_task(&task_id, request).await
    }
}
