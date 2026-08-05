use kanban_protocol::{BoardTaskMap, BoardTaskMapQuery, BoardTaskMapResponse, DataEnvelope};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct GraphMapArgs {
    board: Option<String>,
    #[serde(default = "default_active_only")]
    active_only: bool,
    #[serde(default = "default_context_depth")]
    context_depth: usize,
    #[serde(default = "default_limit_nodes")]
    limit_nodes: usize,
    #[serde(default = "default_include_done_context")]
    include_done_context: bool,
    #[serde(default)]
    include_archived_context: bool,
    #[serde(default)]
    hide_isolated: bool,
}

const fn default_active_only() -> bool {
    true
}
const fn default_context_depth() -> usize {
    1
}
const fn default_limit_nodes() -> usize {
    250
}
const fn default_include_done_context() -> bool {
    true
}

#[tool_router(router = graph_map_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "board_task_map",
        description = "Return a bounded board task map with context expansion"
    )]
    async fn board_task_map(
        &self,
        Parameters(args): Parameters<GraphMapArgs>,
    ) -> Result<Json<BoardTaskMapResponse>, McpError> {
        let board = self.board(args.board);
        let query = BoardTaskMapQuery {
            active_only: args.active_only,
            context_depth: args.context_depth,
            limit_nodes: args.limit_nodes,
            include_done_context: args.include_done_context,
            include_archived_context: args.include_archived_context,
            hide_isolated: args.hide_isolated,
        };
        let client = self.client.clone();
        let value = call_client(move || client.board_task_map(&board, &query)).await?;
        Ok(Json(DataEnvelope::new(BoardTaskMap {
            nodes: value.nodes,
            edges: value.edges,
            meta: value.meta,
        })))
    }
}
