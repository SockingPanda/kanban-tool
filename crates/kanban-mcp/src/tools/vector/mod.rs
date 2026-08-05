mod query_chunks;
mod query_label_atoms;
mod status;

use crate::shared::KanbanMcp;
use rmcp::handler::server::router::tool::ToolRouter;

impl KanbanMcp {
    pub(crate) fn vector_tools() -> ToolRouter<Self> {
        Self::vector_status_tools()
            + Self::vector_query_chunks_tools()
            + Self::vector_query_label_atoms_tools()
    }
}
