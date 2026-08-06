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
    /// child_task_ref 或 parent_task_ref 使用 board-local 值时采用的 board。默认使用 KB_BOARD/default。
    board: Option<String>,
    /// child task 的全局 t_... ID、board#seq、#seq 或数字 board-local 序号。
    child_task_ref: String,
    /// parent task 的全局 t_... ID、board#seq、#seq 或数字 board-local 序号。
    parent_task_ref: String,
}

#[tool_router(router = dependency_remove_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "dependency_remove",
        description = "通过 canonical kanban application service 删除 direct parent dependency"
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
