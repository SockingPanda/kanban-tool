use kanban_protocol::cli_helpers::{CliGraphMaintenance, CliGraphQueryOutput};
use kanban_protocol::{
    BoardTaskMap, BoardTaskMapQuery, GraphNeighborsQuery, GraphNeighborsResponse, GraphStatus,
    TaskNeighborhood, TaskNeighborhoodQuery,
};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn graph_status(&self, board: &str) -> Result<GraphStatus, ClientError> {
        let response: kanban_protocol::GraphStatusResponse = rpc!(
            self,
            graph_status,
            GraphStatusRequest,
            (),
            kanban_protocol::BoardQuery {
                board: board.trim().to_owned()
            },
            ()
        )?;
        Ok(response.data)
    }

    pub async fn graph_rebuild(&self, board: &str) -> Result<CliGraphMaintenance, ClientError> {
        let response: kanban_protocol::GraphMaintenanceResponse = rpc!(
            self,
            graph_rebuild,
            GraphRebuildRequest,
            (),
            kanban_protocol::BoardQuery {
                board: board.trim().to_owned()
            },
            ()
        )?;
        Ok(cli_graph_maintenance(response.data))
    }

    pub async fn graph_sync(&self, board: &str) -> Result<CliGraphMaintenance, ClientError> {
        let response: kanban_protocol::GraphMaintenanceResponse = rpc!(
            self,
            graph_sync,
            GraphSyncRequest,
            (),
            kanban_protocol::BoardQuery {
                board: board.trim().to_owned()
            },
            ()
        )?;
        Ok(cli_graph_maintenance(response.data))
    }

    pub async fn graph_neighbors(
        &self,
        query: &GraphNeighborsQuery,
    ) -> Result<GraphNeighborsResponse, ClientError> {
        let response: kanban_protocol::GraphNeighborsResponse = rpc!(
            self,
            graph_neighbors,
            GraphNeighborsRequest,
            (),
            query.clone(),
            ()
        )?;
        Ok(response)
    }

    pub async fn graph_query(
        &self,
        board: &str,
        query: &str,
        limit: usize,
    ) -> Result<CliGraphQueryOutput, ClientError> {
        let response: kanban_protocol::cli_helpers::CliGraphQueryOutput = rpc!(
            self,
            graph_query,
            GraphQueryRequest,
            (),
            kanban_protocol::GraphQueryQuery {
                board: board.to_owned(),
                query: query.to_owned(),
                limit
            },
            ()
        )?;
        Ok(response)
    }

    pub async fn task_neighborhood(
        &self,
        task_id: &str,
        query: &TaskNeighborhoodQuery,
    ) -> Result<TaskNeighborhood, ClientError> {
        let response: kanban_protocol::TaskNeighborhoodResponse = rpc!(
            self,
            task_neighborhood,
            TaskNeighborhoodRequest,
            kanban_protocol::TaskNeighborhoodPath {
                task_id: task_id.to_owned()
            },
            query.clone(),
            ()
        )?;
        Ok(response.data)
    }

    pub async fn board_task_map(
        &self,
        board: &str,
        query: &BoardTaskMapQuery,
    ) -> Result<BoardTaskMap, ClientError> {
        let response: kanban_protocol::BoardTaskMapResponse = rpc!(
            self,
            board_task_map,
            BoardTaskMapRequest,
            kanban_protocol::BoardTaskMapPath {
                board: board.to_owned()
            },
            query.clone(),
            ()
        )?;
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
