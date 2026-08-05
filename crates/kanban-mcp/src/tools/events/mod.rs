use crate::shared::KanbanMcp;
use rmcp::handler::server::router::tool::ToolRouter;

mod list;

impl KanbanMcp {
    pub(crate) fn event_tools() -> ToolRouter<Self> {
        Self::event_list_tools()
    }
}
