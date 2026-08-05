use kanban_contract::{BlockTaskRequest, BlockTaskResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskBlockArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
    /// Required reason recorded on the task, failed run, and event.
    reason: String,
    /// Exact token returned by task_claim when blocking from running.
    claim_token: Option<String>,
    /// Bypass running caller credentials without bypassing task/run consistency.
    #[serde(default)]
    force: bool,
}

#[tool_router(router = task_block_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_block",
        description = "Block an active task through the canonical application service"
    )]
    async fn task_block(
        &self,
        Parameters(args): Parameters<TaskBlockArgs>,
    ) -> Result<Json<BlockTaskResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let task = call_client(move || {
            client.block_task_by_selector(
                &board,
                &args.task_ref,
                &BlockTaskRequest {
                    actor: None,
                    reason: args.reason,
                    claim_token: args.claim_token,
                    force: args.force,
                },
            )
        })
        .await?;
        Ok(Json(BlockTaskResponse::new(task)))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn task_block_tool_is_independently_locatable() {
        let tools = KanbanMcp::task_block_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "task_block");
    }
}
