use kanban_protocol::{ApiTask, SubmitReviewTaskRequest, SubmitReviewTaskResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn submit_review_task(
        &self,
        task_id: &str,
        request: &SubmitReviewTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let response: SubmitReviewTaskResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/transitions/submit-review",
                encode_path_segment(task_id.trim())
            ),
            request,
        )?;
        Ok(response.data)
    }

    pub fn submit_review_task_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &SubmitReviewTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.submit_review_task(&task_id, request)
    }
}
