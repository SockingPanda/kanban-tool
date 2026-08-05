use crate::shared::{KanbanMcp, call_client};
use kanban_protocol::{ReclaimTargetStatus, ReclaimTaskRequest, ReclaimTaskResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskReclaimArgs {
    board: Option<String>,
    task_ref: String,
    #[serde(default)]
    force: bool,
    to_status: Option<ReclaimTargetStatus>,
    reason: Option<String>,
}
#[tool_router(router = task_reclaim_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_reclaim",
        description = "回收过期或强制回收 running claim"
    )]
    async fn task_reclaim(
        &self,
        Parameters(args): Parameters<TaskReclaimArgs>,
    ) -> Result<Json<ReclaimTaskResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let task = call_client(move || {
            client.reclaim_task_by_selector(
                &board,
                &args.task_ref,
                &ReclaimTaskRequest {
                    actor: None,
                    force: args.force,
                    to_status: args.to_status,
                    reason: args.reason,
                },
            )
        })
        .await?;
        Ok(Json(ReclaimTaskResponse::new(task)))
    }
}
