use kanban_contract::ListStepsResponse;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct StepListArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
}

#[tool_router(router = step_list_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "step_list",
        description = "List execution-plan steps from the canonical application service"
    )]
    async fn step_list(
        &self,
        Parameters(args): Parameters<StepListArgs>,
    ) -> Result<Json<ListStepsResponse>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let client = self.client.clone();
        let steps = call_client(move || client.list_steps_by_selector(&board, &task_ref)).await?;
        Ok(Json(ListStepsResponse { data: steps }))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn step_list_tool_is_independently_locatable() {
        let tools = KanbanMcp::step_list_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "step_list");
    }
}
