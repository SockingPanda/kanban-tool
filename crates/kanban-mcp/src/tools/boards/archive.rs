use kanban_contract::{ArchiveBoardRequest, ArchiveBoardResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client_internal};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct BoardArchiveArgs {
    /// Board slug or id. Defaults to KB_BOARD/default.
    board: Option<String>,
}

#[tool_router(router = board_archive_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "board_archive",
        description = "Archive a board through the canonical kanban host"
    )]
    async fn board_archive(
        &self,
        Parameters(args): Parameters<BoardArchiveArgs>,
    ) -> Result<Json<ArchiveBoardResponse>, McpError> {
        let client = self.client.clone();
        let board = self.board(args.board);
        let result = call_client_internal(move || {
            client.archive_board(&board, &ArchiveBoardRequest::default())
        })
        .await?;
        Ok(Json(ArchiveBoardResponse { data: result }))
    }
}
