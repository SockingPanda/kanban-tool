use crate::shared::KanbanMcp;
use rmcp::handler::server::router::tool::ToolRouter;

mod block;
mod claim;
mod done;
mod heartbeat;
mod release;
mod review;

impl KanbanMcp {
    pub(crate) fn lifecycle_tools() -> ToolRouter<Self> {
        Self::task_claim_tools()
            + Self::task_heartbeat_tools()
            + Self::task_release_tools()
            + Self::task_review_tools()
            + Self::task_done_tools()
            + Self::task_block_tools()
    }
}
