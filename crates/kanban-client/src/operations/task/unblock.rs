use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};
use kanban_contract::{ApiTask, UnblockTaskRequest, UnblockTaskResponse};

impl KanbanClient {
    pub fn unblock_task(
        &self,
        task_id: &str,
        request: &UnblockTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: UnblockTaskResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/transitions/unblock",
                encode_path_segment(task_id.trim())
            ),
            request,
        )?;
        Ok(response.data)
    }
    pub fn unblock_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &UnblockTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.unblock_task(&task_id, request)
    }
}
