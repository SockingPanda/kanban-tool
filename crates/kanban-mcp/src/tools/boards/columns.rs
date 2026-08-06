use kanban_protocol::ListBoardColumnsResponse;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct BoardColumnsArgs {
    /// Board slug 或 ID。默认使用 KB_BOARD/default。
    board: Option<String>,
}

#[tool_router(router = board_columns_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "board_columns",
        description = "通过 canonical kanban host 列出 board columns"
    )]
    async fn board_columns(
        &self,
        Parameters(args): Parameters<BoardColumnsArgs>,
    ) -> Result<Json<ListBoardColumnsResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let columns = call_client(move || client.list_board_columns(&board)).await?;
        Ok(Json(ListBoardColumnsResponse { data: columns }))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn board_columns_tool_is_independently_locatable() {
        let tools = KanbanMcp::board_columns_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "board_columns");
    }
}
