use kanban_protocol::GetBoardResponse;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct BoardShowArgs {
    /// 当前有效看板的 slug 或 ID；默认使用 KB_BOARD/default。
    board: Option<String>,
}

#[tool_router(router = board_show_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(name = "board_show", description = "查看当前有效看板")]
    async fn board_show(
        &self,
        Parameters(args): Parameters<BoardShowArgs>,
    ) -> Result<Json<GetBoardResponse>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        let result = call_client(move || client.get_board(&board)).await?;
        Ok(Json(GetBoardResponse { data: result }))
    }
}
