use kanban_contract::{ApiTaskSteps, RemoveStepResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn remove_step(&self, task_id: &str, step_id: &str) -> Result<ApiTaskSteps, ClientError> {
        let task_id = task_id.trim();
        let step_id = step_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(ClientError::InvalidInput(
                "task selector 必须解析为全局 t_... ID".to_owned(),
            ));
        }
        if !step_id.starts_with("step_") || step_id.len() <= 5 {
            return Err(ClientError::InvalidInput(
                "step selector 必须解析为全局 step_... ID".to_owned(),
            ));
        }
        let response: RemoveStepResponse = self.delete(&format!(
            "/api/v1/tasks/{}/steps/{}",
            encode_path_segment(task_id),
            encode_path_segment(step_id)
        ))?;
        Ok(response.data)
    }

    pub fn remove_step_by_selector(
        &self,
        board: &str,
        task_selector: &str,
        step_selector: &str,
    ) -> Result<ApiTaskSteps, ClientError> {
        let task_id = self.resolve_task_id(board, task_selector)?;
        let step_id = self.resolve_step_id(&task_id, step_selector)?;
        self.remove_step(&task_id, &step_id)
    }
}
