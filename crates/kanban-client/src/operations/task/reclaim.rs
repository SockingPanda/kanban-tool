use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};
use kanban_protocol::{ApiTask, ReclaimTaskRequest, ReclaimTaskResponse};

impl KanbanClient {
    pub fn reclaim_task(
        &self,
        task_id: &str,
        request: &ReclaimTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: ReclaimTaskResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/transitions/reclaim",
                encode_path_segment(task_id.trim())
            ),
            request,
        )?;
        Ok(response.data)
    }
    pub fn reclaim_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &ReclaimTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.reclaim_task(&task_id, request)
    }
}
