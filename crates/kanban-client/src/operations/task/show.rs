use kanban_contract::{ApiTask, GetTaskDetailsResponse, GetTaskResponse, TaskDetailAggregate};

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

    pub fn get_task_details(&self, task_id: &str) -> Result<TaskDetailAggregate, ClientError> {
        let response: GetTaskDetailsResponse = self.get(&format!(
            "/api/v1/tasks/{}?include=details",
            encode_path_segment(task_id.trim())
        ))?;
        Ok(response.data)
    }

    pub fn get_task_details_by_selector(
        &self,
        board: &str,
        selector: &str,
    ) -> Result<TaskDetailAggregate, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.get_task_details(&task_id)
    }
}
