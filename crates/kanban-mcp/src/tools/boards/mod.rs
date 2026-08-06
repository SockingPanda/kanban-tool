use crate::shared::KanbanMcp;
use rmcp::handler::server::router::tool::ToolRouter;

mod archive;
mod columns;
mod create;
mod list;
mod show;

impl KanbanMcp {
    pub(crate) fn board_tools() -> ToolRouter<Self> {
        Self::board_archive_tools()
            + Self::board_columns_tools()
            + Self::board_create_tools()
            + Self::board_list_tools()
            + Self::board_show_tools()
    }
}
