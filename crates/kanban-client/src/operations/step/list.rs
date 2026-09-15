use kanban_protocol::ApiTaskSteps;

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn list_steps(&self, task_id: &str) -> Result<ApiTaskSteps, ClientError> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(ClientError::InvalidInput(
                "任务选择器必须解析为全局 t_... ID".to_owned(),
            ));
        }
        let response: kanban_protocol::ListStepsResponse = rpc!(
            self,
            list_steps,
            ListStepsRequest,
            kanban_protocol::ListStepsPath {
                task_id: task_id.to_owned()
            },
            (),
            ()
        )?;
        Ok(response.data)
    }

    pub async fn list_steps_by_selector(
        &self,
        board: &str,
        selector: &str,
    ) -> Result<ApiTaskSteps, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.list_steps(&task_id).await
    }
}
