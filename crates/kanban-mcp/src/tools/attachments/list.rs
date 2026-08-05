use kanban_contract::ListAttachmentsResponse;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct AttachmentListArgs {
    board: Option<String>,
    task_ref: String,
}

#[tool_router(router = attachment_list_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "attachment_list",
        description = "List task attachment metadata through the canonical host"
    )]
    async fn attachment_list(
        &self,
        Parameters(args): Parameters<AttachmentListArgs>,
    ) -> Result<Json<ListAttachmentsResponse>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let client = self.client.clone();
        let attachments = call_client(move || {
            let task_id = client.resolve_task_id(&board, &task_ref)?;
            client.list_attachments(&task_id)
        })
        .await?;
        Ok(Json(ListAttachmentsResponse { data: attachments }))
    }
}
