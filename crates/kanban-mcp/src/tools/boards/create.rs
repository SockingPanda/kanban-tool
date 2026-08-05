use kanban_contract::{CreateBoardRequest, CreateBoardResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct BoardCreateArgs {
    /// Board slug, using lowercase letters, digits, `-`, `_`, or `.`.
    slug: String,
    /// Human-readable board name.
    name: String,
    /// Optional board description.
    description: Option<String>,
}

#[tool_router(router = board_create_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "board_create",
        description = "Create a board through the canonical kanban host"
    )]
    async fn board_create(
        &self,
        Parameters(args): Parameters<BoardCreateArgs>,
    ) -> Result<Json<CreateBoardResponse>, McpError> {
        let client = self.client.clone();
        let board = call_client(move || {
            client.create_board(CreateBoardRequest {
                slug: args.slug,
                name: args.name,
                description: args.description,
                actor: None,
            })
        })
        .await?;
        Ok(Json(CreateBoardResponse { data: board }))
    }
}
