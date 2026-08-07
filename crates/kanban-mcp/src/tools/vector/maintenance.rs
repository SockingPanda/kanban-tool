use kanban_protocol::{DataEnvelope, VectorConfigureRequest, VectorStatus};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::{Json, Parameters},
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::shared::{KanbanMcp, call_client};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct VectorMaintenanceArgs {
    /// Board slug 或 ID。默认使用 KB_BOARD/default。
    board: Option<String>,
}

#[tool_router(router = vector_maintenance_tools, vis = "pub(crate)")]
impl KanbanMcp {
    #[tool(
        name = "vector_configure",
        description = "配置 canonical vector provider 与 embedding model"
    )]
    async fn vector_configure(
        &self,
        Parameters(args): Parameters<VectorConfigureRequest>,
    ) -> Result<Json<VectorConfigureRequest>, McpError> {
        let client = self.client.clone();
        let config = call_client(move || client.configure_vector(args)).await?;
        Ok(Json(config))
    }

    #[tool(
        name = "vector_rebuild",
        description = "重建 canonical vector derived projection"
    )]
    async fn vector_rebuild(
        &self,
        Parameters(args): Parameters<VectorMaintenanceArgs>,
    ) -> Result<Json<DataEnvelope<VectorStatus>>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let status = call_client(move || client.rebuild_vector(&board)).await?;
        Ok(Json(DataEnvelope::new(status)))
    }

    #[tool(
        name = "vector_sync",
        description = "同步 canonical vector derived projection"
    )]
    async fn vector_sync(
        &self,
        Parameters(args): Parameters<VectorMaintenanceArgs>,
    ) -> Result<Json<DataEnvelope<VectorStatus>>, McpError> {
        let board = self.board(args.board);
        let client = self.client.clone();
        let status = call_client(move || client.sync_vector(&board)).await?;
        Ok(Json(DataEnvelope::new(status)))
    }
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn vector_maintenance_tools_are_independently_locatable() {
        let names = KanbanMcp::vector_maintenance_tools()
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec!["vector_configure", "vector_rebuild", "vector_sync"]
        );
    }
}
