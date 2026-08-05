use kanban_protocol::{CreateBoardRequest, CreateBoardResponse};
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
    /// 看板 slug，只能使用小写字母、数字、`-`、`_` 或 `.`。
    slug: String,
    /// 看板名称。
    name: String,
    /// 可选的看板描述。
    description: Option<String>,
}

#[tool_router(router = board_create_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "board_create",
        description = "通过 canonical kanban host 创建看板"
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
