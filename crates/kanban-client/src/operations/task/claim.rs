use kanban_protocol::{ApiClaim, ClaimTaskRequest};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn claim_task(
        &self,
        task_id: &str,
        request: &ClaimTaskRequest,
    ) -> Result<ApiClaim, ClientError> {
        let response: kanban_protocol::ClaimTaskResponse = rpc!(
            self,
            claim_task,
            ClaimTaskRequest,
            kanban_protocol::ClaimTaskPath {
                task_id: task_id.trim().to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }

    pub async fn claim_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &ClaimTaskRequest,
    ) -> Result<ApiClaim, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.claim_task(&task_id, request).await
    }
}
