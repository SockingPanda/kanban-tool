use crate::shared::KanbanMcp;
use rmcp::handler::server::router::tool::ToolRouter;

mod create;
mod list;
mod plan_not_required;
mod promote;
mod show;

impl KanbanMcp {
    pub(crate) fn task_tools() -> ToolRouter<Self> {
        Self::task_create_tools()
            + Self::task_list_tools()
            + Self::task_show_tools()
            + Self::task_plan_not_required_tools()
            + Self::task_promote_tools()
    }
}
