#![doc = include_str!("../README.md")]

mod shared;
mod tools;

use rmcp::{
    ServerHandler, ServiceExt, handler::server::router::tool::ToolRouter, tool_handler,
    transport::stdio,
};
use shared::KanbanMcp;

#[tool_handler]
impl ServerHandler for KanbanMcp {}

impl KanbanMcp {
    fn tool_router() -> ToolRouter<Self> {
        Self::board_tools()
            + Self::task_tools()
            + Self::comment_tools()
            + Self::context_tools()
            + Self::attachment_tools()
            + Self::dependency_tools()
            + Self::entity_tools()
            + Self::event_tools()
            + Self::label_tools()
            + Self::graph_tools()
            + Self::run_tools()
            + Self::search_tools()
            + Self::signal_tools()
            + Self::step_tools()
            + Self::lifecycle_tools()
            + Self::ontology_tools()
            + Self::stats_tools()
            + Self::vector_tools()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let service = KanbanMcp::from_env()?.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;
    use kanban_protocol::{
        MCP_HOST_ADMIN_OPERATION_IDS, mcp_operation_catalog, validate_mcp_operation_catalog,
    };

    #[test]
    fn tool_inventory_is_stable() {
        let names: Vec<_> = KanbanMcp::tool_router()
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();

        let catalog_names: Vec<_> = mcp_operation_catalog()
            .iter()
            .map(|operation| operation.tool_name)
            .collect();
        assert_eq!(names, catalog_names);
        validate_mcp_operation_catalog(mcp_operation_catalog())
            .expect("MCP catalog 只能绑定已存在的领域 endpoint");
        assert_eq!(catalog_names.len(), 103);
    }

    #[test]
    fn host_admin_operations_are_not_exposed_by_catalog_or_router() {
        let bound_operations = mcp_operation_catalog()
            .iter()
            .flat_map(|operation| operation.http_operations)
            .collect::<std::collections::BTreeSet<_>>();
        for operation_id in MCP_HOST_ADMIN_OPERATION_IDS {
            assert!(
                !bound_operations.contains(operation_id),
                "MCP catalog 意外绑定了 host-admin operation：{operation_id}"
            );
        }

        let names: Vec<_> = KanbanMcp::tool_router()
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();
        for forbidden in [
            "backup",
            "checkpoint",
            "doctor",
            "export",
            "import",
            "maintenance",
            "migration",
            "vacuum",
            "database_replace",
        ] {
            assert!(
                names.iter().all(|name| !name.contains(forbidden)),
                "MCP 意外暴露了 host-admin tool：{forbidden}"
            );
        }
    }
}
