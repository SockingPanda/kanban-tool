use crate::shared::KanbanMcp;
use rmcp::handler::server::router::tool::ToolRouter;

mod create;
mod done;
mod list;
mod remove;
mod reopen;
mod skip;
mod update;

impl KanbanMcp {
    pub(crate) fn step_tools() -> ToolRouter<Self> {
        Self::step_create_tools()
            + Self::step_list_tools()
            + Self::step_update_tools()
            + Self::step_done_tools()
            + Self::step_remove_tools()
            + Self::step_reopen_tools()
            + Self::step_skip_tools()
    }
}
