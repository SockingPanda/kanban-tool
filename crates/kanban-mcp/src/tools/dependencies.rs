use crate::shared::KanbanMcp;
use rmcp::handler::server::router::tool::ToolRouter;

mod create;
mod list;
mod remove;

impl KanbanMcp {
    pub(crate) fn dependency_tools() -> ToolRouter<Self> {
        Self::dependency_create_tools()
            + Self::dependency_remove_tools()
            + Self::dependency_list_tools()
    }
}
