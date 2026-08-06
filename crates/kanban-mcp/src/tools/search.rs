use kanban_protocol::{
    ApiTaskStatus, SearchStatusResponse, SearchTasksByStatusResponse, SearchTasksQuery,
    SearchTasksResponse,
};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct SearchTasksArgs {
    /// Board slug 或 ID。默认使用 KB_BOARD/default。
    board: Option<String>,
    q: String,
    status: Vec<ApiTaskStatus>,
    label: Vec<String>,
    assignee: Option<String>,
    include_archived: bool,
    #[serde(default = "default_search_limit")]
    limit: usize,
    offset: usize,
}

const fn default_search_limit() -> usize {
    20
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct SearchStatusArgs {
    /// Board slug 或 ID。默认使用 KB_BOARD/default。
    board: Option<String>,
}

#[tool_router(router = search_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "search_tasks",
        description = "通过 canonical kanban application service 搜索任务"
    )]
    async fn search_tasks(
        &self,
        Parameters(args): Parameters<SearchTasksArgs>,
    ) -> Result<Json<SearchTasksResponse>, McpError> {
        let query = SearchTasksQuery {
            board: self.board(args.board),
            q: Some(args.q),
            status: args.status,
            label: args.label,
            include_archived: args.include_archived,
            limit: args.limit,
            offset: args.offset,
            assignee: args.assignee,
        };
        let client = self.client.clone();
        let response = call_client(move || client.search_tasks(&query)).await?;
        Ok(Json(response))
    }

    #[tool(
        name = "search_status",
        description = "查看 canonical task search projection 状态"
    )]
    async fn search_status(
        &self,
        Parameters(args): Parameters<SearchStatusArgs>,
    ) -> Result<Json<SearchStatusResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let response = call_client(move || client.search_status(&board)).await?;
        Ok(Json(response))
    }

    #[tool(
        name = "search_tasks_by_status",
        description = "按 canonical task status 搜索任务及状态窗口"
    )]
    async fn search_tasks_by_status(
        &self,
        Parameters(args): Parameters<SearchTasksArgs>,
    ) -> Result<Json<SearchTasksByStatusResponse>, McpError> {
        let query = SearchTasksQuery {
            board: self.board(args.board),
            q: Some(args.q),
            status: args.status,
            label: args.label,
            include_archived: args.include_archived,
            limit: args.limit,
            offset: args.offset,
            assignee: args.assignee,
        };
        let client = self.client.clone();
        let response = call_client(move || client.search_tasks_by_status(&query)).await?;
        Ok(Json(response))
    }

    #[tool(
        name = "search_index_rebuild",
        description = "重建 canonical task search projection"
    )]
    async fn search_index_rebuild(
        &self,
        Parameters(args): Parameters<SearchStatusArgs>,
    ) -> Result<Json<SearchStatusResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let response = call_client(move || client.rebuild_search_index(&board)).await?;
        Ok(Json(response))
    }

    #[tool(
        name = "search_index_sync",
        description = "同步 canonical task search projection"
    )]
    async fn search_index_sync(
        &self,
        Parameters(args): Parameters<SearchStatusArgs>,
    ) -> Result<Json<SearchStatusResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let response = call_client(move || client.sync_search_index(&board)).await?;
        Ok(Json(response))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn search_tools_are_independently_locatable() {
        let tools = KanbanMcp::search_tools().list_all();
        let names = tools
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "search_index_rebuild",
                "search_index_sync",
                "search_status",
                "search_tasks",
                "search_tasks_by_status",
            ]
        );
    }
}
