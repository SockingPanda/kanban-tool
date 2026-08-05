use crate::shared::KanbanMcp;
use rmcp::handler::server::router::tool::ToolRouter;

mod create;
mod list;

impl KanbanMcp {
    pub(crate) fn comment_tools() -> ToolRouter<Self> {
        Self::comment_create_tools() + Self::comment_list_tools()
    }
}
