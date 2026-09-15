use kanban_protocol::{ApiTask, CreateTaskRequest};

use crate::{KanbanClient, error::ClientError, shared::prepare_create_request, transport::rpc};

impl KanbanClient {
    pub async fn create_task(
        &self,
        board: &str,
        request: CreateTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        let request = prepare_create_request(request);
        let response: kanban_protocol::CreateTaskResponse = rpc!(
            self,
            create_task,
            CreateTaskRequest,
            kanban_protocol::CreateTaskPath {
                board: board.to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }
}
