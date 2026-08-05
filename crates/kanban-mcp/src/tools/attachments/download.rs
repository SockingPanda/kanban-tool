use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::{Deserialize, Serialize};

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct AttachmentDownloadArgs {
    board: Option<String>,
    task_ref: String,
    attachment_id: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct AttachmentDownloadOutput {
    content_type: Option<String>,
    attachment_id: Option<String>,
    sha256: Option<String>,
    content: Vec<u8>,
}

#[tool_router(router = attachment_download_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "attachment_download",
        description = "Download file-backed attachment bytes through the canonical host"
    )]
    async fn attachment_download(
        &self,
        Parameters(args): Parameters<AttachmentDownloadArgs>,
    ) -> Result<Json<AttachmentDownloadOutput>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let attachment_id = args.attachment_id;
        let client = self.client.clone();
        let attachment = call_client(move || {
            let task_id = client.resolve_task_id(&board, &task_ref)?;
            client.download_attachment(&task_id, &attachment_id)
        })
        .await?;
        Ok(Json(AttachmentDownloadOutput {
            content_type: attachment.content_type,
            attachment_id: attachment.attachment_id,
            sha256: attachment.sha256,
            content: attachment.content,
        }))
    }
}
