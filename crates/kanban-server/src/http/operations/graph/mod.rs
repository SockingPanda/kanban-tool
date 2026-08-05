use crate::{
    error::ApiError,
    http::operations::tasks::support::{api_task, api_task_status},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State, rejection::QueryRejection},
    routing::{get, post},
};
use kanban_service::dto::{
    BoardTaskMapRecord, RelationRecord, TaskGraphEdgeRecord, TaskGraphMetaRecord,
    TaskGraphNodeRecord, TaskNeighborhoodRecord,
};
use kanban_service::operations::{
    BoardTaskMapOptions, GraphNeighborsOptions, GraphQueryOptions, TaskNeighborhoodOptions,
};
use kanban_service::KanbanError;
use kanban_protocol::cli_helpers::{CliGraphQueryBinding, CliGraphQueryOutput, CliGraphQueryRow};
use kanban_protocol::{
    ApiRelation, ApiRelationProvenance, BoardQuery, BoardTaskMap, BoardTaskMapPath,
    BoardTaskMapQuery, BoardTaskMapResponse, DataEnvelope, GraphMaintenance,
    GraphMaintenanceResponse, GraphNeighborsQuery, GraphNeighborsResponse, GraphQueryQuery,
    GraphStatus, GraphStatusResponse, LimitMeta, MetadataEnvelope, TaskGraphEdge, TaskGraphMeta,
    TaskGraphNode, TaskNeighborhood, TaskNeighborhoodPath, TaskNeighborhoodQuery,
    TaskNeighborhoodResponse,
};

pub(crate) async fn graph_status(
    State(state): State<AppState>,
    query: Result<Query<BoardQuery>, QueryRejection>,
) -> Result<Json<GraphStatusResponse>, ApiError> {
    let Query(query) =
        query.map_err(|error| KanbanError::InvalidInput(format!("invalid query: {error}")))?;
    let status = state.application().graph_status(&query.board).await?;
    Ok(Json(DataEnvelope::new(GraphStatus {
        backend: status.backend,
        enabled: status.enabled,
        message: status.message,
    })))
}

pub(crate) async fn graph_neighbors(
    State(state): State<AppState>,
    query: Result<Query<GraphNeighborsQuery>, QueryRejection>,
) -> Result<Json<GraphNeighborsResponse>, ApiError> {
    let Query(query) =
        query.map_err(|error| KanbanError::InvalidInput(format!("invalid query: {error}")))?;
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
    Ok(Json(MetadataEnvelope::new(relations, LimitMeta { limit })))
}

pub(crate) async fn graph_query(
    State(state): State<AppState>,
    query: Result<Query<GraphQueryQuery>, QueryRejection>,
) -> Result<Json<CliGraphQueryOutput>, ApiError> {
    let Query(query) =
        query.map_err(|error| KanbanError::InvalidInput(format!("invalid query: {error}")))?;
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
    Ok(Json(DataEnvelope::new(rows)))
}

pub(crate) async fn graph_rebuild(
    State(state): State<AppState>,
    query: Result<Query<BoardQuery>, QueryRejection>,
) -> Result<Json<GraphMaintenanceResponse>, ApiError> {
    let Query(query) =
        query.map_err(|error| KanbanError::InvalidInput(format!("invalid query: {error}")))?;
    let maintenance = state.application().graph_rebuild(&query.board).await?;
    Ok(Json(DataEnvelope::new(api_graph_maintenance(maintenance))))
}

pub(crate) async fn graph_sync(
    State(state): State<AppState>,
    query: Result<Query<BoardQuery>, QueryRejection>,
) -> Result<Json<GraphMaintenanceResponse>, ApiError> {
    let Query(query) =
        query.map_err(|error| KanbanError::InvalidInput(format!("invalid query: {error}")))?;
    let maintenance = state.application().graph_sync(&query.board).await?;
    Ok(Json(DataEnvelope::new(api_graph_maintenance(maintenance))))
}

pub(crate) async fn task_neighborhood(
    State(state): State<AppState>,
    Path(TaskNeighborhoodPath { task_id }): Path<TaskNeighborhoodPath>,
    query: Result<Query<TaskNeighborhoodQuery>, QueryRejection>,
) -> Result<Json<TaskNeighborhoodResponse>, ApiError> {
    let Query(query) =
        query.map_err(|error| KanbanError::InvalidInput(format!("invalid query: {error}")))?;
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
    Ok(Json(DataEnvelope::new(api_task_neighborhood(graph)?)))
}

pub(crate) async fn board_task_map(
    State(state): State<AppState>,
    Path(BoardTaskMapPath { board }): Path<BoardTaskMapPath>,
    query: Result<Query<BoardTaskMapQuery>, QueryRejection>,
) -> Result<Json<BoardTaskMapResponse>, ApiError> {
    let Query(query) =
        query.map_err(|error| KanbanError::InvalidInput(format!("invalid query: {error}")))?;
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
    Ok(Json(DataEnvelope::new(api_board_task_map(graph)?)))
}

fn api_relation(relation: RelationRecord) -> Result<ApiRelation, ApiError> {
    let metadata = serde_json::from_str(&relation.metadata_json).map_err(|error| {
        KanbanError::Storage(format!("stored relation metadata is invalid JSON: {error}"))
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

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/graph/status", get(graph_status))
        .route("/api/v1/graph/neighbors", get(graph_neighbors))
        .route("/api/v1/graph/query", get(graph_query))
        .route("/api/v1/graph/rebuild", post(graph_rebuild))
        .route("/api/v1/graph/sync", post(graph_sync))
        .route(
            "/api/v1/tasks/:task_id/neighborhood",
            get(task_neighborhood),
        )
        .route("/api/v1/boards/:board/task-map", get(board_task_map))
}

#[cfg(test)]
mod tests {
    use crate::http::operations::test_support::*;
    use kanban_protocol::cli_helpers::CliGraphMaintenance;
    use kanban_protocol::{BoardTaskMapResponse, DataEnvelope, GraphStatusResponse};

    #[tokio::test]
    async fn graph_status_and_task_map_routes_are_adopted() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/graph/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let status: GraphStatusResponse = serde_json::from_slice(&body).unwrap();
        assert!(status.data.enabled);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/boards/default/task-map")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let map: BoardTaskMapResponse = serde_json::from_slice(&body).unwrap();
        assert!(map.data.nodes.is_empty());
    }

    #[tokio::test]
    async fn neighborhood_missing_task_is_not_found() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let response = build_router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/v1/tasks/t_missing/neighborhood")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn graph_maintenance_routes_publish_generation_and_counts() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/graph/rebuild?board=default")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let rebuild: DataEnvelope<CliGraphMaintenance> = serde_json::from_slice(&body).unwrap();
        assert_eq!(rebuild.data.mode, "rebuild");
        assert!(!rebuild.data.generation.is_empty());
        assert!(!rebuild.data.fingerprint.is_empty());

        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/graph/sync?board=default")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let sync: DataEnvelope<CliGraphMaintenance> = serde_json::from_slice(&body).unwrap();
        assert_eq!(sync.data.mode, "sync");
        assert_eq!(sync.data.pending_jobs, 0);
    }
}
