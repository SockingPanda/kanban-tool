use kanban_protocol::{DeleteAttachmentResponse, DeleteResult};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct AttachmentRemoveArgs {
    board: Option<String>,
    task_ref: String,
    attachment_id: String,
}

#[tool_router(router = attachment_remove_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "attachment_remove",
        description = "Remove attachment metadata and retain a recoverable host-local trash copy"
    )]
    async fn attachment_remove(
        &self,
        Parameters(args): Parameters<AttachmentRemoveArgs>,
    ) -> Result<Json<DeleteAttachmentResponse>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let attachment_id = args.attachment_id;
        let client = self.client.clone();
        let deleted = call_client(move || {
            let task_id = client.resolve_task_id(&board, &task_ref)?;
            client.delete_attachment(&task_id, &attachment_id)
        })
        .await?;
        Ok(Json(DeleteAttachmentResponse {
            data: DeleteResult { deleted },
        }))
    }
}
