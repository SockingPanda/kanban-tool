use kanban_contract::{ApiTask, ReleaseTaskRequest, ReleaseTaskResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn release_task(
        &self,
        task_id: &str,
        request: &ReleaseTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: ReleaseTaskResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/transitions/release",
                encode_path_segment(task_id.trim())
            ),
            request,
        )?;
        Ok(response.data)
    }

    pub fn release_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &ReleaseTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.release_task(&task_id, request)
    }
}
