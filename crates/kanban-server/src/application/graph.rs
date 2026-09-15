use crate::{
    application::tasks::support::{api_task, api_task_status},
    error::ApiError,
    state::AppState,
};
use kanban_protocol::cli_helpers::{CliGraphQueryBinding, CliGraphQueryOutput, CliGraphQueryRow};
use kanban_protocol::{
    ApiRelation, ApiRelationProvenance, BoardQuery, BoardTaskMap, BoardTaskMapPath,
    BoardTaskMapQuery, BoardTaskMapResponse, DataEnvelope, GraphMaintenance,
    GraphMaintenanceResponse, GraphNeighborsQuery, GraphNeighborsResponse, GraphQueryQuery,
    GraphStatus, GraphStatusResponse, LimitMeta, MetadataEnvelope, TaskGraphEdge, TaskGraphMeta,
    TaskGraphNode, TaskNeighborhood, TaskNeighborhoodPath, TaskNeighborhoodQuery,
    TaskNeighborhoodResponse,
};
use kanban_service::KanbanError;
use kanban_service::dto::{
    BoardTaskMapRecord, RelationRecord, TaskGraphEdgeRecord, TaskGraphMetaRecord,
    TaskGraphNodeRecord, TaskNeighborhoodRecord,
};
use kanban_service::operations::{
    BoardTaskMapOptions, GraphNeighborsOptions, GraphQueryOptions, TaskNeighborhoodOptions,
};
pub(crate) async fn graph_status(
    state: AppState,
    query: BoardQuery,
) -> Result<GraphStatusResponse, ApiError> {
    let status = state.application().graph_status(&query.board).await?;
    Ok(DataEnvelope::new(GraphStatus {
        backend: status.backend,
        enabled: status.enabled,
        message: status.message,
    }))
}
pub(crate) async fn graph_neighbors(
    state: AppState,
    query: GraphNeighborsQuery,
) -> Result<GraphNeighborsResponse, ApiError> {
    let relations = state
        .application()
        .graph_neighbors(GraphNeighborsOptions {
            board: query.board,
            entity_uri: query.entity_uri,
            predicate: query.predicate,
            limit: query.limit,
        })
        .await?
        .into_iter()
        .map(api_relation)
        .collect::<Result<Vec<_>, _>>()?;
    let limit = query.limit;
    Ok(MetadataEnvelope::new(relations, LimitMeta { limit }))
}
pub(crate) async fn graph_query(
    state: AppState,
    query: GraphQueryQuery,
) -> Result<CliGraphQueryOutput, ApiError> {
    let rows = state
        .application()
        .graph_query(GraphQueryOptions {
            board: query.board,
            query: query.query,
            limit: query.limit,
        })
        .await?
        .into_iter()
        .map(|row| CliGraphQueryRow {
            bindings: row
                .bindings
                .into_iter()
                .map(|binding| CliGraphQueryBinding {
                    name: binding.name,
                    value: binding.value,
                })
                .collect(),
        })
        .collect();
    Ok(DataEnvelope::new(rows))
}
pub(crate) async fn graph_rebuild(
    state: AppState,
    query: BoardQuery,
) -> Result<GraphMaintenanceResponse, ApiError> {
    let maintenance = state.application().graph_rebuild(&query.board).await?;
    Ok(DataEnvelope::new(api_graph_maintenance(maintenance)))
}
pub(crate) async fn graph_sync(
    state: AppState,
    query: BoardQuery,
) -> Result<GraphMaintenanceResponse, ApiError> {
    let maintenance = state.application().graph_sync(&query.board).await?;
    Ok(DataEnvelope::new(api_graph_maintenance(maintenance)))
}
pub(crate) async fn task_neighborhood(
    state: AppState,
    TaskNeighborhoodPath { task_id }: TaskNeighborhoodPath,
    query: TaskNeighborhoodQuery,
) -> Result<TaskNeighborhoodResponse, ApiError> {
    let graph = state
        .application()
        .task_neighborhood(
            &task_id,
            TaskNeighborhoodOptions {
                depth: query.depth,
                limit_nodes: query.limit_nodes,
                include_archived_context: query.include_archived_context,
            },
        )
        .await?;
    Ok(DataEnvelope::new(api_task_neighborhood(graph)?))
}
pub(crate) async fn board_task_map(
    state: AppState,
    BoardTaskMapPath { board }: BoardTaskMapPath,
    query: BoardTaskMapQuery,
) -> Result<BoardTaskMapResponse, ApiError> {
    let graph = state
        .application()
        .board_task_map(
            &board,
            BoardTaskMapOptions {
                active_only: query.active_only,
                context_depth: query.context_depth,
                limit_nodes: query.limit_nodes,
                include_done_context: query.include_done_context,
                include_archived_context: query.include_archived_context,
                hide_isolated: query.hide_isolated,
            },
        )
        .await?;
    Ok(DataEnvelope::new(api_board_task_map(graph)?))
}
fn api_relation(relation: RelationRecord) -> Result<ApiRelation, ApiError> {
    let metadata = serde_json::from_str(&relation.metadata_json).map_err(|error| {
        KanbanError::Storage(format!("存储的 relation metadata 不是有效 JSON：{error}"))
    })?;
    Ok(ApiRelation {
        subject_uri: relation.subject_uri,
        predicate: relation.predicate,
        object_uri: relation.object_uri,
        graph_uri: relation.graph_uri,
        provenance: ApiRelationProvenance {
            source_table: relation.source_table,
            source_id: relation.source_id,
            source_event_id: relation.source_event_id,
            authoritative_store: relation.authoritative_store,
        },
        metadata,
        created_at: relation.created_at,
        updated_at: relation.updated_at,
    })
}
fn api_graph_maintenance(
    maintenance: kanban_service::dto::GraphMaintenanceRecord,
) -> GraphMaintenance {
    GraphMaintenance {
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
fn api_task_neighborhood(graph: TaskNeighborhoodRecord) -> Result<TaskNeighborhood, ApiError> {
    Ok(TaskNeighborhood {
        center_task_id: graph.center_task_id,
        nodes: graph
            .nodes
            .into_iter()
            .map(api_graph_node)
            .collect::<Result<Vec<_>, _>>()?,
        edges: graph
            .edges
            .into_iter()
            .map(api_graph_edge)
            .collect::<Result<Vec<_>, _>>()?,
        meta: api_graph_meta(graph.meta)?,
    })
}
fn api_board_task_map(graph: BoardTaskMapRecord) -> Result<BoardTaskMap, ApiError> {
    Ok(BoardTaskMap {
        nodes: graph
            .nodes
            .into_iter()
            .map(api_graph_node)
            .collect::<Result<Vec<_>, _>>()?,
        edges: graph
            .edges
            .into_iter()
            .map(api_graph_edge)
            .collect::<Result<Vec<_>, _>>()?,
        meta: api_graph_meta(graph.meta)?,
    })
}
fn api_graph_node(node: TaskGraphNodeRecord) -> Result<TaskGraphNode, ApiError> {
    Ok(TaskGraphNode {
        task: api_task(node.task)?,
        role: match node.role {
            kanban_service::dto::TaskGraphNodeRole::Center => {
                kanban_protocol::ApiTaskGraphNodeRole::Center
            }
            kanban_service::dto::TaskGraphNodeRole::DependencyParent => {
                kanban_protocol::ApiTaskGraphNodeRole::DependencyParent
            }
            kanban_service::dto::TaskGraphNodeRole::DependencyChild => {
                kanban_protocol::ApiTaskGraphNodeRole::DependencyChild
            }
            kanban_service::dto::TaskGraphNodeRole::StepParent => {
                kanban_protocol::ApiTaskGraphNodeRole::StepParent
            }
            kanban_service::dto::TaskGraphNodeRole::StepChild => {
                kanban_protocol::ApiTaskGraphNodeRole::StepChild
            }
            kanban_service::dto::TaskGraphNodeRole::Active => {
                kanban_protocol::ApiTaskGraphNodeRole::Active
            }
            kanban_service::dto::TaskGraphNodeRole::Context => {
                kanban_protocol::ApiTaskGraphNodeRole::Context
            }
        },
        context_only: node.context_only,
    })
}
fn api_graph_edge(edge: TaskGraphEdgeRecord) -> Result<TaskGraphEdge, ApiError> {
    Ok(TaskGraphEdge {
        id: edge.id,
        source_task_id: edge.source_task_id,
        target_task_id: edge.target_task_id,
        kind: match edge.kind {
            kanban_service::dto::TaskGraphEdgeKind::Dependency => {
                kanban_protocol::ApiTaskGraphEdgeKind::Dependency
            }
            kanban_service::dto::TaskGraphEdgeKind::Step => {
                kanban_protocol::ApiTaskGraphEdgeKind::Step
            }
        },
        required: edge.required,
        blocking: edge.blocking,
    })
}
fn api_graph_meta(meta: TaskGraphMetaRecord) -> Result<TaskGraphMeta, ApiError> {
    Ok(TaskGraphMeta {
        depth: meta.depth,
        context_depth: meta.context_depth,
        generated_at: meta.generated_at,
        node_count: meta.node_count,
        edge_count: meta.edge_count,
        truncated: meta.truncated,
        active_statuses: meta
            .active_statuses
            .into_iter()
            .map(api_task_status)
            .collect(),
        active_only: meta.active_only,
        include_done_context: meta.include_done_context,
        include_archived_context: meta.include_archived_context,
        hide_isolated: meta.hide_isolated,
        limit_nodes: meta.limit_nodes,
    })
}
