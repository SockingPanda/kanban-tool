use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, Query, State, rejection::QueryRejection},
    routing::get,
};
use kanban_service::dto::EntityRecord;
use kanban_service::operations::{EntityListOptions, EntityUpsertCommand};
use kanban_service::KanbanError;
use kanban_protocol::{
    CliEntity, CliEntityListOutput, CliEntityShowOutput, DataEnvelope, EntityListQuery, EntityPath,
    EntityUpsertRequest,
};

pub(crate) async fn list_entities(
    State(state): State<AppState>,
    query: Result<Query<EntityListQuery>, QueryRejection>,
) -> Result<Json<CliEntityListOutput>, ApiError> {
    let Query(query) =
        query.map_err(|error| KanbanError::InvalidInput(format!("invalid query: {error}")))?;
    let entities = state
        .application()
        .list_entities(EntityListOptions {
            board: query.board,
            kind: query.kind,
            limit: query.limit,
        })
        .await?
        .into_iter()
        .map(api_entity)
        .collect();
    Ok(Json(DataEnvelope::new(entities)))
}

pub(crate) async fn get_entity(
    State(state): State<AppState>,
    Path(EntityPath { uri }): Path<EntityPath>,
) -> Result<Json<CliEntityShowOutput>, ApiError> {
    let entity = state.application().get_entity(&uri).await?;
    Ok(Json(DataEnvelope::new(api_entity(entity))))
}

pub(crate) async fn upsert_entity(
    State(state): State<AppState>,
    Json(request): Json<EntityUpsertRequest>,
) -> Result<Json<CliEntityShowOutput>, ApiError> {
    let entity = state
        .application()
        .upsert_entity(EntityUpsertCommand {
            uri: request.uri,
            kind: request.kind,
            source_table: request.source_table,
            source_id: request.source_id,
            board: request.board,
            task_id: request.task_id,
            title: request.title,
            summary: request.summary,
            content_hash: request.content_hash,
            archived_at: request.archived_at,
        })
        .await?;
    Ok(Json(DataEnvelope::new(api_entity(entity))))
}

fn api_entity(entity: EntityRecord) -> CliEntity {
    CliEntity {
        uri: entity.uri,
        kind: entity.kind,
        source_table: entity.source_table,
        source_id: entity.source_id,
        board_id: entity.board_id,
        task_id: entity.task_id,
        title: entity.title,
        summary: entity.summary,
        content_hash: entity.content_hash,
        created_at: entity.created_at,
        updated_at: entity.updated_at,
        archived_at: entity.archived_at,
    }
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/entities", get(list_entities).put(upsert_entity))
        .route("/api/v1/entities/:uri", get(get_entity))
}

#[cfg(test)]
mod tests {
    use crate::http::operations::test_support::*;
    use kanban_protocol::{ApiErrorCode, ErrorEnvelope};

    #[tokio::test]
    async fn entity_upsert_list_and_show_are_available_on_host() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);

        let request = Request::builder()
            .method("PUT")
            .uri("/api/v1/entities")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "uri": "kb://task/t_entity_http",
                    "kind": "task",
                    "source_table": "tasks",
                    "source_id": "t_entity_http"
                }))
                .unwrap(),
            ))
            .unwrap();
        let response = router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/entities?kind=task")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let entities: CliEntityListOutput = serde_json::from_slice(&body).unwrap();
        assert_eq!(entities.data.len(), 1);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/entities/kb%3A%2F%2Ftask%2Ft_entity_http")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn unknown_entity_is_not_found() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let response = build_router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/v1/entities/kb%3A%2F%2Fmissing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorEnvelope = serde_json::from_slice(&body).unwrap();
        assert_eq!(error.error.code, ApiErrorCode::NotFound);
    }
}
