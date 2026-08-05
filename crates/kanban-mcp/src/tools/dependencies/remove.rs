use kanban_protocol::RemoveDependencyResponse;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct DependencyRemoveArgs {
    /// Board used when child_task_ref or parent_task_ref is board-local. Defaults to KB_BOARD/default.
    board: Option<String>,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence for the child task.
    child_task_ref: String,
    /// Global t_... id, board#seq, #seq, or numeric board-local sequence for the parent task.
    parent_task_ref: String,
}

#[tool_router(router = dependency_remove_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "dependency_remove",
        description = "Remove a direct parent dependency through the canonical kanban application service"
    )]
    async fn dependency_remove(
        &self,
        Parameters(args): Parameters<DependencyRemoveArgs>,
    ) -> Result<Json<RemoveDependencyResponse>, McpError> {
        let board = self.board(args.board);
        let child_task_ref = args.child_task_ref;
        let parent_task_ref = args.parent_task_ref;
        let client = self.client.clone();
        let dependencies = call_client(move || {
            client.remove_dependency_by_selector(&board, &child_task_ref, &parent_task_ref)
        })
        .await?;
        Ok(Json(RemoveDependencyResponse { data: dependencies }))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn dependency_remove_tool_is_independently_locatable() {
        let tools = KanbanMcp::dependency_remove_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "dependency_remove");
    }
}
