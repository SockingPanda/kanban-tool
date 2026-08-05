use kanban_protocol::{ApiTaskSteps, ListStepsResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn list_steps(&self, task_id: &str) -> Result<ApiTaskSteps, ClientError> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(ClientError::InvalidInput(
                "task selector must resolve to a global t_... id".to_owned(),
            ));
        }
        let response: ListStepsResponse = self.get(&format!(
            "/api/v1/tasks/{}/steps",
            encode_path_segment(task_id)
        ))?;
        Ok(response.data)
    }

    pub fn list_steps_by_selector(
        &self,
        board: &str,
        selector: &str,
    ) -> Result<ApiTaskSteps, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.list_steps(&task_id)
    }
}
