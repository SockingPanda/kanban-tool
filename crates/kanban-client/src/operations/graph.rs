use kanban_protocol::cli_helpers::{CliGraphMaintenance, CliGraphQueryOutput};
use kanban_protocol::{
    BoardTaskMap, BoardTaskMapQuery, BoardTaskMapResponse, GraphMaintenanceResponse,
    GraphNeighborsQuery, GraphNeighborsResponse, GraphStatus, GraphStatusResponse,
    TaskNeighborhood, TaskNeighborhoodQuery, TaskNeighborhoodResponse,
};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn graph_status(&self, board: &str) -> Result<GraphStatus, ClientError> {
        let path = format!("/api/v1/graph/status?board={}", encode_path_segment(board));
        let response: GraphStatusResponse = self.get(&path)?;
        Ok(response.data)
    }

    pub fn graph_rebuild(&self, board: &str) -> Result<CliGraphMaintenance, ClientError> {
        let response: GraphMaintenanceResponse = self.post(
            &format!("/api/v1/graph/rebuild?board={}", encode_path_segment(board)),
            &serde_json::json!({}),
        )?;
        Ok(cli_graph_maintenance(response.data))
    }

    pub fn graph_sync(&self, board: &str) -> Result<CliGraphMaintenance, ClientError> {
        let response: GraphMaintenanceResponse = self.post(
            &format!("/api/v1/graph/sync?board={}", encode_path_segment(board)),
            &serde_json::json!({}),
        )?;
        Ok(cli_graph_maintenance(response.data))
    }

    pub fn graph_neighbors(
        &self,
        query: &GraphNeighborsQuery,
    ) -> Result<GraphNeighborsResponse, ClientError> {
        let mut path = format!(
            "/api/v1/graph/neighbors?board={}&entity_uri={}&limit={}",
            encode_path_segment(&query.board),
            encode_path_segment(&query.entity_uri),
            query.limit
        );
        if let Some(predicate) = query.predicate.as_deref() {
            path.push_str("&predicate=");
            path.push_str(&encode_path_segment(predicate));
        }
        self.get(&path)
    }

    pub fn graph_query(
        &self,
        board: &str,
        query: &str,
        limit: usize,
    ) -> Result<CliGraphQueryOutput, ClientError> {
        let path = format!(
            "/api/v1/graph/query?board={}&query={}&limit={}",
            encode_path_segment(board),
            encode_path_segment(query),
            limit
        );
        self.get(&path)
    }

    pub fn task_neighborhood(
        &self,
        task_id: &str,
        query: &TaskNeighborhoodQuery,
    ) -> Result<TaskNeighborhood, ClientError> {
        let path = format!(
            "/api/v1/tasks/{}/neighborhood?depth={}&limit_nodes={}&include_archived_context={}",
            encode_path_segment(task_id),
            query.depth,
            query.limit_nodes,
            query.include_archived_context
        );
        let response: TaskNeighborhoodResponse = self.get(&path)?;
        Ok(response.data)
    }

    pub fn board_task_map(
        &self,
        board: &str,
        query: &BoardTaskMapQuery,
    ) -> Result<BoardTaskMap, ClientError> {
        let path = format!(
            "/api/v1/boards/{}/task-map?active_only={}&context_depth={}&limit_nodes={}&include_done_context={}&include_archived_context={}&hide_isolated={}",
            encode_path_segment(board),
            query.active_only,
            query.context_depth,
            query.limit_nodes,
            query.include_done_context,
            query.include_archived_context,
            query.hide_isolated
        );
        let response: BoardTaskMapResponse = self.get(&path)?;
        Ok(response.data)
    }
}

fn cli_graph_maintenance(maintenance: kanban_protocol::GraphMaintenance) -> CliGraphMaintenance {
    CliGraphMaintenance {
        mode: maintenance.mode,
        board_id: maintenance.board_id,
        generation: maintenance.generation,
        fingerprint: maintenance.fingerprint,
        validated_tasks: maintenance.validated_tasks,
        validated_entities: maintenance.validated_entities,
        validated_relations: maintenance.validated_relations,
        pending_jobs: maintenance.pending_jobs,
        consumed_jobs: maintenance.consumed_jobs,
        updated_at: maintenance.updated_at,
        message: maintenance.message,
    }
}
