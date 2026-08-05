use kanban_contract::{ApiTask, GetTaskResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn get_task(&self, task_id: &str) -> Result<ApiTask, ClientError> {
        let response: GetTaskResponse = self.get(&format!(
            "/api/v1/tasks/{}",
            encode_path_segment(task_id.trim())
        ))?;
        Ok(response.data)
    }

    pub fn get_task_by_selector(
        &self,
        board: &str,
        selector: &str,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.get_task(&task_id)
    }
}
