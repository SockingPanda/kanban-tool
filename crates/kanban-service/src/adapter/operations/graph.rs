use crate::dto::{
    BoardTaskMapRecord, GraphMaintenanceRecord, GraphQueryRowRecord, GraphStatusRecord,
    ProjectionStateRecord, RelationRecord, TaskGraphEdgeKind, TaskGraphEdgeRecord,
    TaskGraphMetaRecord, TaskGraphNodeRecord, TaskGraphNodeRole, TaskNeighborhoodRecord,
};
use crate::operations::{
    BoardTaskMapOptions, GraphNeighborsOptions, GraphQuery, GraphQueryOptions,
    ProjectionStatusOptions, TaskNeighborhoodOptions,
};
use crate::{
    StoreBoardTaskMapOptions, StoreGraphNeighborsOptions, StoreGraphQueryOptions,
    StoreProjectionStatusOptions, StoreTaskNeighborhoodOptions,
};
use kanban_core::{KanbanError, Result, TaskStatus};

use super::relations::application_relation_record;
use crate::adapter::{TursoApplicationStore, application_task, store_error};

impl GraphQuery for TursoApplicationStore {
    async fn graph_neighbors(&self, options: GraphNeighborsOptions) -> Result<Vec<RelationRecord>> {
        self.store
            .graph_neighbors(StoreGraphNeighborsOptions {
                board: options.board,
                entity_uri: options.entity_uri,
                predicate: options.predicate,
                limit: options.limit,
            })
            .await
            .map_err(store_error)
            .map(|relations| {
                relations
                    .into_iter()
                    .map(application_relation_record)
                    .collect()
            })
    }

    async fn graph_status(&self, board: &str) -> Result<GraphStatusRecord> {
        self.store
            .graph_status(board)
            .await
            .map_err(store_error)
            .and_then(application_graph_status)
    }

    async fn graph_rebuild(&self, board: &str) -> Result<GraphMaintenanceRecord> {
        self.store
            .graph_rebuild(board)
            .await
            .map_err(store_error)
            .map(application_graph_maintenance)
    }

    async fn graph_sync(&self, board: &str) -> Result<GraphMaintenanceRecord> {
        self.store
            .graph_sync(board)
            .await
            .map_err(store_error)
            .map(application_graph_maintenance)
    }

    async fn projection_status(
        &self,
        options: ProjectionStatusOptions,
    ) -> Result<ProjectionStateRecord> {
        self.store
            .projection_status(StoreProjectionStatusOptions {
                board: options.board,
                projection: options.projection,
            })
            .await
            .map_err(store_error)
            .map(application_projection_state)
    }

    async fn graph_query(&self, options: GraphQueryOptions) -> Result<Vec<GraphQueryRowRecord>> {
        self.store
            .graph_query(StoreGraphQueryOptions {
                board: options.board,
                query: options.query,
                limit: options.limit,
            })
            .await
            .map_err(store_error)
            .map(|rows| {
                rows.into_iter()
                    .map(|row| GraphQueryRowRecord {
                        bindings: row
                            .bindings
                            .into_iter()
                            .map(|binding| crate::GraphQueryBindingRecord {
                                name: binding.name,
                                value: binding.value,
                            })
                            .collect(),
                    })
                    .collect()
            })
    }

