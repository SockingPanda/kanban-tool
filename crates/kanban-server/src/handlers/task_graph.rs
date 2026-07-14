use crate::dto::api_task_from_record;
use crate::error::{ApiError, extractor_error};
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
};
use kanban_contract::{
    ApiTaskGraphEdgeKind, ApiTaskGraphNodeRole, BoardTaskMap as ContractBoardTaskMap,
    BoardTaskMapPath, BoardTaskMapQuery, BoardTaskMapResponse, DataEnvelope, TaskGraphEdge,
    TaskGraphMeta, TaskGraphNode, TaskNeighborhood as ContractTaskNeighborhood,
    TaskNeighborhoodPath, TaskNeighborhoodQuery, TaskNeighborhoodResponse,
};
use kanban_sqlite::api::{TaskGraphEdgeKind, TaskGraphNodeRole};
fn graph_role(role: TaskGraphNodeRole) -> ApiTaskGraphNodeRole {
    match role {
        TaskGraphNodeRole::Center => ApiTaskGraphNodeRole::Center,
        TaskGraphNodeRole::DependencyParent => ApiTaskGraphNodeRole::DependencyParent,
        TaskGraphNodeRole::DependencyChild => ApiTaskGraphNodeRole::DependencyChild,
        TaskGraphNodeRole::StepParent => ApiTaskGraphNodeRole::StepParent,
        TaskGraphNodeRole::StepChild => ApiTaskGraphNodeRole::StepChild,
        TaskGraphNodeRole::Active => ApiTaskGraphNodeRole::Active,
        TaskGraphNodeRole::Context => ApiTaskGraphNodeRole::Context,
    }
}
fn edge_kind(kind: TaskGraphEdgeKind) -> ApiTaskGraphEdgeKind {
    match kind {
        TaskGraphEdgeKind::Dependency => ApiTaskGraphEdgeKind::Dependency,
        TaskGraphEdgeKind::Step => ApiTaskGraphEdgeKind::Step,
    }
}
fn meta(meta: kanban_sqlite::api::TaskGraphMeta) -> TaskGraphMeta {
    TaskGraphMeta {
        depth: meta.depth,
        context_depth: meta.context_depth,
        generated_at: meta.generated_at,
        node_count: meta.node_count,
        edge_count: meta.edge_count,
        truncated: meta.truncated,
        active_statuses: meta
            .active_statuses
            .into_iter()
            .map(|s| kanban_contract::ApiTaskStatus::from_str(s.as_str()).expect("status"))
            .collect(),
        active_only: meta.active_only,
        include_done_context: meta.include_done_context,
        include_archived_context: meta.include_archived_context,
        hide_isolated: meta.hide_isolated,
        limit_nodes: meta.limit_nodes,
    }
}
fn node(node: kanban_sqlite::api::TaskGraphNodeRecord) -> Result<TaskGraphNode, ApiError> {
    Ok(TaskGraphNode {
        task: api_task_from_record(node.task)?,
        role: graph_role(node.role),
        context_only: node.context_only,
    })
}
fn edge(edge: kanban_sqlite::api::TaskGraphEdgeRecord) -> TaskGraphEdge {
    TaskGraphEdge {
        id: edge.id,
        source_task_id: edge.source_task_id,
        target_task_id: edge.target_task_id,
        kind: edge_kind(edge.kind),
        required: edge.required,
        blocking: edge.blocking,
    }
}
use std::str::FromStr;
pub(crate) async fn task_neighborhood(
    State(state): State<AppState>,
    Path(path): Path<TaskNeighborhoodPath>,
    query: Result<Query<TaskNeighborhoodQuery>, QueryRejection>,
) -> Result<Json<TaskNeighborhoodResponse>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let graph = kanban_sqlite::api::task_neighborhood(
        state.db_path(),
        &path.task_id,
        kanban_sqlite::api::TaskNeighborhoodOptions {
            depth: query.depth,
            limit_nodes: query.limit_nodes,
            include_archived_context: query.include_archived_context,
        },
    )?;
    Ok(Json(DataEnvelope::new(ContractTaskNeighborhood {
        center_task_id: graph.center_task_id,
        nodes: graph
            .nodes
            .into_iter()
            .map(node)
            .collect::<Result<_, _>>()?,
        edges: graph.edges.into_iter().map(edge).collect(),
        meta: meta(graph.meta),
    })))
}
pub(crate) async fn board_task_map(
    State(state): State<AppState>,
    Path(path): Path<BoardTaskMapPath>,
    query: Result<Query<BoardTaskMapQuery>, QueryRejection>,
) -> Result<Json<BoardTaskMapResponse>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let graph = kanban_sqlite::api::board_task_map(
        state.db_path(),
        &path.board,
        kanban_sqlite::api::BoardTaskMapOptions {
            active_only: query.active_only,
            context_depth: query.context_depth,
            limit_nodes: query.limit_nodes,
            include_done_context: query.include_done_context,
            include_archived_context: query.include_archived_context,
            hide_isolated: query.hide_isolated,
        },
    )?;
    Ok(Json(DataEnvelope::new(ContractBoardTaskMap {
        nodes: graph
            .nodes
            .into_iter()
            .map(node)
            .collect::<Result<_, _>>()?,
        edges: graph.edges.into_iter().map(edge).collect(),
        meta: meta(graph.meta),
    })))
}
