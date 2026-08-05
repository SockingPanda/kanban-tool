use kanban_contract::{ApiTask, PromoteTaskRequest, PromoteTaskResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn promote_task(
        &self,
        task_id: &str,
        request: &PromoteTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: PromoteTaskResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/transitions/promote",
                encode_path_segment(task_id.trim())
            ),
            request,
        )?;
        Ok(response.data)
    }

    pub fn promote_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &PromoteTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.promote_task(&task_id, request)
    }
}
