use kanban_contract::{ApiTaskStatus, SearchStatusResponse, SearchTasksQuery, SearchTasksResponse};
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
    /// Board slug or id. Defaults to KB_BOARD/default.
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
    /// Board slug or id. Defaults to KB_BOARD/default.
    board: Option<String>,
}

#[tool_router(router = search_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "search_tasks",
        description = "Search tasks through the canonical kanban application service"
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
        description = "Inspect canonical task search projection status"
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
        assert_eq!(names, vec!["search_status", "search_tasks"]);
    }
}
