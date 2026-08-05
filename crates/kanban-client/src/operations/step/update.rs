use kanban_contract::{ApiTaskSteps, UpdateStepRequest, UpdateStepResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn update_step(
        &self,
        task_id: &str,
        step_id: &str,
        request: &UpdateStepRequest,
    ) -> Result<ApiTaskSteps, ClientError> {
        let task_id = task_id.trim();
        let step_id = step_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(ClientError::InvalidInput(
                "task selector must resolve to a global t_... id".to_owned(),
            ));
        }
        if !step_id.starts_with("step_") || step_id.len() <= 5 {
            return Err(ClientError::InvalidInput(
                "step selector must resolve to a global step_... id".to_owned(),
            ));
        }
        let mut request = request.clone();
        request.actor = Some(self.actor.clone());
        let response: UpdateStepResponse = self.patch(
            &format!(
                "/api/v1/tasks/{}/steps/{}",
                encode_path_segment(task_id),
                encode_path_segment(step_id)
            ),
            &request,
        )?;
        Ok(response.data)
    }

    pub fn update_step_by_selector(
        &self,
        board: &str,
        task_selector: &str,
        step_selector: &str,
        request: &UpdateStepRequest,
    ) -> Result<ApiTaskSteps, ClientError> {
        let task_id = self.resolve_task_id(board, task_selector)?;
        let step_id = self.resolve_step_id(&task_id, step_selector)?;
        let mut request = request.clone();
        if let Some(linked_task_ref) = request.linked_task_ref.as_deref() {
            let linked_task_id = self.resolve_task_id(board, linked_task_ref)?;
            request.linked_task_ref = Some(linked_task_id);
        }
        self.update_step(&task_id, &step_id, &request)
    }
}
