use kanban_protocol::{ApiTask, CreateTaskRequest};

use crate::{KanbanClient, error::ClientError, shared::prepare_create_request, transport::rpc};

impl KanbanClient {
    pub async fn create_task(
        &self,
        board: &str,
        request: CreateTaskRequest,
    ) -> Result<ApiTask, ClientError> {
        if !request.module_ids.is_empty()
            || request.cycle_id.is_some()
            || !request.expected_versions.is_empty()
            || request.expected_catalog_version.is_some()
        {
            self.require_planning(false).await?;
        }
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
