use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};
use kanban_contract::{ApiTask, ReopenTaskRequest, ReopenTaskResponse};

impl KanbanClient {
    pub fn reopen_task(
        &self,
        task_id: &str,
        request: &ReopenTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: ReopenTaskResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/transitions/reopen",
                encode_path_segment(task_id.trim())
            ),
            request,
        )?;
        Ok(response.data)
    }
    pub fn reopen_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &ReopenTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.reopen_task(&task_id, request)
    }
}
