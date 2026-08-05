use crate::shared::{KanbanMcp, call_client};
use kanban_contract::{UnblockTaskRequest, UnblockTaskResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskUnblockArgs {
    board: Option<String>,
    task_ref: String,
}
#[tool_router(router = task_unblock_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(name = "task_unblock", description = "解除 blocked 任务并重算状态")]
    async fn task_unblock(
        &self,
        Parameters(args): Parameters<TaskUnblockArgs>,
    ) -> Result<Json<UnblockTaskResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let task = call_client(move || {
            client.unblock_task_by_selector(
                &board,
                &args.task_ref,
                &UnblockTaskRequest { actor: None },
            )
        })
        .await?;
        Ok(Json(UnblockTaskResponse::new(task)))
    }
}
