use crate::shared::KanbanMcp;
use rmcp::handler::server::router::tool::ToolRouter;

mod create;
mod list;
mod update;

impl KanbanMcp {
    pub(crate) fn step_tools() -> ToolRouter<Self> {
        Self::step_create_tools() + Self::step_list_tools() + Self::step_update_tools()
    }
}
