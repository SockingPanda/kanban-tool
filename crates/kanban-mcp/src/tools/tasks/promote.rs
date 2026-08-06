use kanban_protocol::{PromoteTaskRequest, PromoteTaskResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskPromoteArgs {
    /// task_ref 使用 board-local 值时采用的 board。默认使用 KB_BOARD/default。
    board: Option<String>,
    /// 全局 t_... ID、board#seq、#seq 或数字 board-local 序号。
    task_ref: String,
}

#[tool_router(router = task_promote_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_promote",
        description = "通过 canonical application service 将符合条件的任务 promote 为 ready"
    )]
    async fn task_promote(
        &self,
        Parameters(args): Parameters<TaskPromoteArgs>,
    ) -> Result<Json<PromoteTaskResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let task = call_client(move || {
            client.promote_task_by_selector(
                &board,
                &args.task_ref,
                &PromoteTaskRequest { actor: None },
            )
        })
        .await?;
        Ok(Json(PromoteTaskResponse::new(task)))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn task_promote_tool_is_independently_locatable() {
        let tools = KanbanMcp::task_promote_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "task_promote");
    }
}
