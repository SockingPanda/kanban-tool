use kanban_protocol::ListDependenciesResponse;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct DependencyListArgs {
    /// task_ref 使用 board-local 值时采用的 board。默认使用 KB_BOARD/default。
    board: Option<String>,
    /// 全局 t_... ID、board#seq、#seq 或数字 board-local 序号。
    task_ref: String,
}

#[tool_router(router = dependency_list_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "dependency_list",
        description = "通过 canonical kanban application service 列出 direct task dependencies"
    )]
    async fn dependency_list(
        &self,
        Parameters(args): Parameters<DependencyListArgs>,
    ) -> Result<Json<ListDependenciesResponse>, McpError> {
        let board = self.board(args.board);
        let task_ref = args.task_ref;
        let client = self.client.clone();
        let dependencies =
            call_client(move || client.list_dependencies_by_selector(&board, &task_ref)).await?;
        Ok(Json(ListDependenciesResponse { data: dependencies }))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn dependency_list_tool_is_independently_locatable() {
        let tools = KanbanMcp::dependency_list_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "dependency_list");
    }
}
