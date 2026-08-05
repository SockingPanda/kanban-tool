use crate::shared::KanbanMcp;
use rmcp::handler::server::router::tool::ToolRouter;

mod list;

impl KanbanMcp {
    pub(crate) fn board_tools() -> ToolRouter<Self> {
        Self::board_list_tools()
    }
}
