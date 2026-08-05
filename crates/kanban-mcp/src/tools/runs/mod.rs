use crate::shared::KanbanMcp;
use rmcp::handler::server::router::tool::ToolRouter;

mod list;
mod log;
mod show;

impl KanbanMcp {
    pub(crate) fn run_tools() -> ToolRouter<Self> {
        Self::run_list_tools()
    }
}
