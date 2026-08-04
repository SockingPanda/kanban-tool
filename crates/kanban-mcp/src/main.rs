use std::{env, sync::Arc};

use kanban_client::{DEFAULT_SERVER_URL, KanbanClient};
use kanban_contract::ListBoardsResponse;
use rmcp::{
    ErrorData as McpError, ServiceExt,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
    transport::stdio,
};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct BoardListArgs {
    /// Include archived boards in the result.
    include_archived: bool,
}

#[derive(Clone)]
struct KanbanMcp {
    client: Arc<KanbanClient>,
}

#[tool_router(server_handler)]
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
        let boards = tokio::task::spawn_blocking(move || client.list_boards(args.include_archived))
            .await
            .map_err(|error| McpError::internal_error(error.to_string(), None))?
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;

        Ok(Json(ListBoardsResponse { data: boards }))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server_url =
        env::var("KANBAN_SERVER_URL").unwrap_or_else(|_| DEFAULT_SERVER_URL.to_owned());
    let actor = env::var("KANBAN_ACTOR").unwrap_or_else(|_| "mcp".to_owned());
    let service = KanbanMcp {
        client: Arc::new(KanbanClient::new(server_url, actor)?),
    }
    .serve(stdio())
    .await?;
    service.waiting().await?;
    Ok(())
}
