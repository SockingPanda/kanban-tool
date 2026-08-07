mod maintenance;
mod map;
mod neighborhood;
mod neighbors;
mod query;
mod status;

use rmcp::handler::server::router::tool::ToolRouter;

use crate::shared::KanbanMcp;

impl KanbanMcp {
    pub(crate) fn graph_tools() -> ToolRouter<Self> {
        Self::graph_map_tools()
            + Self::graph_maintenance_tools()
            + Self::graph_neighbors_tools()
            + Self::graph_neighborhood_tools()
            + Self::graph_query_tools()
            + Self::graph_status_tools()
    }
}
