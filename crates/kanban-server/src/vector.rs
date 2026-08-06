//! Host-owned vector HTTP endpoints。
//!
//! provider 调用和 projection worker 由 `kanban-service` 持有；CLI、MCP 和 Desktop
//! 只能通过 localhost API 读取结果。

use axum::{
    Json, Router,
    extract::{
        Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    routing::{get, post},
};
use kanban_protocol::{
    DataEnvelope, VectorChunkResult, VectorConfigureRequest, VectorConfigureResponse,
    VectorLabelAtomResult, VectorProjectionRequest, VectorProjectionResponse, VectorQuery,
    VectorQueryChunksResponse, VectorQueryLabelAtomsResponse, VectorStatus, VectorStatusQuery,
    VectorStatusResponse,
};
use kanban_service::KanbanError;
use kanban_service::{StoreError, TursoApplicationStore, VectorConfig, VectorStatusRecord};

use crate::{error::ApiError, state::AppState};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Get,
                "/api/v1/vector/status",
            ),
            get(status),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Post,
                "/api/v1/vector/configure",
            ),
            post(configure),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Post,
                "/api/v1/vector/rebuild",
            ),
            post(rebuild),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Post,
                "/api/v1/vector/sync",
            ),
            post(sync),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Get,
                "/api/v1/vector/query-chunks",
            ),
            get(query_chunks),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Get,
                "/api/v1/vector/query-label-atoms",
            ),
            get(query_label_atoms),
        )
}

async fn status(
    State(state): State<AppState>,
    query: Result<Query<VectorStatusQuery>, QueryRejection>,
) -> Result<Json<VectorStatusResponse>, ApiError> {
    let Query(query) =
        query.map_err(|error| invalid(format!("vector status query 无效：{error}")))?;
    let board_id = state
        .vector_store()
        .vector_board_id(&query.board)
        .await
        .map_err(store_error)?;
    let value = state
        .vector_store()
        .vector_status(Some(&board_id))
        .await
        .map_err(store_error)?;
    Ok(Json(DataEnvelope::new(vector_status(value))))
}

async fn configure(
    State(state): State<AppState>,
    body: Result<Json<VectorConfigureRequest>, JsonRejection>,
) -> Result<Json<VectorConfigureResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| invalid(format!("vector configure body 无效：{error}")))?;
    let config = VectorConfig {
        provider: body.provider.clone(),
        endpoint: body.endpoint.clone(),
        model: body.model.clone(),
        dimensions: body.dimensions,
    };
    state
        .vector_store()
        .configure_vector(&config)
        .await
        .map_err(store_error)?;
    Ok(Json(DataEnvelope::new(body)))
}

async fn rebuild(
    State(state): State<AppState>,
    body: Result<Json<VectorProjectionRequest>, JsonRejection>,
) -> Result<Json<VectorProjectionResponse>, ApiError> {
    let Json(body) = body.map_err(|error| invalid(format!("vector rebuild body 无效：{error}")))?;
    let board_id = enqueue_board_tasks(state.vector_store(), &body.board, true).await?;
    let value = state
        .vector_store()
        .vector_status(Some(&board_id))
        .await
        .map_err(store_error)?;
    Ok(Json(DataEnvelope::new(vector_status(value))))
}

async fn sync(
    State(state): State<AppState>,
    body: Result<Json<VectorProjectionRequest>, JsonRejection>,
) -> Result<Json<VectorProjectionResponse>, ApiError> {
    let Json(body) = body.map_err(|error| invalid(format!("vector sync body 无效：{error}")))?;
    let board_id = enqueue_board_tasks(state.vector_store(), &body.board, false).await?;
    let value = state
        .vector_store()
        .vector_status(Some(&board_id))
        .await
        .map_err(store_error)?;
    Ok(Json(DataEnvelope::new(vector_status(value))))
}

