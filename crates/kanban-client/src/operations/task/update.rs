use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};
use kanban_protocol::{ApiTask, UpdateTaskRequest, UpdateTaskResponse};

impl KanbanClient {
    pub fn update_task(
        &self,
        task_id: &str,
        request: &UpdateTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: UpdateTaskResponse = self.patch(
            &format!("/api/v1/tasks/{}", encode_path_segment(task_id.trim())),
            request,
        )?;
        Ok(response.data)
    }
    pub fn update_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &UpdateTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.update_task(&task_id, request)
    }
}
