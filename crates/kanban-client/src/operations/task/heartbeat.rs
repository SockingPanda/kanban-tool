use kanban_protocol::{ApiTask, HeartbeatTaskRequest};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn heartbeat_task(
        &self,
        task_id: &str,
        request: &HeartbeatTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: kanban_protocol::HeartbeatTaskResponse = rpc!(
            self,
            heartbeat_task,
            HeartbeatTaskRequest,
            kanban_protocol::HeartbeatTaskPath {
                task_id: task_id.trim().to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }

    pub async fn heartbeat_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &HeartbeatTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.heartbeat_task(&task_id, request).await
    }
}
