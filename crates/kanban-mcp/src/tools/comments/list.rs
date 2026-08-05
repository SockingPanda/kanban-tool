use kanban_contract::ListCommentsResponse;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct CommentListArgs {
    /// Board used when task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence.
    task_ref: String,
}

#[tool_router(router = comment_list_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "comment_list",
        description = "List task comments from the canonical kanban application service"
    )]
    async fn comment_list(
        &self,
        Parameters(args): Parameters<CommentListArgs>,
    ) -> Result<Json<ListCommentsResponse>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let client = self.client.clone();
        let comments =
            call_client(move || client.list_comments_by_selector(&board, &task_ref)).await?;
        Ok(Json(ListCommentsResponse { data: comments }))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn comment_list_tool_is_independently_locatable() {
        let tools = KanbanMcp::comment_list_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "comment_list");
    }
}
