use kanban_protocol::{
    ApiTaskStep, ApiTaskSteps, CompleteStepRequest, ReopenStepRequest, SkipStepRequest,
};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn complete_step(
        &self,
        task_id: &str,
        step_id: &str,
        request: &CompleteStepRequest,
    ) -> Result<ApiTaskSteps, ClientError> {
        let task_id = task_id.trim();
        let step_id = step_id.trim();
        validate_ids(task_id, step_id)?;
        let mut request = request.clone();
        request.actor = Some(self.actor.clone());
        let response: kanban_protocol::CompleteStepResponse = rpc!(
            self,
            complete_step,
            CompleteStepRequest,
            kanban_protocol::CompleteStepPath {
                task_id: task_id.to_owned(),
                step_id: step_id.to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }

    pub async fn complete_step_by_selector(
        &self,
        board: &str,
        task_selector: &str,
        step_selector: &str,
        request: &CompleteStepRequest,
    ) -> Result<ApiTaskStep, ClientError> {
        let task_id = self.resolve_task_id(board, task_selector).await?;
        let step_id = self.resolve_step_id(&task_id, step_selector).await?;
        let steps = self.complete_step(&task_id, &step_id, request).await?;
        select_step(steps, &step_id)
    }

    pub async fn skip_step(
        &self,
        task_id: &str,
        step_id: &str,
        request: &SkipStepRequest,
    ) -> Result<ApiTaskSteps, ClientError> {
        let task_id = task_id.trim();
        let step_id = step_id.trim();
        validate_ids(task_id, step_id)?;
        let mut request = request.clone();
        request.actor = Some(self.actor.clone());
        let response: kanban_protocol::SkipStepResponse = rpc!(
            self,
            skip_step,
            SkipStepRequest,
            kanban_protocol::SkipStepPath {
                task_id: task_id.to_owned(),
                step_id: step_id.to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }

    pub async fn skip_step_by_selector(
        &self,
        board: &str,
        task_selector: &str,
        step_selector: &str,
        request: &SkipStepRequest,
    ) -> Result<ApiTaskStep, ClientError> {
        let task_id = self.resolve_task_id(board, task_selector).await?;
        let step_id = self.resolve_step_id(&task_id, step_selector).await?;
        let steps = self.skip_step(&task_id, &step_id, request).await?;
        select_step(steps, &step_id)
    }

    pub async fn reopen_step(
        &self,
        task_id: &str,
        step_id: &str,
        request: &ReopenStepRequest,
    ) -> Result<ApiTaskSteps, ClientError> {
        let task_id = task_id.trim();
        let step_id = step_id.trim();
        validate_ids(task_id, step_id)?;
        let mut request = request.clone();
        request.actor = Some(self.actor.clone());
        let response: kanban_protocol::ReopenStepResponse = rpc!(
            self,
            reopen_step,
            ReopenStepRequest,
            kanban_protocol::ReopenStepPath {
                task_id: task_id.to_owned(),
                step_id: step_id.to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }

    pub async fn reopen_step_by_selector(
        &self,
        board: &str,
        task_selector: &str,
        step_selector: &str,
        request: &ReopenStepRequest,
    ) -> Result<ApiTaskStep, ClientError> {
        let task_id = self.resolve_task_id(board, task_selector).await?;
        let step_id = self.resolve_step_id(&task_id, step_selector).await?;
        let steps = self.reopen_step(&task_id, &step_id, request).await?;
        select_step(steps, &step_id)
    }
}

fn validate_ids(task_id: &str, step_id: &str) -> Result<(), ClientError> {
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
    Ok(())
}

fn select_step(steps: ApiTaskSteps, step_id: &str) -> Result<ApiTaskStep, ClientError> {
    steps
        .steps
        .into_iter()
        .find(|step| step.id == step_id)
        .ok_or_else(|| {
            ClientError::InvalidResponse(format!("step 响应缺少已解析的 step {step_id}"))
        })
}
