use kanban_protocol::cli_helpers::CliGraphQueryOutput;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct GraphQueryArgs {
    board: Option<String>,
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

const fn default_limit() -> usize {
    100
}

#[tool_router(router = graph_query_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "graph_query",
        description = "运行有界只读 canonical graph query compatibility surface"
    )]
    async fn graph_query(
        &self,
        Parameters(args): Parameters<GraphQueryArgs>,
    ) -> Result<Json<CliGraphQueryOutput>, McpError> {
        let board = self.board(args.board);
        let query = args.query;
        let limit = args.limit;
        let client = self.client.clone();
        Ok(Json(
            call_client(move || client.graph_query(&board, &query, limit)).await?,
        ))
    }
}
