use kanban_contract::GetTaskResponse;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskShowArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
}

#[tool_router(router = task_show_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_show",
        description = "Show one task through the canonical kanban application service"
    )]
    async fn task_show(
        &self,
        Parameters(args): Parameters<TaskShowArgs>,
    ) -> Result<Json<GetTaskResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let task = call_client(move || client.get_task_by_selector(&board, &args.task_ref)).await?;
        Ok(Json(GetTaskResponse::new(task, None)))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn task_show_tool_is_independently_locatable() {
        let tools = KanbanMcp::task_show_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "task_show");
    }
}
