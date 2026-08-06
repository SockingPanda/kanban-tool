use kanban_protocol::{MarkExecutionPlanNotRequiredRequest, MarkExecutionPlanNotRequiredResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskPlanNotRequiredArgs {
    /// task_ref 使用 board-local 值时采用的 board。默认使用 KB_BOARD/default。
    board: Option<String>,
    /// 全局 t_... ID、board#seq、#seq 或数字 board-local 序号。
    task_ref: String,
    reason: String,
}

#[tool_router(router = task_plan_not_required_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_plan_not_required",
        description = "通过 canonical application service 将任务 execution plan 标记为 not required"
    )]
    async fn task_plan_not_required(
        &self,
        Parameters(args): Parameters<TaskPlanNotRequiredArgs>,
    ) -> Result<Json<MarkExecutionPlanNotRequiredResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let plan = call_client(move || {
            client.mark_execution_plan_not_required_by_selector(
                &board,
                &args.task_ref,
                &MarkExecutionPlanNotRequiredRequest {
                    reason: args.reason,
                    actor: None,
                },
            )
        })
        .await?;
        Ok(Json(MarkExecutionPlanNotRequiredResponse { data: plan }))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn task_plan_not_required_tool_is_independently_locatable() {
        let tools = KanbanMcp::task_plan_not_required_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "task_plan_not_required");
    }
}
