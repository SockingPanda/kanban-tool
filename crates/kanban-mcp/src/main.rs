#![doc = include_str!("../README.md")]

mod bounded;
mod config;
mod errors;
mod execution;
mod handler;
mod metadata;
mod pagination;
mod policy;
mod prompts;
mod resources;
mod shared;
mod tools;
mod uri;

use rmcp::{ServiceExt, handler::server::router::tool::ToolRouter, transport::stdio};
use shared::KanbanMcp;

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

    fn work_tool_router() -> ToolRouter<Self> {
        Self::board_tools()
            + Self::task_tools()
            + Self::comment_tools()
            + Self::context_tools()
            + Self::attachment_tools()
            + Self::dependency_tools()
            + Self::event_tools()
            + Self::run_tools()
            + Self::search_tools()
            + Self::step_tools()
            + Self::lifecycle_tools()
            + Self::stats_tools()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--help" {
        println!(
            "kanban-mcp [--inspect | --help]\n无参数时运行 MCP 2026-07-28 stdio server。\n--inspect 只打印脱敏配置和目录，不连接 host，不运行 MCP。\nKANBAN_MCP_CONFIG 指定 JSON 配置文件。"
        );
        return Ok(());
    }
    anyhow::ensure!(
        args.is_empty() || (args.len() == 1 && args[0] == "--inspect"),
        "未知参数；使用 --help"
    );
    let server = KanbanMcp::from_env()?;
    if !args.is_empty() {
        let snapshot = serde_json::json!({
            "runtime": server.runtime_info(),
            "tools": server.policy.tools,
            "resources": server.resource_catalog(),
            "resourceTemplates": server.resource_templates(),
            "prompts": server.prompt_catalog()
        });
        serde_json::to_writer_pretty(std::io::stdout().lock(), &snapshot)?;
        println!();
        return Ok(());
    }
    let service = server.serve(stdio()).await?;
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
