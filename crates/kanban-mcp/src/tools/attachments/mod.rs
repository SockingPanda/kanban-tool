mod create;
mod download;
mod list;
mod remove;

use crate::shared::KanbanMcp;
use rmcp::handler::server::router::tool::ToolRouter;

impl KanbanMcp {
    pub(crate) fn attachment_tools() -> ToolRouter<Self> {
        Self::attachment_create_tools()
            + Self::attachment_list_tools()
            + Self::attachment_download_tools()
            + Self::attachment_remove_tools()
    }
}
