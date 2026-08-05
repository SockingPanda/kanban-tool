use kanban_protocol::{ApiClaim, ClaimTaskRequest, ClaimTaskResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn claim_task(
        &self,
        task_id: &str,
        request: &ClaimTaskRequest,
    ) -> Result<ApiClaim, ClientError> {
        let response: ClaimTaskResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/transitions/claim",
                encode_path_segment(task_id.trim())
            ),
            request,
        )?;
        Ok(response.data)
    }

    pub fn claim_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &ClaimTaskRequest,
    ) -> Result<ApiClaim, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.claim_task(&task_id, request)
    }
}
