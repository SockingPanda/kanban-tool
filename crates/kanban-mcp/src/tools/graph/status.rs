use kanban_contract::{DataEnvelope, GraphStatus, GraphStatusResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client_internal};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct GraphStatusArgs {
    board: Option<String>,
}

#[tool_router(router = graph_status_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "graph_status",
        description = "Report canonical graph relation and projection status"
    )]
    async fn graph_status(
        &self,
        Parameters(args): Parameters<GraphStatusArgs>,
    ) -> Result<Json<GraphStatusResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let status = call_client_internal(move || client.graph_status(&board)).await?;
        Ok(Json(DataEnvelope::new(GraphStatus {
            backend: status.backend,
            enabled: status.enabled,
            message: status.message,
        })))
    }
}
