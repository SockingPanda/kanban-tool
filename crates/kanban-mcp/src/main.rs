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
        McpOperationClass, operation_catalog, project_mcp_policy, validate_mcp_policy_projection,
    };

    #[test]
    fn tool_inventory_is_stable() {
        let names: Vec<_> = KanbanMcp::tool_router()
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();

        let projection = project_mcp_policy(operation_catalog()).expect("MCP policy projection");
        let mut catalog_names: Vec<_> = projection
            .tool_bindings()
            .iter()
            .map(|binding| binding.tool_name)
            .collect();
        catalog_names.sort_unstable();
        assert_eq!(names, catalog_names);
        validate_mcp_policy_projection(&projection)
            .expect("MCP projection 只能绑定已存在的领域 endpoint");
        assert_eq!(catalog_names.len(), 104);
    }

    #[test]
    fn host_admin_operations_are_not_exposed_by_catalog_or_router() {
        let projection = project_mcp_policy(operation_catalog()).expect("MCP policy projection");
        let bound_operations = projection
            .tool_bindings()
            .iter()
            .filter(|binding| binding.class == McpOperationClass::Domain)
            .flat_map(|binding| binding.http_operations.iter().copied())
            .collect::<std::collections::BTreeSet<_>>();
        for operation_id in projection.operations(McpOperationClass::HostAdmin) {
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
