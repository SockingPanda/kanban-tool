use crate::shared::KanbanMcp;
use rmcp::handler::server::router::tool::ToolRouter;

mod archive;
mod block;
mod claim;
mod done;
mod heartbeat;
mod reclaim;
mod release;
mod reopen;
mod review;
mod specify;
mod unblock;

impl KanbanMcp {
    pub(crate) fn lifecycle_tools() -> ToolRouter<Self> {
        Self::task_claim_tools()
            + Self::task_heartbeat_tools()
            + Self::task_release_tools()
            + Self::task_review_tools()
            + Self::task_done_tools()
            + Self::task_block_tools()
            + Self::task_specify_tools()
            + Self::task_unblock_tools()
            + Self::task_reopen_tools()
            + Self::task_reclaim_tools()
            + Self::task_archive_tools()
    }
}
