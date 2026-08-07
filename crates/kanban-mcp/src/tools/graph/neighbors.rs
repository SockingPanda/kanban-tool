use kanban_protocol::{GraphNeighborsQuery, GraphNeighborsResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct GraphNeighborsArgs {
    board: Option<String>,
    entity_uri: String,
    predicate: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

const fn default_limit() -> usize {
    50
}

#[tool_router(router = graph_neighbors_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "graph_neighbors",
        description = "列出实体发出的 canonical relation facts"
    )]
    async fn graph_neighbors(
        &self,
        Parameters(args): Parameters<GraphNeighborsArgs>,
    ) -> Result<Json<GraphNeighborsResponse>, McpError> {
        let query = GraphNeighborsQuery {
            board: self.board(args.board),
            entity_uri: args.entity_uri,
            predicate: args.predicate,
            limit: args.limit,
        };
        let client = self.client.clone();
        Ok(Json(
            call_client(move || client.graph_neighbors(&query)).await?,
        ))
    }
}
