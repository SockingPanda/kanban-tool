use crate::shared::{KanbanMcp, call_client_internal};
use kanban_protocol::VectorStatusResponse;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Args {
    board: Option<String>,
}

#[tool_router(router = vector_status_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "vector_status",
        description = "Read Turso vector projection status"
    )]
    async fn vector_status(
        &self,
        Parameters(args): Parameters<Args>,
    ) -> Result<Json<VectorStatusResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let status = call_client_internal(move || client.vector_status(&board)).await?;
        Ok(Json(VectorStatusResponse { data: status }))
    }
}
