use kanban_protocol::{
    ApiExecutionPlan, MarkExecutionPlanNotRequiredRequest, MarkExecutionPlanNotRequiredResponse,
};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn mark_execution_plan_not_required(
        &self,
        task_id: &str,
        request: &MarkExecutionPlanNotRequiredRequest,
    ) -> Result<ApiExecutionPlan, ClientError> {
        let response: MarkExecutionPlanNotRequiredResponse = self.post(
            &format!(
                "/api/v1/tasks/{}/execution-plan/not-required",
                encode_path_segment(task_id.trim())
            ),
            request,
        )?;
        Ok(response.data)
    }

    pub fn mark_execution_plan_not_required_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &MarkExecutionPlanNotRequiredRequest,
    ) -> Result<ApiExecutionPlan, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.mark_execution_plan_not_required(&task_id, request)
    }
}
