use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};
use kanban_contract::{ApiTask, SpecifyTaskRequest, SpecifyTaskResponse};

impl KanbanClient {
    pub fn specify_task(
        &self,
        task_id: &str,
        request: &SpecifyTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: SpecifyTaskResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/transitions/specify",
                encode_path_segment(task_id.trim())
            ),
            request,
        )?;
        Ok(response.data)
    }
    pub fn specify_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &SpecifyTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.specify_task(&task_id, request)
    }
}
