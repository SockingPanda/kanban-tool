use kanban_protocol::{ApiTaskSteps, CreateStepRequest};

use crate::{
    KanbanClient, error::ClientError, shared::prepare_create_step_request, transport::rpc,
};

impl KanbanClient {
    pub async fn create_step(
        &self,
        task_id: &str,
        request: &CreateStepRequest,
    ) -> Result<ApiTaskSteps, ClientError> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(ClientError::InvalidInput(
                "任务选择器必须解析为全局 t_... ID".to_owned(),
            ));
        }
        let request = prepare_create_step_request(request.clone());
        let response: kanban_protocol::CreateStepResponse = rpc!(
            self,
            create_step,
            CreateStepRequest,
            kanban_protocol::CreateStepPath {
                task_id: task_id.to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }

    pub async fn create_step_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &CreateStepRequest,
    ) -> Result<ApiTaskSteps, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        let mut request = request.clone();
        if let Some(linked_task_ref) = request.linked_task_ref.as_deref() {
            let linked_task_id = self.resolve_task_id(board, linked_task_ref).await?;
            request.linked_task_ref = Some(linked_task_id);
        }
        self.create_step(&task_id, &request).await
    }
}
