use kanban_protocol::{ApiTaskSteps, CreateStepRequest, CreateStepResponse};

use crate::{
    KanbanClient, error::ClientError, shared::prepare_create_step_request,
    transport::encode_path_segment,
};

impl KanbanClient {
    pub fn create_step(
        &self,
        task_id: &str,
        request: &CreateStepRequest,
    ) -> Result<ApiTaskSteps, ClientError> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(ClientError::InvalidInput(
                "task selector must resolve to a global t_... id".to_owned(),
            ));
        }
        let request = prepare_create_step_request(request.clone());
        let response: CreateStepResponse = self.post(
            &format!("/api/v1/tasks/{}/steps", encode_path_segment(task_id)),
            &request,
        )?;
        Ok(response.data)
    }

    pub fn create_step_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &CreateStepRequest,
    ) -> Result<ApiTaskSteps, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        let mut request = request.clone();
        if let Some(linked_task_ref) = request.linked_task_ref.as_deref() {
            let linked_task_id = self.resolve_task_id(board, linked_task_ref)?;
            request.linked_task_ref = Some(linked_task_id);
        }
        self.create_step(&task_id, &request)
    }
}
