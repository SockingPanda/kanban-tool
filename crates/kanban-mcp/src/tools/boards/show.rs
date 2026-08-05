use kanban_contract::GetBoardResponse;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client_internal};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct BoardShowArgs {
    /// Board slug or id. Defaults to KB_BOARD/default.
    board: Option<String>,
}

#[tool_router(router = board_show_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "board_show",
        description = "Show a board, including archived history"
    )]
    async fn board_show(
        &self,
        Parameters(args): Parameters<BoardShowArgs>,
    ) -> Result<Json<GetBoardResponse>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        let result = call_client_internal(move || client.get_board(&board)).await?;
        Ok(Json(GetBoardResponse { data: result }))
    }
}
