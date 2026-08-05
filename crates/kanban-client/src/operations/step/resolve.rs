use kanban_protocol::{
    ApiTaskStep, ApiTaskSteps, CompleteStepRequest, CompleteStepResponse, ReopenStepRequest,
    ReopenStepResponse, SkipStepRequest, SkipStepResponse,
};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn complete_step(
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
        let response: CompleteStepResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/steps/{}/done",
                encode_path_segment(task_id),
                encode_path_segment(step_id)
            ),
            &request,
        )?;
        Ok(response.data)
    }

    pub fn complete_step_by_selector(
        &self,
        board: &str,
        task_selector: &str,
        step_selector: &str,
        request: &CompleteStepRequest,
    ) -> Result<ApiTaskStep, ClientError> {
        let task_id = self.resolve_task_id(board, task_selector)?;
        let step_id = self.resolve_step_id(&task_id, step_selector)?;
        let steps = self.complete_step(&task_id, &step_id, request)?;
        select_step(steps, &step_id)
    }

    pub fn skip_step(
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
        let response: SkipStepResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/steps/{}/skip",
                encode_path_segment(task_id),
                encode_path_segment(step_id)
            ),
            &request,
        )?;
        Ok(response.data)
    }

    pub fn skip_step_by_selector(
        &self,
        board: &str,
        task_selector: &str,
        step_selector: &str,
        request: &SkipStepRequest,
    ) -> Result<ApiTaskStep, ClientError> {
        let task_id = self.resolve_task_id(board, task_selector)?;
        let step_id = self.resolve_step_id(&task_id, step_selector)?;
        let steps = self.skip_step(&task_id, &step_id, request)?;
        select_step(steps, &step_id)
    }

    pub fn reopen_step(
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
        let response: ReopenStepResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/steps/{}/reopen",
                encode_path_segment(task_id),
                encode_path_segment(step_id)
            ),
            &request,
        )?;
        Ok(response.data)
    }

    pub fn reopen_step_by_selector(
        &self,
        board: &str,
        task_selector: &str,
        step_selector: &str,
        request: &ReopenStepRequest,
    ) -> Result<ApiTaskStep, ClientError> {
        let task_id = self.resolve_task_id(board, task_selector)?;
        let step_id = self.resolve_step_id(&task_id, step_selector)?;
        let steps = self.reopen_step(&task_id, &step_id, request)?;
        select_step(steps, &step_id)
    }
}

fn validate_ids(task_id: &str, step_id: &str) -> Result<(), ClientError> {
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
