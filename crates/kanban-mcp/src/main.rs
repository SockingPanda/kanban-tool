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
            + Self::dependency_tools()
            + Self::event_tools()
            + Self::run_tools()
            + Self::step_tools()
            + Self::lifecycle_tools()
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

    #[test]
    fn tool_inventory_is_stable() {
        let names: Vec<_> = KanbanMcp::tool_router()
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();

        assert_eq!(
            names,
            vec![
                "board_archive",
                "board_create",
                "board_list",
                "board_show",
                "comment_create",
                "comment_list",
                "dependency_create",
                "dependency_list",
                "dependency_remove",
                "event_list",
                "run_list",
                "run_log",
                "run_show",
                "step_create",
                "step_done",
                "step_list",
                "step_remove",
                "step_reopen",
                "step_skip",
                "step_update",
                "task_block",
                "task_claim",
                "task_create",
                "task_done",
                "task_heartbeat",
                "task_list",
                "task_plan_not_required",
                "task_promote",
                "task_release",
                "task_review",
                "task_show",
            ]
        );
    }
}