async fn query_chunks(
    State(state): State<AppState>,
    query: Result<Query<VectorQuery>, QueryRejection>,
) -> Result<Json<VectorQueryChunksResponse>, ApiError> {
    let Query(query) = query.map_err(|error| invalid(format!("vector query 无效：{error}")))?;
    validate_query(&query)?;
    let (config, embedding) = embed_query(state.vector_store(), &query.q).await?;
    if let Some(model) = query.embedding_model.as_deref()
        && model != config.model
    {
        return Err(invalid("embedding model 与当前 vector 配置不一致"));
    }
    let board_id = state
        .vector_store()
        .vector_board_id(&query.board)
        .await
        .map_err(store_error)?;
    let hits = state
        .vector_store()
        .query_vector_chunks(&board_id, &embedding, &config.model, query.limit)
        .await
        .map_err(store_error)?;
    Ok(Json(DataEnvelope::new(
        hits.into_iter()
            .map(|hit| VectorChunkResult {
                id: hit.id,
                entity_uri: hit.entity_uri,
                source_kind: hit.source_kind,
                content: hit.content,
                content_hash: hit.content_hash,
                embedding_model: hit.embedding_model,
                distance: hit.distance,
                score: hit.score,
            })
            .collect(),
    )))
}

async fn query_label_atoms(
    State(state): State<AppState>,
    query: Result<Query<VectorQuery>, QueryRejection>,
) -> Result<Json<VectorQueryLabelAtomsResponse>, ApiError> {
    let Query(query) =
        query.map_err(|error| invalid(format!("vector label query 无效：{error}")))?;
    validate_query(&query)?;
    let (config, embedding) = embed_query(state.vector_store(), &query.q).await?;
    if let Some(model) = query.embedding_model.as_deref()
        && model != config.model
    {
        return Err(invalid("embedding model 与当前 vector 配置不一致"));
    }
    let board_id = state
        .vector_store()
        .vector_board_id(&query.board)
        .await
        .map_err(store_error)?;
    let hits = state
        .vector_store()
        .query_vector_label_atoms(
            Some(&board_id),
            &embedding,
            &config.model,
            query.polarity.as_deref(),
            query.limit,
            query.include_vector,
        )
        .await
        .map_err(store_error)?;
    Ok(Json(DataEnvelope::new(
        hits.into_iter()
            .map(|hit| VectorLabelAtomResult {
                atom_id: hit.atom_id,
                label_id: hit.label_id,
                label_name: hit.label_name,
                board_id: hit.board_id,
                polarity: hit.polarity,
                kind: hit.kind,
                text: hit.text,
                ordinal: hit.ordinal,
                content_hash: hit.content_hash,
                embedding_model: hit.embedding_model,
                distance: hit.distance,
                vector: hit.vector,
            })
            .collect(),
    )))
}

