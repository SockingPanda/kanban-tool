use kanban_protocol::{ApiTask, SubmitReviewTaskRequest};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn submit_review_task(
        &self,
        task_id: &str,
        request: &SubmitReviewTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: kanban_protocol::SubmitReviewTaskResponse = rpc!(
            self,
            submit_review_task,
            SubmitReviewTaskRequest,
            kanban_protocol::SubmitReviewTaskPath {
                task_id: task_id.trim().to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }

    pub async fn submit_review_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &SubmitReviewTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.submit_review_task(&task_id, request).await
    }
}
