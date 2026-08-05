use crate::shared::{KanbanMcp, call_client};
use kanban_contract::{SpecifyTaskRequest, SpecifyTaskResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskSpecifyArgs {
    board: Option<String>,
    task_ref: String,
    description: Option<String>,
    scheduled_at: Option<i64>,
}
#[tool_router(router = task_specify_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(name = "task_specify", description = "补充 triage 任务规格并重算状态")]
    async fn task_specify(
        &self,
        Parameters(args): Parameters<TaskSpecifyArgs>,
    ) -> Result<Json<SpecifyTaskResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let task = call_client(move || {
            client.specify_task_by_selector(
                &board,
                &args.task_ref,
                &SpecifyTaskRequest {
                    actor: None,
                    description: args.description,
                    scheduled_at: args.scheduled_at,
                },
            )
        })
        .await?;
        Ok(Json(SpecifyTaskResponse::new(task)))
    }
}
