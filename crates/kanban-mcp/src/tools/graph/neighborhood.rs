use kanban_protocol::{
    DataEnvelope, TaskNeighborhood, TaskNeighborhoodQuery, TaskNeighborhoodResponse,
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
struct GraphNeighborhoodArgs {
    task_id: String,
    #[serde(default = "default_depth")]
    depth: usize,
    #[serde(default = "default_limit_nodes")]
    limit_nodes: usize,
    #[serde(default)]
    include_archived_context: bool,
}

const fn default_depth() -> usize {
    1
}
const fn default_limit_nodes() -> usize {
    250
}

#[tool_router(router = graph_neighborhood_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "task_neighborhood",
        description = "返回支持 cycle-safe traversal 的有界任务邻域"
    )]
    async fn task_neighborhood(
        &self,
        Parameters(args): Parameters<GraphNeighborhoodArgs>,
    ) -> Result<Json<TaskNeighborhoodResponse>, McpError> {
        let query = TaskNeighborhoodQuery {
            depth: args.depth,
            limit_nodes: args.limit_nodes,
            include_archived_context: args.include_archived_context,
        };
        let client = self.client.clone();
        let task_id = args.task_id;
        let value = call_client(move || client.task_neighborhood(&task_id, &query)).await?;
        Ok(Json(DataEnvelope::new(TaskNeighborhood {
            center_task_id: value.center_task_id,
            nodes: value.nodes,
            edges: value.edges,
            meta: value.meta,
        })))
    }
}
