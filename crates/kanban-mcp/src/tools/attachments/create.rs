use kanban_contract::{CreateAttachmentRequest, CreateAttachmentResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct AttachmentCreateArgs {
    board: Option<String>,
    task_ref: String,
    filename: String,
    #[serde(default)]
    content: Vec<u8>,
    content_type: Option<String>,
    attachment_id: Option<String>,
}

#[tool_router(router = attachment_create_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "attachment_create",
        description = "Create a metadata-safe file-backed task attachment through the host"
    )]
    async fn attachment_create(
        &self,
        Parameters(args): Parameters<AttachmentCreateArgs>,
    ) -> Result<Json<CreateAttachmentResponse>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let client = self.client.clone();
        let attachment = call_client(move || {
            let task_id = client.resolve_task_id(&board, &task_ref)?;
            client.create_attachment(
                &task_id,
                &CreateAttachmentRequest {
                    id: args.attachment_id,
                    filename: args.filename,
                    content: args.content,
                    content_type: args.content_type,
                    rel_path: None,
                    sha256: None,
                    actor: None,
                },
            )
        })
        .await?;
        Ok(Json(CreateAttachmentResponse { data: attachment }))
    }
}
