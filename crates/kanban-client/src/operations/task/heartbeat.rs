use kanban_contract::{ApiTask, HeartbeatTaskRequest, HeartbeatTaskResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn heartbeat_task(
        &self,
        task_id: &str,
        request: &HeartbeatTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: HeartbeatTaskResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/transitions/heartbeat",
                encode_path_segment(task_id.trim())
            ),
            request,
        )?;
        Ok(response.data)
    }

    pub fn heartbeat_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &HeartbeatTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.heartbeat_task(&task_id, request)
    }
}
