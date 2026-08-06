use kanban_protocol::{CompleteTaskRequest, CompleteTaskResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskDoneArgs {
    /// task_ref 使用 board-local 值时采用的 board。默认使用 KB_BOARD/default。
    board: Option<String>,
    /// 全局 t_... ID、board#seq、#seq 或数字 board-local 序号。
    task_ref: String,
    /// 从 running 阶段完成时由 task_claim 返回的 exact token。
    claim_token: Option<String>,
    /// 绕过 running caller credentials，但不绕过 required-step guards。
    #[serde(default)]
    force: bool,
    /// 可选 summary，会存储在 task 和 active run 上。
    summary: Option<String>,
    /// 可选的不透明 JSON result，会存储在 task 和 completion event 上。
    result: Option<serde_json::Value>,
}

#[tool_router(router = task_done_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_done",
        description = "通过 canonical application service 完成 running 或 reviewed task"
    )]
    async fn task_done(
        &self,
        Parameters(args): Parameters<TaskDoneArgs>,
    ) -> Result<Json<CompleteTaskResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let task = call_client(move || {
            client.complete_task_by_selector(
                &board,
                &args.task_ref,
                &CompleteTaskRequest {
                    actor: None,
                    claim_token: args.claim_token,
                    force: args.force,
                    summary: args.summary,
                    result: args.result,
                },
            )
        })
        .await?;
        Ok(Json(CompleteTaskResponse::new(task)))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn task_done_tool_is_independently_locatable() {
        let tools = KanbanMcp::task_done_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "task_done");
    }
}
