use kanban_protocol::{BlockTaskRequest, BlockTaskResponse};
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
    /// task_ref 使用 board-local 值时采用的 board。默认使用 KB_BOARD/default。
    board: Option<String>,
    /// 全局 t_... ID、board#seq、#seq 或数字 board-local 序号。
    task_ref: String,
    /// 必填 reason，会记录在 task、failed run 和 event 上。
    reason: String,
    /// 从 running 阶段阻塞时由 task_claim 返回的 exact token。
    claim_token: Option<String>,
    /// 绕过 running caller credentials，但不绕过 task/run consistency。
    #[serde(default)]
    force: bool,
}

#[tool_router(router = task_block_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_block",
        description = "通过 canonical application service 阻塞 active task"
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
