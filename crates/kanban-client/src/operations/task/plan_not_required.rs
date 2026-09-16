use kanban_protocol::{ApiExecutionPlan, MarkExecutionPlanNotRequiredRequest};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn mark_execution_plan_not_required(
        &self,
        task_id: &str,
        request: &MarkExecutionPlanNotRequiredRequest,
    ) -> Result<ApiExecutionPlan, ClientError> {
        let response: kanban_protocol::MarkExecutionPlanNotRequiredResponse = rpc!(
            self,
            mark_execution_plan_not_required,
            MarkExecutionPlanNotRequiredRequest,
            kanban_protocol::MarkExecutionPlanNotRequiredPath {
                task_id: task_id.trim().to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }

    pub async fn mark_execution_plan_not_required_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &MarkExecutionPlanNotRequiredRequest,
    ) -> Result<ApiExecutionPlan, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.mark_execution_plan_not_required(&task_id, request)
            .await
    }
}
