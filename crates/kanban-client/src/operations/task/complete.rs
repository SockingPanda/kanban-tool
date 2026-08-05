use kanban_contract::{ApiTask, CompleteTaskRequest, CompleteTaskResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn complete_task(
        &self,
        task_id: &str,
        request: &CompleteTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: CompleteTaskResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/transitions/complete",
                encode_path_segment(task_id.trim())
            ),
            request,
        )?;
        Ok(response.data)
    }

    pub fn complete_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &CompleteTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.complete_task(&task_id, request)
    }
}
