use kanban_contract::ListBoardsResponse;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client_internal};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct BoardListArgs {
    /// Include archived boards in the result.
    include_archived: bool,
}

#[tool_router(router = board_list_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "board_list",
        description = "List boards from the canonical kanban host"
    )]
    async fn board_list(
        &self,
        Parameters(args): Parameters<BoardListArgs>,
    ) -> Result<Json<ListBoardsResponse>, McpError> {
        let client = self.client.clone();
        let boards =
            call_client_internal(move || client.list_boards(args.include_archived)).await?;

        Ok(Json(ListBoardsResponse { data: boards }))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn board_list_tool_is_independently_locatable() {
        let tools = KanbanMcp::board_list_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "board_list");
    }
}
