use crate::shared::{KanbanMcp, call_client};
use kanban_protocol::{ReopenTaskRequest, ReopenTaskResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskReopenArgs {
    board: Option<String>,
    task_ref: String,
    reason: String,
}
#[tool_router(router = task_reopen_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(name = "task_reopen", description = "重新打开已完成任务")]
    async fn task_reopen(
        &self,
        Parameters(args): Parameters<TaskReopenArgs>,
    ) -> Result<Json<ReopenTaskResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let task = call_client(move || {
            client.reopen_task_by_selector(
                &board,
                &args.task_ref,
                &ReopenTaskRequest {
                    actor: None,
                    reason: args.reason,
                },
            )
        })
        .await?;
        Ok(Json(ReopenTaskResponse::new(task)))
    }
}
