use kanban_protocol::{BuildContextQuery, BuildContextResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct ContextBuildArgs {
    /// Board slug or id. Defaults to KB_BOARD/default.
    board: Option<String>,
    task: Option<String>,
    reference: Option<String>,
    query: Option<String>,
    #[serde(default = "default_depth")]
    depth: usize,
    #[serde(default = "default_lexical_limit")]
    lexical_limit: usize,
    #[serde(default = "default_graph_limit")]
    graph_limit: usize,
    #[serde(default = "default_vector_limit")]
    vector_limit: usize,
    #[serde(default = "default_budget")]
    budget: usize,
}

const fn default_lexical_limit() -> usize {
    5
}
const fn default_depth() -> usize {
    1
}
const fn default_graph_limit() -> usize {
    10
}
const fn default_vector_limit() -> usize {
    5
}
const fn default_budget() -> usize {
    20
}

#[tool_router(router = context_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "context_build",
        description = "Build a bounded read-only context pack from lexical, graph, and vector providers"
    )]
    async fn context_build(
        &self,
        Parameters(args): Parameters<ContextBuildArgs>,
    ) -> Result<Json<BuildContextResponse>, McpError> {
        if args.task.is_none() && args.reference.is_none() && args.query.is_none() {
            return Err(McpError::invalid_params(
                "one of task, reference or query is required",
                None,
            ));
        }
        let board = self.board(args.board);
        let subject = args
            .task
            .clone()
            .or_else(|| args.reference.clone())
            .unwrap_or_else(|| "query".to_owned());
        let query = BuildContextQuery {
            board,
            lexical_limit: args.lexical_limit,
            graph_limit: args.graph_limit,
            vector_limit: args.vector_limit,
            max_items: args.budget,
            task: args.task,
            reference: args.reference,
            query: args.query,
            depth: args.depth,
            budget: Some(args.budget),
        };
        let client = self.client.clone();
        let response = call_client(move || client.build_context(&subject, &query)).await?;
        Ok(Json(response))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn context_tool_is_readonly_and_locatable() {
        let tools = KanbanMcp::context_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "context_build");
    }
}
