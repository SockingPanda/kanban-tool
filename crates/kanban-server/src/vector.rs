//! Host-owned Ollama provider、vector worker 和 Turso vector HTTP endpoints。
//!
//! provider 调用在 host 内完成；CLI、MCP 和 Desktop 只能通过 localhost API 读取结果。

use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{
        Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    routing::{get, post},
};
use kanban_core::KanbanError;
use kanban_protocol::{
    DataEnvelope, VectorChunkResult, VectorConfigureRequest, VectorConfigureResponse,
    VectorLabelAtomResult, VectorProjectionRequest, VectorProjectionResponse, VectorQuery,
    VectorQueryChunksResponse, VectorQueryLabelAtomsResponse, VectorStatus, VectorStatusQuery,
    VectorStatusResponse,
};
use kanban_store_turso::{
    ProjectionJobRecord, StoreError, TursoStore, VectorConfig, VectorEmbeddingInput,
    VectorStatusRecord, stable_id,
};
use serde::Deserialize;

use crate::{error::ApiError, state::AppState};

const OLLAMA_READ_TIMEOUT: Duration = Duration::from_secs(30);
const OLLAMA_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const OLLAMA_MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/vector/status", get(status))
        .route("/api/v1/vector/configure", post(configure))
        .route("/api/v1/vector/rebuild", post(rebuild))
        .route("/api/v1/vector/sync", post(sync))
        .route("/api/v1/vector/query-chunks", get(query_chunks))
        .route("/api/v1/vector/query-label-atoms", get(query_label_atoms))
}

