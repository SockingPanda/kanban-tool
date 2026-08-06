use kanban_protocol::{DataEnvelope, cli_helpers::CliGraphMaintenance};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct GraphMaintenanceArgs {
    /// Board slug 或 ID。默认使用 KB_BOARD/default。
    board: Option<String>,
}

#[tool_router(router = graph_maintenance_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "graph_rebuild",
        description = "重建 canonical graph derived projection"
    )]
    async fn graph_rebuild(
        &self,
        Parameters(args): Parameters<GraphMaintenanceArgs>,
    ) -> Result<Json<DataEnvelope<CliGraphMaintenance>>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let result = call_client(move || client.graph_rebuild(&board)).await?;
        Ok(Json(DataEnvelope::new(result)))
    }

    #[tool(
        name = "graph_sync",
        description = "同步 canonical graph derived projection"
    )]
    async fn graph_sync(
        &self,
        Parameters(args): Parameters<GraphMaintenanceArgs>,
    ) -> Result<Json<DataEnvelope<CliGraphMaintenance>>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let result = call_client(move || client.graph_sync(&board)).await?;
        Ok(Json(DataEnvelope::new(result)))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn graph_maintenance_tools_are_independently_locatable() {
        let names = KanbanMcp::graph_maintenance_tools()
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["graph_rebuild", "graph_sync"]);
    }
}
