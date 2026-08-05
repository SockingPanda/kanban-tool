use std::future::Future;

use kanban_core::{Clock, KanbanError, Result};

use crate::{
    ApplicationService, ApplicationStore, BoardTaskMapRecord, GraphMaintenanceRecord,
    GraphQueryRowRecord, GraphStatusRecord, ProjectionStateRecord, RelationRecord,
    TaskNeighborhoodRecord,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphNeighborsOptions {
    pub board: String,
    pub entity_uri: String,
    pub predicate: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionStatusOptions {
    pub board: Option<String>,
    pub projection: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphQueryOptions {
    pub board: String,
    pub query: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskNeighborhoodOptions {
    pub depth: usize,
    pub limit_nodes: usize,
    pub include_archived_context: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardTaskMapOptions {
    pub active_only: bool,
    pub context_depth: usize,
    pub limit_nodes: usize,
    pub include_done_context: bool,
    pub include_archived_context: bool,
    pub hide_isolated: bool,
}

pub trait GraphQuery: ApplicationStore {
    fn graph_neighbors(
        &self,
        options: GraphNeighborsOptions,
    ) -> impl Future<Output = Result<Vec<RelationRecord>>> + Send;
    fn graph_status(&self, board: &str) -> impl Future<Output = Result<GraphStatusRecord>> + Send;
    fn graph_rebuild(
        &self,
        board: &str,
    ) -> impl Future<Output = Result<GraphMaintenanceRecord>> + Send;
    fn graph_sync(
        &self,
        board: &str,
    ) -> impl Future<Output = Result<GraphMaintenanceRecord>> + Send;
    fn projection_status(
        &self,
        options: ProjectionStatusOptions,
    ) -> impl Future<Output = Result<ProjectionStateRecord>> + Send;
    fn graph_query(
        &self,
        options: GraphQueryOptions,
    ) -> impl Future<Output = Result<Vec<GraphQueryRowRecord>>> + Send;
    fn task_neighborhood(
        &self,
        task_id: &str,
        options: TaskNeighborhoodOptions,
    ) -> impl Future<Output = Result<TaskNeighborhoodRecord>> + Send;
    fn board_task_map(
        &self,
        board: &str,
        options: BoardTaskMapOptions,
    ) -> impl Future<Output = Result<BoardTaskMapRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: GraphQuery,
    C: Clock,
{
    pub async fn graph_neighbors(
        &self,
        options: GraphNeighborsOptions,
    ) -> Result<Vec<RelationRecord>> {
        if options.board.trim().is_empty() || options.entity_uri.trim().is_empty() {
            return Err(KanbanError::InvalidInput(
                "graph board and entity_uri are required".to_owned(),
            ));
        }
        if options.limit == 0 || options.limit > 1_000 {
            return Err(KanbanError::InvalidInput(
                "graph limit must be between 1 and 1000".to_owned(),
            ));
        }
        self.store.graph_neighbors(options).await
    }

    pub async fn graph_status(&self, board: &str) -> Result<GraphStatusRecord> {
        if board.trim().is_empty() {
            return Err(KanbanError::InvalidInput("board is required".to_owned()));
        }
        self.store.graph_status(board.trim()).await
    }

    pub async fn graph_rebuild(&self, board: &str) -> Result<GraphMaintenanceRecord> {
        if board.trim().is_empty() {
            return Err(KanbanError::InvalidInput("board is required".to_owned()));
        }
        self.store.graph_rebuild(board.trim()).await
    }

    pub async fn graph_sync(&self, board: &str) -> Result<GraphMaintenanceRecord> {
        if board.trim().is_empty() {
            return Err(KanbanError::InvalidInput("board is required".to_owned()));
        }
        self.store.graph_sync(board.trim()).await
    }

    pub async fn projection_status(
        &self,
        options: ProjectionStatusOptions,
    ) -> Result<ProjectionStateRecord> {
        self.store.projection_status(options).await
    }

    pub async fn graph_query(
        &self,
        options: GraphQueryOptions,
    ) -> Result<Vec<GraphQueryRowRecord>> {
        if options.board.trim().is_empty() || options.query.trim().is_empty() {
            return Err(KanbanError::InvalidInput(
                "graph board and query are required".to_owned(),
            ));
        }
        if options.limit == 0 || options.limit > 1_000 {
            return Err(KanbanError::InvalidInput(
                "graph limit must be between 1 and 1000".to_owned(),
            ));
        }
        self.store.graph_query(options).await
    }

    pub async fn task_neighborhood(
        &self,
        task_id: &str,
        options: TaskNeighborhoodOptions,
    ) -> Result<TaskNeighborhoodRecord> {
        if !task_id.trim().starts_with("t_") || task_id.trim().len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task id must start with t_".to_owned(),
            ));
        }
        self.store.task_neighborhood(task_id.trim(), options).await
    }

    pub async fn board_task_map(
        &self,
        board: &str,
        options: BoardTaskMapOptions,
    ) -> Result<BoardTaskMapRecord> {
        if board.trim().is_empty() {
            return Err(KanbanError::InvalidInput("board is required".to_owned()));
        }
        self.store.board_task_map(board.trim(), options).await
    }
}