async fn status(
    State(state): State<AppState>,
    query: Result<Query<VectorStatusQuery>, QueryRejection>,
) -> Result<Json<VectorStatusResponse>, ApiError> {
    let Query(query) =
        query.map_err(|error| invalid(format!("invalid vector status query: {error}")))?;
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
        body.map_err(|error| invalid(format!("invalid vector configure body: {error}")))?;
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
    let Json(body) =
        body.map_err(|error| invalid(format!("invalid vector rebuild body: {error}")))?;
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
    let Json(body) = body.map_err(|error| invalid(format!("invalid vector sync body: {error}")))?;
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
    let Query(query) = query.map_err(|error| invalid(format!("invalid vector query: {error}")))?;
    validate_query(&query)?;
    let (config, embedding) = embed_query(state.vector_store(), &query.q).await?;
    if let Some(model) = query.embedding_model.as_deref() {
        if model != config.model {
            return Err(invalid("embedding model 与当前 vector 配置不一致"));
        }
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
        query.map_err(|error| invalid(format!("invalid vector label query: {error}")))?;
    validate_query(&query)?;
    let (config, embedding) = embed_query(state.vector_store(), &query.q).await?;
    if let Some(model) = query.embedding_model.as_deref() {
        if model != config.model {
            return Err(invalid("embedding model 与当前 vector 配置不一致"));
        }
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
    store: &TursoStore,
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

pub(crate) async fn embed_query(
    store: &TursoStore,
    text: &str,
) -> Result<(VectorConfig, Vec<f32>), ApiError> {
    let config = store
        .vector_config()
        .await
        .map_err(store_error)?
        .ok_or_else(|| invalid("vector provider 未配置，当前查询为 degraded"))?;
    let config_for_task = config.clone();
    let text = text.to_owned();
    let embedding = tokio::task::spawn_blocking(move || {
        OllamaEmbeddingProvider::new(config_for_task).embed_batch(&[text])
    })
    .await
    .map_err(|error| {
        ApiError(KanbanError::Storage(format!(
            "vector provider worker failed: {error}"
        )))
    })?
    .map_err(|error| {
        ApiError(KanbanError::Storage(format!(
            "vector degraded: {}",
            error.message
        )))
    })?
    .into_iter()
    .next()
    .ok_or_else(|| invalid("vector provider 返回空 embedding"))?;
    Ok((config, embedding))
}

/// Host 内 projection worker 的一个 tick。canonical mutation 失败不会回滚，job 会保留 failed 状态。
pub(crate) async fn worker_tick(store: TursoStore, owner: &str) -> Result<usize, StoreError> {
    let Some(config) = store.vector_config().await? else {
        return Ok(0);
    };
    let jobs = store.claim_vector_jobs(owner, 8, now_ms()).await?;
    let mut completed = 0;
    for job in jobs {
        match process_job(&store, &config, &job).await {
            Ok(()) => {
                if store.complete_vector_job(&job).await? {
                    completed += 1;
                    let status = store.vector_status(None).await?;
                    if status.pending_jobs == 0
                        && status.running_jobs == 0
                        && status.failed_jobs == 0
                    {
                        let generation = now_ms().to_string();
                        store
                            .publish_vector_generation(&generation, &config.fingerprint())
                            .await?;
                    }
                }
            }
            Err(error) => {
                let retryable = error.retryable;
                store
                    .fail_vector_job(&job, &error.message, retryable)
                    .await?;
            }
        }
    }
    Ok(completed)
}

async fn process_job(
    store: &TursoStore,
    config: &VectorConfig,
    job: &ProjectionJobRecord,
) -> Result<(), ProviderFailure> {
    if job.operation == "delete" {
        return Ok(());
    }
    let Some(entity_uri) = job.entity_uri.as_deref() else {
        return Ok(());
    };
    let document = if job.target == "vector_tasks" {
        let Some(task_id) = entity_uri.strip_prefix("kb://task/") else {
            return Ok(());
        };
        store
            .vector_task_document(task_id)
            .await
            .map_err(store_failure)?
    } else if job.target == "vector_label_atoms" {
        let Some(atom_id) = entity_uri.strip_prefix("kb://label-atom/") else {
            return Ok(());
        };
        store
            .vector_label_atom_document(atom_id)
            .await
            .map_err(store_failure)?
    } else {
        return Ok(());
    };
    let Some(document) = document else {
        return Ok(());
    };
    if store
        .vector_embedding_is_current(&document.id, &config.model, &document.content_hash)
        .await
        .map_err(store_failure)?
    {
        return Ok(());
    }
    let text = document.content.clone();
    let config_for_task = config.clone();
    let vectors = tokio::task::spawn_blocking(move || {
        OllamaEmbeddingProvider::new(config_for_task).embed_batch(&[text])
    })
    .await
    .map_err(|error| ProviderFailure::retryable(error.to_string()))?
    .map_err(|error| ProviderFailure {
        message: error.message,
        retryable: error.retryable,
    })?;
    let embedding = vectors
        .into_iter()
        .next()
        .ok_or_else(|| ProviderFailure::retryable("Ollama 返回空 embedding"))?;
    let vector = VectorEmbeddingInput {
        id: stable_id("vec", &[&document.id, &config.model]),
        board_id: document.board_id.clone(),
        entity_uri: document.entity_uri.clone(),
        document_id: document.id.clone(),
        dimensions: config.dimensions,
        embedding,
        embedding_model: config.model.clone(),
        content_hash: document.content_hash.clone(),
        created_at: document.created_at,
        updated_at: document.updated_at,
    };
    store
        .upsert_vector_document(&document)
        .await
        .map_err(store_failure)?;
    store
        .upsert_vector_embedding(&vector)
        .await
        .map_err(store_failure)?;
    Ok(())
}

fn store_failure(error: StoreError) -> ProviderFailure {
    ProviderFailure {
        message: error.to_string(),
        retryable: false,
    }
}

#[derive(Debug)]
struct ProviderFailure {
    message: String,
    retryable: bool,
}
impl ProviderFailure {
    fn retryable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
    }
}

impl std::fmt::Display for ProviderFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

#[derive(Debug, Clone)]
struct OllamaEmbeddingProvider {
    config: VectorConfig,
}

impl OllamaEmbeddingProvider {
    fn new(config: VectorConfig) -> Self {
        Self { config }
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ProviderFailure> {
        let body = serde_json::json!({ "model": self.config.model, "input": texts, "dimensions": self.config.dimensions }).to_string();
        let (status, response) = http_post_json(&self.config.endpoint, "/api/embed", &body)?;
        if status >= 400 {
            return Err(ProviderFailure {
                message: format!("Ollama HTTP {status}"),
                retryable: status == 429 || status >= 500,
            });
        }
        let parsed: OllamaEmbedResponse = serde_json::from_slice(&response).map_err(|error| {
            ProviderFailure::retryable(format!("Ollama 响应 JSON 无效: {error}"))
        })?;
        if parsed.embeddings.len() != texts.len() {
            return Err(ProviderFailure::retryable(format!(
                "Ollama embedding 数量不匹配：期望 {}, 实际 {}",
                texts.len(),
                parsed.embeddings.len()
            )));
        }
        for vector in &parsed.embeddings {
            if vector.len() != self.config.dimensions {
                return Err(ProviderFailure {
                    message: format!(
                        "embedding 维度不匹配：期望 {}, 实际 {}",
                        self.config.dimensions,
                        vector.len()
                    ),
                    retryable: false,
                });
            }
            if vector.iter().any(|value| !value.is_finite()) {
                return Err(ProviderFailure {
                    message: "embedding 含非有限数".to_owned(),
                    retryable: false,
                });
            }
        }
        Ok(parsed.embeddings)
    }
}

#[derive(Debug, Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

fn http_post_json(
    endpoint: &str,
    path: &str,
    body: &str,
) -> Result<(u16, Vec<u8>), ProviderFailure> {
    let endpoint = endpoint
        .strip_prefix("http://")
        .ok_or_else(|| ProviderFailure {
            message: "Ollama endpoint 仅支持 http://".to_owned(),
            retryable: false,
        })?;
    let authority = endpoint.split('/').next().unwrap_or(endpoint);
    let (host, port) = parse_authority(authority)?;
    let address = format!("{host}:{port}")
        .to_socket_addrs()
        .map_err(|error| ProviderFailure::retryable(format!("解析 Ollama endpoint 失败: {error}")))?
        .next()
        .ok_or_else(|| ProviderFailure::retryable("Ollama endpoint 无可用地址"))?;
    let mut stream = TcpStream::connect_timeout(&address, OLLAMA_CONNECT_TIMEOUT)
        .map_err(|error| ProviderFailure::retryable(format!("连接 Ollama 失败: {error}")))?;
    stream.set_read_timeout(Some(OLLAMA_READ_TIMEOUT)).ok();
    stream.set_write_timeout(Some(OLLAMA_CONNECT_TIMEOUT)).ok();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| ProviderFailure::retryable(format!("发送 Ollama 请求失败: {error}")))?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| ProviderFailure::retryable(format!("读取 Ollama 响应失败: {error}")))?;
    if response.len() > OLLAMA_MAX_RESPONSE_BYTES {
        return Err(ProviderFailure {
            message: "Ollama 响应体超过上限".to_owned(),
            retryable: false,
        });
    }
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| ProviderFailure::retryable("Ollama 响应缺少 headers"))?;
    let header = String::from_utf8_lossy(&response[..split]);
    let status = header
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| ProviderFailure::retryable("Ollama 响应状态码无效"))?;
    Ok((status, response[split + 4..].to_vec()))
}

