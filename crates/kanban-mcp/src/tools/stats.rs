use kanban_protocol::{DataEnvelope, StatsResponse};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct StatsArgs {
    /// Board slug 或 ID。默认使用 KB_BOARD/default。
    board: Option<String>,
}

#[tool_router(router = stats_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(name = "stats", description = "查看 canonical board task queue 统计")]
    async fn stats(
        &self,
        Parameters(args): Parameters<StatsArgs>,
    ) -> Result<Json<StatsResponse>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let stats = call_client(move || client.stats(&board)).await?;
        Ok(Json(DataEnvelope::new(stats)))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn stats_tool_is_independently_locatable() {
        let tools = KanbanMcp::stats_tools().list_all();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "stats");
    }
}
