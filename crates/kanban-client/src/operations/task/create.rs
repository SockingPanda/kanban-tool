use kanban_protocol::{ApiTask, CreateTaskRequest, CreateTaskResponse};

use crate::{
    KanbanClient, error::ClientError, shared::prepare_create_request,
    transport::encode_path_segment,
};

impl KanbanClient {
    pub fn create_task(
        &self,
        board: &str,
        request: CreateTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let request = prepare_create_request(request);
        let path = format!("/api/v1/boards/{}/tasks", encode_path_segment(board));
        let response: CreateTaskResponse = self.post(&path, &request)?;
        Ok(response.data)
    }
}