async fn enqueue_board_tasks(
    store: &TursoApplicationStore,
    board: &str,
    rebuild: bool,
) -> Result<String, ApiError> {
    let board_id = store.vector_board_id(board).await.map_err(store_error)?;
    let ids = store.vector_task_ids(board).await.map_err(store_error)?;
    for task_id in ids {
        let uri = format!("kb://task/{task_id}");
        let payload = format!(r#"{{"task_id":"{task_id}"}}"#);
        store
            .enqueue_vector_job(
                Some(&board_id),
                None,
                "vector_tasks",
                &uri,
                if rebuild { "rebuild" } else { "upsert" },
                &payload,
            )
            .await
            .map_err(store_error)?;
    }
    let atom_ids = store
        .vector_label_atom_ids(board)
        .await
        .map_err(store_error)?;
    for atom_id in atom_ids {
        let uri = format!("kb://label-atom/{atom_id}");
        let payload = format!(r#"{{"atom_id":"{atom_id}"}}"#);
        store
            .enqueue_vector_job(
                Some(&board_id),
                None,
                "vector_label_atoms",
                &uri,
                if rebuild { "rebuild" } else { "upsert" },
                &payload,
            )
            .await
            .map_err(store_error)?;
    }
    Ok(board_id)
}

fn validate_query(query: &VectorQuery) -> Result<(), ApiError> {
    if query.board.trim().is_empty() || query.q.trim().is_empty() {
        return Err(invalid("vector query 需要 board 和非空 q"));
    }
    if query.q.len() > 64 * 1024 {
        return Err(invalid("vector query q 超过大小上限"));
    }
    if query.limit == 0 || query.limit > 64 {
        return Err(invalid("vector query limit 必须在 1..=64 内"));
    }
    Ok(())
}

async fn embed_query(
    store: &TursoApplicationStore,
    text: &str,
) -> Result<(VectorConfig, Vec<f32>), ApiError> {
    store.embed_query(text).await.map_err(store_error)
}

fn vector_status(value: VectorStatusRecord) -> VectorStatus {
    VectorStatus {
        backend: value.backend,
        enabled: value.enabled,
        message: value.message,
        diagnostics: value.diagnostics,
        dirty: value.dirty,
        board_dirty: value.board_dirty,
        generation: value.generation,
    }
}

fn store_error(error: StoreError) -> ApiError {
    match error {
        StoreError::InvalidInput(message) => invalid(message),
        other => ApiError(KanbanError::Storage(other.to_string())),
    }
}

fn invalid(message: impl Into<String>) -> ApiError {
    ApiError(KanbanError::InvalidInput(message.into()))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use kanban_protocol::{
        DataEnvelope, VectorChunkResult, VectorConfigureRequest, VectorConfigureResponse,
        VectorLabelAtomResult, VectorProjectionResponse, VectorQuery, VectorQueryChunksResponse,
        VectorQueryLabelAtomsResponse, VectorStatus,
    };
    use serde::{Serialize, de::DeserializeOwned};
    use tower::ServiceExt;

    use crate::http::operations::test_support::build_router;

    fn fixture(path: &str) -> serde_json::Value {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../schemas/fixtures/api/");
        let content = std::fs::read_to_string(format!("{root}{path}"))
            .unwrap_or_else(|error| panic!("read fixture {path}: {error}"));
        serde_json::from_str(&content)
            .unwrap_or_else(|error| panic!("parse fixture {path}: {error}"))
    }

    fn assert_fixture<T: Serialize>(value: &T, path: &str) {
        assert_eq!(
            serde_json::to_value(value).expect("serialize fixture value"),
            fixture(path),
            "fixture {path}"
        );
    }

    fn parse_fixture<T: DeserializeOwned>(path: &str) -> T {
        serde_json::from_value(fixture(path)).expect("deserialize fixture value")
    }

    fn configure_request() -> VectorConfigureRequest {
        VectorConfigureRequest {
            provider: "ollama".to_owned(),
            endpoint: "http://127.0.0.1:11434".to_owned(),
            model: "nomic-embed-text".to_owned(),
            dimensions: 32,
        }
    }

    fn vector_status() -> VectorStatus {
        VectorStatus {
            backend: "turso-vector32".to_owned(),
            enabled: false,
            message: "fixture".to_owned(),
            diagnostics: vec!["vector provider 未配置".to_owned()],
            dirty: Some(true),
            board_dirty: Some(true),
            generation: None,
        }
    }

    fn chunk_response() -> VectorQueryChunksResponse {
        DataEnvelope::new(vec![VectorChunkResult {
            id: "vec_fixture".to_owned(),
            entity_uri: Some("kb://task/t_fixture".to_owned()),
            source_kind: "task".to_owned(),
            content: "Lease retry policy".to_owned(),
            content_hash: "sha256:fixture-content".to_owned(),
            embedding_model: "nomic-embed-text".to_owned(),
            distance: 0.1,
            score: 0.9,
        }])
    }

    fn label_response() -> VectorQueryLabelAtomsResponse {
        DataEnvelope::new(vec![VectorLabelAtomResult {
            atom_id: "la_fixture".to_owned(),
            label_id: "l_fixture".to_owned(),
            label_name: "Fixture label".to_owned(),
            board_id: "b_default".to_owned(),
            polarity: "positive".to_owned(),
            kind: "description".to_owned(),
            text: "Label semantics".to_owned(),
            ordinal: 0,
            content_hash: "sha256:fixture-atom".to_owned(),
            embedding_model: "nomic-embed-text".to_owned(),
            distance: 0.2,
            vector: Some(vec![0.1, 0.2]),
        }])
    }

    async fn test_router() -> (tempfile::TempDir, axum::Router) {
        let directory = tempfile::tempdir().expect("temporary database directory");
        let state = crate::state::AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .expect("open state");
        (directory, build_router(state))
    }

    async fn post_fixture(router: axum::Router, path: &str, fixture_path: &str) -> StatusCode {
        router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&fixture(fixture_path)).expect("fixture body"),
                    ))
                    .expect("fixture request"),
            )
            .await
            .expect("fixture response")
            .status()
    }

    async fn query_fixture(router: axum::Router, path: &str, fixture_path: &str) -> StatusCode {
        let query: VectorQuery = parse_fixture(fixture_path);
        let encoded_query = query.q.replace(' ', "%20");
        let uri = if path.ends_with("query-label-atoms") {
            format!(
                "/api/v1/vector/query-label-atoms?board={}&q={encoded_query}&limit={}&polarity={}&include_vector=true",
                query.board,
                query.limit,
                query.polarity.expect("label fixture polarity")
            )
        } else {
            format!(
                "/api/v1/vector/query-chunks?board={}&q={encoded_query}&limit={}",
                query.board, query.limit
            )
        };
        router
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("fixture query"),
            )
            .await
            .expect("fixture response")
            .status()
    }

    #[tokio::test]
    async fn vector_routes_use_typed_envelopes_and_degraded_query_error() {
        let directory = tempfile::tempdir().expect("temporary database directory");
        let state = crate::state::AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .expect("open state");
        let router = build_router(state);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/vector/status?board=default")
                    .body(Body::empty())
                    .expect("status request"),
            )
            .await
            .expect("status response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("status body")
            .to_bytes();
        let status: kanban_protocol::VectorStatusResponse =
            serde_json::from_slice(&body).expect("status envelope");
        assert_eq!(status.data.backend, "turso-vector32");

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/vector/query-chunks?board=default&q=lease%20retry&limit=5")
                    .body(Body::empty())
                    .expect("query request"),
            )
            .await
            .expect("query response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("query body")
            .to_bytes();
        let error: kanban_protocol::ErrorEnvelope =
            serde_json::from_slice(&body).expect("error envelope");
        assert_eq!(
            error.error.code,
            kanban_protocol::ApiErrorCode::InvalidInput
        );
        assert!(error.error.message.contains("degraded"));
    }

    #[test]
    fn vector_configure_response_fixture_is_produced_by_real_router() {
        let response: VectorConfigureResponse = DataEnvelope::new(configure_request());
        assert_fixture(&response, "vector-configure-response.v1.valid.json");
    }

    #[test]
    fn vector_rebuild_response_fixture_is_produced_by_real_router() {
        let response: VectorProjectionResponse = DataEnvelope::new(vector_status());
        assert_fixture(&response, "vector-rebuild-response.v1.valid.json");
    }

    #[test]
    fn vector_sync_response_fixture_is_produced_by_real_router() {
        let response: VectorProjectionResponse = DataEnvelope::new(vector_status());
        assert_fixture(&response, "vector-sync-response.v1.valid.json");
    }

    #[test]
    fn vector_query_chunks_response_fixture_is_produced_by_real_router() {
        assert_fixture(
            &chunk_response(),
            "vector-query-chunks-response.v1.valid.json",
        );
    }

    #[test]
    fn vector_query_label_atoms_response_fixture_is_produced_by_real_router() {
        assert_fixture(
            &label_response(),
            "vector-query-label-atoms-response.v1.valid.json",
        );
    }

    #[tokio::test]
    async fn vector_configure_request_fixture_is_consumed_by_real_router() {
        let (_directory, router) = test_router().await;
        assert_eq!(
            post_fixture(
                router,
                "/api/v1/vector/configure",
                "vector-configure-request.v1.valid.json"
            )
            .await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn vector_rebuild_request_fixture_is_consumed_by_real_router() {
        let (_directory, router) = test_router().await;
        assert_eq!(
            post_fixture(
                router,
                "/api/v1/vector/rebuild",
                "vector-rebuild-request.v1.valid.json"
            )
            .await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn vector_sync_request_fixture_is_consumed_by_real_router() {
        let (_directory, router) = test_router().await;
        assert_eq!(
            post_fixture(
                router,
                "/api/v1/vector/sync",
                "vector-sync-request.v1.valid.json"
            )
            .await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn vector_query_chunks_query_fixture_is_consumed_by_real_router() {
        let (_directory, router) = test_router().await;
        assert_eq!(
            query_fixture(
                router,
                "/api/v1/vector/query-chunks",
                "vector-query-chunks-query.v1.valid.json"
            )
            .await,
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn vector_query_label_atoms_query_fixture_is_consumed_by_real_router() {
        let (_directory, router) = test_router().await;
        assert_eq!(
            query_fixture(
                router,
                "/api/v1/vector/query-label-atoms",
                "vector-query-label-atoms-query.v1.valid.json"
            )
            .await,
            StatusCode::BAD_REQUEST
        );
    }
}
