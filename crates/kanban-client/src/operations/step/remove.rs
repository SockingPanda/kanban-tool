use kanban_protocol::ApiTaskSteps;

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn remove_step(
        &self,
        task_id: &str,
        step_id: &str,
    ) -> Result<ApiTaskSteps, ClientError> {
        let task_id = task_id.trim();
        let step_id = step_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(ClientError::InvalidInput(
                "任务选择器必须解析为全局 t_... ID".to_owned(),
            ));
        }
        if !step_id.starts_with("step_") || step_id.len() <= 5 {
            return Err(ClientError::InvalidInput(
                "步骤选择器必须解析为全局 step_... ID".to_owned(),
            ));
        }
        let response: kanban_protocol::RemoveStepResponse = rpc!(
            self,
            remove_step,
            RemoveStepRequest,
            kanban_protocol::RemoveStepPath {
                task_id: task_id.to_owned(),
                step_id: step_id.to_owned()
            },
            (),
            ()
        )?;
        Ok(response.data)
    }

    pub async fn remove_step_by_selector(
        &self,
        board: &str,
        task_selector: &str,
        step_selector: &str,
    ) -> Result<ApiTaskSteps, ClientError> {
        let task_id = self.resolve_task_id(board, task_selector).await?;
        let step_id = self.resolve_step_id(&task_id, step_selector).await?;
        self.remove_step(&task_id, &step_id).await
    }
}
