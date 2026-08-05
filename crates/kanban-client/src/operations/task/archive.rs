use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};
use kanban_contract::{ApiTask, ArchiveTaskRequest, ArchiveTaskResponse};

impl KanbanClient {
    pub fn archive_task(
        &self,
        task_id: &str,
        request: &ArchiveTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: ArchiveTaskResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/transitions/archive",
                encode_path_segment(task_id.trim())
            ),
            request,
        )?;
        Ok(response.data)
    }
    pub fn archive_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &ArchiveTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.archive_task(&task_id, request)
    }
}