    async fn task_neighborhood(
        &self,
        task_id: &str,
        options: TaskNeighborhoodOptions,
    ) -> Result<TaskNeighborhoodRecord> {
        self.store
            .task_neighborhood(
                task_id,
                StoreTaskNeighborhoodOptions {
                    depth: options.depth,
                    limit_nodes: options.limit_nodes,
                    include_archived_context: options.include_archived_context,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_task_neighborhood)
    }

    async fn board_task_map(
        &self,
        board: &str,
        options: BoardTaskMapOptions,
    ) -> Result<BoardTaskMapRecord> {
        self.store
            .board_task_map(
                board,
                StoreBoardTaskMapOptions {
                    active_only: options.active_only,
                    context_depth: options.context_depth,
                    limit_nodes: options.limit_nodes,
                    include_done_context: options.include_done_context,
                    include_archived_context: options.include_archived_context,
                    hide_isolated: options.hide_isolated,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_board_task_map)
    }
}

fn application_projection_state(
    state: crate::domain::ProjectionStateRecord,
) -> ProjectionStateRecord {
    ProjectionStateRecord {
        projection: state.projection,
        lifecycle_status: state.lifecycle_status,
        active_generation: state.active_generation,
        active_fingerprint: state.active_fingerprint,
        last_event_id: state.last_event_id,
        dirty: state.dirty,
        last_success_at: state.last_success_at,
        last_error: state.last_error,
        updated_at: state.updated_at,
        pending_jobs: state.pending_jobs,
        running_jobs: state.running_jobs,
        failed_jobs: state.failed_jobs,
    }
}

fn application_graph_status(status: crate::domain::GraphStatusRecord) -> Result<GraphStatusRecord> {
    Ok(GraphStatusRecord {
        backend: status.backend,
        enabled: status.enabled,
        message: status.message,
        projection: application_projection_state(status.projection),
    })
}

fn application_graph_maintenance(
    maintenance: crate::domain::GraphMaintenanceRecord,
) -> GraphMaintenanceRecord {
    GraphMaintenanceRecord {
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

fn application_task_neighborhood(
    graph: crate::domain::TaskNeighborhoodRecord,
) -> Result<TaskNeighborhoodRecord> {
    Ok(TaskNeighborhoodRecord {
        center_task_id: graph.center_task_id,
        nodes: graph
            .nodes
            .into_iter()
            .map(application_graph_node)
            .collect::<Result<Vec<_>>>()?,
        edges: graph
            .edges
            .into_iter()
            .map(application_graph_edge)
            .collect::<Result<Vec<_>>>()?,
        meta: application_graph_meta(graph.meta)?,
    })
}

fn application_board_task_map(
    graph: crate::domain::BoardTaskMapRecord,
) -> Result<BoardTaskMapRecord> {
    Ok(BoardTaskMapRecord {
        nodes: graph
            .nodes
            .into_iter()
            .map(application_graph_node)
            .collect::<Result<Vec<_>>>()?,
        edges: graph
            .edges
            .into_iter()
            .map(application_graph_edge)
            .collect::<Result<Vec<_>>>()?,
        meta: application_graph_meta(graph.meta)?,
    })
}

fn application_graph_node(node: crate::domain::TaskGraphNodeRecord) -> Result<TaskGraphNodeRecord> {
    Ok(TaskGraphNodeRecord {
        task: application_task(node.task)?,
        role: graph_node_role(&node.role)?,
        context_only: node.context_only,
    })
}

fn application_graph_edge(edge: crate::domain::TaskGraphEdgeRecord) -> Result<TaskGraphEdgeRecord> {
    Ok(TaskGraphEdgeRecord {
        id: edge.id,
        source_task_id: edge.source_task_id,
        target_task_id: edge.target_task_id,
        kind: graph_edge_kind(&edge.kind)?,
        required: edge.required,
        blocking: edge.blocking,
    })
}

fn application_graph_meta(meta: crate::domain::TaskGraphMetaRecord) -> Result<TaskGraphMetaRecord> {
    Ok(TaskGraphMetaRecord {
        depth: meta.depth,
        context_depth: meta.context_depth,
        generated_at: meta.generated_at,
        node_count: meta.node_count,
        edge_count: meta.edge_count,
        truncated: meta.truncated,
        active_statuses: meta
            .active_statuses
            .into_iter()
            .map(|status| {
                status
                    .parse::<TaskStatus>()
                    .map_err(|error| KanbanError::Storage(error.to_string()))
            })
            .collect::<Result<Vec<_>>>()?,
        active_only: meta.active_only,
        include_done_context: meta.include_done_context,
        include_archived_context: meta.include_archived_context,
        hide_isolated: meta.hide_isolated,
        limit_nodes: meta.limit_nodes,
    })
}

fn graph_node_role(value: &str) -> Result<TaskGraphNodeRole> {
    match value {
        "center" => Ok(TaskGraphNodeRole::Center),
        "dependency_parent" => Ok(TaskGraphNodeRole::DependencyParent),
        "dependency_child" => Ok(TaskGraphNodeRole::DependencyChild),
        "step_parent" => Ok(TaskGraphNodeRole::StepParent),
        "step_child" => Ok(TaskGraphNodeRole::StepChild),
        "active" => Ok(TaskGraphNodeRole::Active),
        "context" => Ok(TaskGraphNodeRole::Context),
        other => Err(KanbanError::Storage(format!(
            "stored graph node role is invalid: {other}"
        ))),
    }
}

fn graph_edge_kind(value: &str) -> Result<TaskGraphEdgeKind> {
    match value {
        "dependency" => Ok(TaskGraphEdgeKind::Dependency),
        "step" => Ok(TaskGraphEdgeKind::Step),
        other => Err(KanbanError::Storage(format!(
            "stored graph edge kind is invalid: {other}"
        ))),
    }
}