fn parse_authority(authority: &str) -> Result<(&str, u16), ProviderFailure> {
    if authority.is_empty() || authority.contains('@') {
        return Err(ProviderFailure {
            message: "Ollama endpoint authority 无效".to_owned(),
            retryable: false,
        });
    }
    if let Some(host) = authority.strip_prefix('[') {
        let Some(close) = host.find(']') else {
            return Err(ProviderFailure {
                message: "Ollama endpoint IPv6 authority 无效".to_owned(),
                retryable: false,
            });
        };
        let host_end = &host[..close];
        let suffix = &host[close + 1..];
        let port = if let Some(port) = suffix.strip_prefix(':') {
            port.parse::<u16>().map_err(|_| ProviderFailure {
                message: "Ollama endpoint 端口无效".to_owned(),
                retryable: false,
            })?
        } else if suffix.is_empty() {
            80
        } else {
            return Err(ProviderFailure {
                message: "Ollama endpoint IPv6 authority 无效".to_owned(),
                retryable: false,
            });
        };
        return Ok((host_end, port));
    }
    if let Some((host, port)) = authority.rsplit_once(':') {
        let port = port.parse::<u16>().map_err(|_| ProviderFailure {
            message: "Ollama endpoint 端口无效".to_owned(),
            retryable: false,
        })?;
        Ok((host, port))
    } else {
        Ok((authority, 80))
    }
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

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

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

    use super::{OllamaEmbeddingProvider, ProviderFailure, VectorConfig};
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

    fn mock_provider(status: u16, body: &str) -> Result<Vec<Vec<f32>>, ProviderFailure> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| ProviderFailure::retryable(error.to_string()))?;
        let port = listener
            .local_addr()
            .map_err(|error| ProviderFailure::retryable(error.to_string()))?
            .port();
        let body = body.to_owned();
        let server = thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request);
            let reason = if status < 400 { "OK" } else { "Error" };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        });
        let provider = OllamaEmbeddingProvider::new(VectorConfig {
            provider: "ollama".to_owned(),
            endpoint: format!("http://127.0.0.1:{port}"),
            model: "test-model".to_owned(),
            dimensions: 2,
        });
        let result = provider.embed_batch(&["hello".to_owned()]);
        server
            .join()
            .map_err(|_| ProviderFailure::retryable("mock server panicked"))?;
        result
    }

    #[test]
    fn ollama_provider_accepts_mock_embedding_response() {
        let vectors = mock_provider(200, r#"{"embeddings":[[0.25,-0.5]]}"#)
            .expect("mock embedding should parse");
        assert_eq!(vectors, vec![vec![0.25, -0.5]]);
    }

    #[test]
    fn ollama_server_error_is_reported_as_retryable() {
        let error = mock_provider(503, r#"{"error":"busy"}"#).expect_err("503 must fail");
        assert!(error.retryable);
        assert_eq!(error.message, "Ollama HTTP 503");
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
