use kanban_protocol::{ReleaseTaskRequest, ReleaseTaskResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskReleaseArgs {
    /// task_ref 使用 board-local 值时采用的 board。默认使用 KB_BOARD/default。
    board: Option<String>,
    /// 全局 t_... ID、board#seq、#seq 或数字 board-local 序号。
    task_ref: String,
    /// task_claim 返回的 exact token。
    claim_token: String,
}

#[tool_router(router = task_release_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_release",
        description = "通过 canonical application service 将 active claim 的 task 返回 ready"
    )]
    async fn task_release(
        &self,
        Parameters(args): Parameters<TaskReleaseArgs>,
    ) -> Result<Json<ReleaseTaskResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let task = call_client(move || {
            client.release_task_by_selector(
                &board,
                &args.task_ref,
                &ReleaseTaskRequest {
                    actor: None,
                    claim_token: args.claim_token,
                },
            )
        })
        .await?;
        Ok(Json(ReleaseTaskResponse::new(task)))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn task_release_tool_is_independently_locatable() {
        let tools = KanbanMcp::task_release_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "task_release");
    }
}
