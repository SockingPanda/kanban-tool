use std::collections::BTreeMap;

use kanban_protocol::{
    CommentAuthorType, CommentKind, CreateCommentRequest, CreateCommentResponse,
};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct CommentCreateArgs {
    /// task_ref 使用 board-local 值时采用的 board。默认使用 KB_BOARD/default。
    board: Option<String>,
    /// 全局 t_... ID、board#seq、#seq 或数字 board-local 序号。
    task_ref: String,
    body: String,
    kind: Option<CommentKind>,
    author: Option<String>,
    author_type: Option<CommentAuthorType>,
    agent_type: Option<String>,
    metadata: Option<BTreeMap<String, serde_json::Value>>,
    idempotency_key: Option<String>,
}

#[tool_router(router = comment_create_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "comment_create",
        description = "通过 canonical application service 创建 note 或 decision comment"
    )]
    async fn comment_create(
        &self,
        Parameters(args): Parameters<CommentCreateArgs>,
    ) -> Result<Json<CreateCommentResponse>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let client = self.client.clone();
        let request = CreateCommentRequest {
            idempotency_key: args.idempotency_key,
            author: args.author,
            body: args.body,
            kind: args.kind,
            author_type: args.author_type,
            agent_type: args.agent_type,
            metadata: args
                .metadata
                .map(|metadata| serde_json::Value::Object(metadata.into_iter().collect())),
        };
        let comment =
            call_client(move || client.create_comment_by_selector(&board, &task_ref, &request))
                .await?;
        Ok(Json(CreateCommentResponse { data: comment }))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn comment_create_tool_is_independently_locatable() {
        let tools = KanbanMcp::comment_create_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "comment_create");
    }
}
