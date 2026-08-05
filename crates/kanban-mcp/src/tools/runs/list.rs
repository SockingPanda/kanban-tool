use kanban_contract::ListRunsResponse;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RunListArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
}

#[tool_router(router = run_list_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "run_list",
        description = "List task runs through the canonical kanban application service"
    )]
    async fn run_list(
        &self,
        Parameters(args): Parameters<RunListArgs>,
    ) -> Result<Json<ListRunsResponse>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let client = self.client.clone();
        let runs = call_client(move || client.list_runs_by_selector(&board, &task_ref)).await?;
        Ok(Json(ListRunsResponse { data: runs }))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn run_list_tool_is_independently_locatable() {
        let tools = KanbanMcp::run_list_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "run_list");
    }
}
