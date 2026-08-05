use kanban_contract::{ApiTask, BlockTaskRequest, BlockTaskResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn block_task(
        &self,
        task_id: &str,
        request: &BlockTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: BlockTaskResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/transitions/block",
                encode_path_segment(task_id.trim())
            ),
            request,
        )?;
        Ok(response.data)
    }

    pub fn block_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &BlockTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.block_task(&task_id, request)
    }
}
