use std::{convert::Infallible, fs, net::SocketAddr, path::PathBuf, str::FromStr, time::Duration};

use axum::{
    Json, Router,
    extract::{
        Path, Query, RawQuery, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{
        IntoResponse, Response,
        sse::{Event, Sse},
    },
    routing::{delete, get, post},
};
use futures_util::stream;
use kanban_core::{KanbanError, TaskStatus};
use kanban_entity::{EntityUri, Predicate};
use kanban_search::SearchQuery;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::cors::{AllowOrigin, CorsLayer};

#[derive(Debug, Clone)]
pub struct AppState {
    db_path: PathBuf,
    default_actor: String,
}

impl AppState {
    pub fn new(db_path: impl Into<PathBuf>, default_actor: impl Into<String>) -> Self {
        Self {
            db_path: db_path.into(),
            default_actor: default_actor.into(),
        }
    }

    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }

    pub fn default_actor(&self) -> &str {
        &self.default_actor
    }
}

#[derive(Debug, Clone)]
pub struct SearchSyncConfig {
    board: String,
    interval: Duration,
}

impl SearchSyncConfig {
    pub fn new(board: impl Into<String>, interval: Duration) -> Self {
        Self {
            board: board.into(),
            interval,
        }
    }

    pub fn disabled(board: impl Into<String>) -> Self {
        Self::new(board, Duration::ZERO)
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    pub fn board(&self) -> &str {
        &self.board
    }
}

pub fn search_sync_task_enabled(config: &SearchSyncConfig) -> bool {
    #[cfg(feature = "tantivy-backend")]
    {
        !config.interval.is_zero()
    }
    #[cfg(not(feature = "tantivy-backend"))]
    {
        let _ = config;
        false
    }
}

pub fn spawn_search_sync_task(
    state: AppState,
    config: SearchSyncConfig,
) -> Option<tokio::task::JoinHandle<()>> {
    if !search_sync_task_enabled(&config) {
        return None;
    }

    #[cfg(feature = "tantivy-backend")]
    {
        Some(tokio::spawn(async move {
            run_search_sync_once(state.db_path.clone(), config.board.clone()).await;
            loop {
                tokio::time::sleep(config.interval).await;
                run_search_sync_once(state.db_path.clone(), config.board.clone()).await;
            }
        }))
    }
    #[cfg(not(feature = "tantivy-backend"))]
    {
        let _ = (state, config);
        None
    }
}

#[cfg(feature = "tantivy-backend")]
async fn run_search_sync_once(db_path: PathBuf, board: String) {
    let _ = tokio::task::spawn_blocking(move || kanban_sqlite::sync_search_index(db_path, &board))
        .await;
}

#[derive(Debug, Serialize)]
struct Envelope<T> {
    data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

#[derive(Debug, Serialize)]
struct TaskDto {
    id: String,
    board_id: String,
    seq: i64,
    title: String,
    description: Option<String>,
    status: TaskStatus,
    status_reason: Option<String>,
    assignee: Option<String>,
    priority: i64,
    position: i64,
    scheduled_at: Option<i64>,
    due_at: Option<i64>,
    created_by: String,
    created_at: i64,
    updated_at: i64,
    started_at: Option<i64>,
    completed_at: Option<i64>,
    archived_at: Option<i64>,
    claim_owner: Option<String>,
    claim_expires_at: Option<i64>,
    last_heartbeat_at: Option<i64>,
    current_run_id: Option<String>,
    retry_count: i64,
    max_retries: Option<i64>,
    result_summary: Option<String>,
    result_json: Option<String>,
    metadata_json: String,
    lock_version: i64,
}

impl From<kanban_sqlite::TaskRecord> for TaskDto {
    fn from(task: kanban_sqlite::TaskRecord) -> Self {
        Self {
            id: task.id,
            board_id: task.board_id,
            seq: task.seq,
            title: task.title,
            description: task.description,
            status: task.status,
            status_reason: task.status_reason,
            assignee: task.assignee,
            priority: task.priority,
            position: task.position,
            scheduled_at: task.scheduled_at,
            due_at: task.due_at,
            created_by: task.created_by,
            created_at: task.created_at,
            updated_at: task.updated_at,
            started_at: task.started_at,
            completed_at: task.completed_at,
            archived_at: task.archived_at,
            claim_owner: task.claim_owner,
            claim_expires_at: task.claim_expires_at,
            last_heartbeat_at: task.last_heartbeat_at,
            current_run_id: task.current_run_id,
            retry_count: task.retry_count,
            max_retries: task.max_retries,
            result_summary: task.result_summary,
            result_json: task.result_json,
            metadata_json: task.metadata_json,
            lock_version: task.lock_version,
        }
    }
}

#[derive(Debug, Serialize)]
struct RunDto {
    id: String,
    task_id: String,
    status: String,
    worker_profile: Option<String>,
    worker_pid: Option<i64>,
    claim_owner: String,
    started_at: i64,
    finished_at: Option<i64>,
    exit_code: Option<i64>,
    summary: Option<String>,
    error: Option<String>,
    log_path: Option<String>,
    metadata_json: String,
}

impl From<kanban_sqlite::RunRecord> for RunDto {
    fn from(run: kanban_sqlite::RunRecord) -> Self {
        Self {
            id: run.id,
            task_id: run.task_id,
            status: run.status,
            worker_profile: run.worker_profile,
            worker_pid: run.worker_pid,
            claim_owner: run.claim_owner,
            started_at: run.started_at,
            finished_at: run.finished_at,
            exit_code: run.exit_code,
            summary: run.summary,
            error: run.error,
            log_path: run.log_path,
            metadata_json: run.metadata_json,
        }
    }
}

#[derive(Debug, Serialize)]
struct EventDto {
    id: i64,
    event_id: String,
    board_id: String,
    task_id: Option<String>,
    run_id: Option<String>,
    kind: String,
    actor: Option<String>,
    payload: serde_json::Value,
    created_at: i64,
}

#[derive(Debug, Serialize)]
struct CommentDto {
    id: String,
    board_id: String,
    task_id: String,
    author: String,
    body: String,
    kind: String,
    created_at: i64,
}

impl From<kanban_sqlite::CommentRecord> for CommentDto {
    fn from(comment: kanban_sqlite::CommentRecord) -> Self {
        Self {
            id: comment.id,
            board_id: comment.board_id,
            task_id: comment.task_id,
            author: comment.author,
            body: comment.body,
            kind: comment.kind,
            created_at: comment.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
struct RunLogDto {
    run_id: String,
    content: String,
    truncated: bool,
}

#[derive(Debug, Serialize)]
struct ClaimDto {
    task: TaskDto,
    run: RunDto,
    claim_token: String,
    claim_expires_at: Option<i64>,
}

#[derive(Debug, Serialize)]
struct DependenciesDto {
    parents: Vec<TaskDto>,
    children: Vec<TaskDto>,
}

#[derive(Debug, Serialize)]
struct SearchTaskHitDto {
    task_id: String,
    seq: i64,
    score: f64,
    snippet: Option<String>,
    task: TaskDto,
}

#[derive(Debug, Serialize)]
struct SearchTasksDto {
    hits: Vec<SearchTaskHitDto>,
    meta: kanban_search::SearchMeta,
}

#[derive(Debug, Deserialize)]
struct ContextBuildQuery {
    #[serde(default = "default_board")]
    board: String,
    #[serde(default = "default_context_lexical_limit")]
    lexical_limit: usize,
    #[serde(default = "default_context_graph_limit")]
    graph_limit: usize,
    #[serde(default = "default_context_vector_limit")]
    vector_limit: usize,
    #[serde(default = "default_context_max_items")]
    max_items: usize,
}

#[derive(Debug, Deserialize)]
struct BoardQuery {
    #[serde(default = "default_board")]
    board: String,
}

#[derive(Debug, Deserialize)]
struct GraphNeighborsQuery {
    entity_uri: String,
    predicate: Option<String>,
    #[serde(default = "default_graph_limit")]
    limit: usize,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/boards", get(list_boards))
        .route("/api/v1/boards/:board", get(get_board))
        .route("/api/v1/boards/:board/columns", get(list_board_columns))
        .route(
            "/api/v1/boards/:board/tasks",
            get(list_tasks).post(create_task),
        )
        .route("/api/v1/tasks/:task_id", get(get_task).patch(update_task))
        .route(
            "/api/v1/tasks/:task_id/transitions/specify",
            post(specify_task),
        )
        .route(
            "/api/v1/tasks/:task_id/transitions/promote",
            post(promote_task),
        )
        .route("/api/v1/tasks/:task_id/transitions/claim", post(claim_task))
        .route(
            "/api/v1/tasks/:task_id/transitions/reclaim",
            post(reclaim_task),
        )
        .route(
            "/api/v1/tasks/:task_id/transitions/heartbeat",
            post(heartbeat_task),
        )
        .route(
            "/api/v1/tasks/:task_id/transitions/complete",
            post(complete_task),
        )
        .route(
            "/api/v1/tasks/:task_id/transitions/submit-review",
            post(submit_review_task),
        )
        .route("/api/v1/tasks/:task_id/transitions/block", post(block_task))
        .route(
            "/api/v1/tasks/:task_id/transitions/unblock",
            post(unblock_task),
        )
        .route(
            "/api/v1/tasks/:task_id/transitions/archive",
            post(archive_task),
        )
        .route(
            "/api/v1/tasks/:task_id/dependencies",
            get(list_dependencies).post(add_dependency),
        )
        .route(
            "/api/v1/tasks/:child_task_id/dependencies/:parent_task_id",
            delete(remove_dependency),
        )
        .route("/api/v1/tasks/:task_id/runs", get(list_runs))
        .route("/api/v1/runs/:run_id", get(get_run))
        .route("/api/v1/runs/:run_id/log", get(get_run_log))
        .route(
            "/api/v1/tasks/:task_id/comments",
            get(list_comments).post(create_comment),
        )
        .route("/api/v1/stats", get(get_stats))
        .route("/api/v1/search/tasks", get(search_tasks))
        .route("/api/v1/search/status", get(search_status))
        .route("/api/v1/tasks/:task_id/context", get(build_context))
        .route("/api/v1/graph/status", get(graph_status))
        .route("/api/v1/graph/neighbors", get(graph_neighbors))
        .route("/api/v1/vector/status", get(vector_status))
        .route("/api/v1/events", get(list_events))
        .route("/api/v1/stream/events", get(stream_events))
        .route("/api/v1/maintenance/doctor", post(doctor))
        .route("/api/v1/maintenance/checkpoint", post(checkpoint))
        .with_state(state)
}

pub fn build_desktop_router(state: AppState) -> Router {
    build_router(state).layer(desktop_cors_layer())
}

fn desktop_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            HeaderValue::from_static("http://127.0.0.1:1420"),
            HeaderValue::from_static("http://localhost:1420"),
            HeaderValue::from_static("http://tauri.localhost"),
            HeaderValue::from_static("https://tauri.localhost"),
            HeaderValue::from_static("tauri://localhost"),
        ]))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::HeaderName::from_static("x-kb-actor"),
        ])
}

pub async fn serve(addr: SocketAddr, state: AppState) -> std::io::Result<()> {
    serve_with_search_sync(addr, state, SearchSyncConfig::disabled("default")).await
}

pub async fn serve_with_search_sync(
    addr: SocketAddr,
    state: AppState,
    search_sync: SearchSyncConfig,
) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let _search_sync_task = spawn_search_sync_task(state.clone(), search_sync);
    axum::serve(listener, build_desktop_router(state)).await
}

async fn health(
    State(state): State<AppState>,
) -> Result<Json<Envelope<serde_json::Value>>, ApiError> {
    let _conn = kanban_sqlite::connect_file(state.db_path())?;
    Ok(Json(Envelope {
        data: json!({
            "ok": true,
            "db": "ok",
            "version": env!("CARGO_PKG_VERSION"),
        }),
        meta: None,
    }))
}

async fn list_boards(
    State(state): State<AppState>,
) -> Result<Json<Envelope<Vec<kanban_sqlite::BoardRecord>>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::list_boards(state.db_path())?,
        meta: None,
    }))
}

async fn get_board(
    State(state): State<AppState>,
    Path(board): Path<String>,
) -> Result<Json<Envelope<kanban_sqlite::BoardRecord>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::get_board(state.db_path(), &board)?,
        meta: None,
    }))
}

async fn list_board_columns(
    State(state): State<AppState>,
    Path(board): Path<String>,
) -> Result<Json<Envelope<Vec<kanban_sqlite::BoardColumnRecord>>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::list_board_columns(state.db_path(), &board)?,
        meta: None,
    }))
}

#[derive(Debug, Deserialize)]
struct TaskListQuery {
    #[serde(default)]
    include_archived: bool,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
    #[serde(default)]
    assignee: Option<String>,
    q: Option<String>,
    search: Option<String>,
    sort: Option<String>,
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchTasksQuery {
    #[serde(default = "default_board")]
    board: String,
    q: Option<String>,
    #[serde(default)]
    include_archived: bool,
    #[serde(default = "default_search_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
    assignee: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchStatusQuery {
    #[serde(default = "default_board")]
    board: String,
}

fn default_limit() -> usize {
    100
}

fn default_search_limit() -> usize {
    20
}

fn default_context_lexical_limit() -> usize {
    5
}

fn default_context_graph_limit() -> usize {
    10
}

fn default_context_vector_limit() -> usize {
    5
}

fn default_context_max_items() -> usize {
    20
}

fn default_graph_limit() -> usize {
    50
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateTaskBody {
    title: String,
    description: Option<String>,
    status: Option<TaskStatus>,
    assignee: Option<String>,
    #[serde(default)]
    priority: i64,
    scheduled_at: Option<i64>,
    due_at: Option<i64>,
    max_retries: Option<i64>,
    metadata: Option<serde_json::Value>,
    #[serde(default)]
    depends_on: Vec<String>,
    actor: Option<String>,
}

async fn list_tasks(
    State(state): State<AppState>,
    Path(board): Path<String>,
    RawQuery(raw_query): RawQuery,
    query: Result<Query<TaskListQuery>, QueryRejection>,
) -> Result<Json<Envelope<Vec<TaskDto>>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(
        query.limit,
        kanban_sqlite::MAX_TASK_LIST_LIMIT,
        query.offset,
    )?;
    if query
        .label
        .as_deref()
        .is_some_and(|label| !label.trim().is_empty())
    {
        return Err(invalid_input("label filter is not supported yet"));
    }
    let statuses = parse_status_filters(raw_query.as_deref())?;
    let assignee = query
        .assignee
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let search = query
        .q
        .as_deref()
        .or(query.search.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let sort = parse_task_sort(query.sort.as_deref())?;
    let page = kanban_sqlite::list_tasks_page(
        state.db_path(),
        &board,
        kanban_sqlite::TaskListOptions {
            statuses,
            include_archived: query.include_archived,
            assignee,
            search,
            sort,
            limit: query.limit,
            offset: query.offset,
        },
    )?;
    let tasks = page.tasks.into_iter().map(TaskDto::from).collect();
    Ok(Json(Envelope {
        data: tasks,
        meta: Some(json!({ "limit": query.limit, "offset": query.offset, "total": page.total })),
    }))
}

async fn search_tasks(
    State(state): State<AppState>,
    RawQuery(raw_query): RawQuery,
    query: Result<Query<SearchTasksQuery>, QueryRejection>,
) -> Result<Json<Envelope<SearchTasksDto>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(query.limit, kanban_sqlite::MAX_SEARCH_LIMIT, query.offset)?;
    let statuses = parse_status_filters(raw_query.as_deref())?;
    let q = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let assignee = query
        .assignee
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let results = kanban_sqlite::search_tasks(
        state.db_path(),
        SearchQuery {
            board: query.board,
            q,
            statuses,
            assignee,
            include_archived: query.include_archived,
            limit: query.limit,
            offset: query.offset,
        },
    )?;
    let hits = results
        .hits
        .into_iter()
        .map(|hit| {
            let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &hit.task_id)?;
            Ok(SearchTaskHitDto {
                task_id: hit.task_id,
                seq: hit.seq,
                score: hit.score,
                snippet: hit.snippet,
                task: TaskDto::from(task),
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(Json(Envelope {
        data: SearchTasksDto {
            hits,
            meta: results.meta,
        },
        meta: Some(json!({ "limit": query.limit, "offset": query.offset })),
    }))
}

async fn search_status(
    State(state): State<AppState>,
    query: Result<Query<SearchStatusQuery>, QueryRejection>,
) -> Result<Json<Envelope<kanban_search::SearchIndexStatus>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    Ok(Json(Envelope {
        data: kanban_sqlite::search_index_status(state.db_path(), &query.board)?,
        meta: None,
    }))
}

async fn build_context(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    query: Result<Query<ContextBuildQuery>, QueryRejection>,
) -> Result<Json<Envelope<kanban_context::ContextPack>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(query.lexical_limit, kanban_sqlite::MAX_SEARCH_LIMIT, 0)?;
    validate_page_bounds(query.graph_limit, kanban_sqlite::MAX_TASK_LIST_LIMIT, 0)?;
    validate_page_bounds(query.vector_limit, kanban_sqlite::MAX_TASK_LIST_LIMIT, 0)?;
    validate_page_bounds(query.max_items, kanban_sqlite::MAX_TASK_LIST_LIMIT, 0)?;
    let policy = kanban_context::ContextPolicy {
        lexical_limit: query.lexical_limit,
        graph_limit: query.graph_limit,
        vector_limit: query.vector_limit,
        max_items: query.max_items,
    };
    Ok(Json(Envelope {
        data: kanban_sqlite::build_context_pack(state.db_path(), &query.board, &task_id, policy)?,
        meta: None,
    }))
}

async fn graph_status(
    State(state): State<AppState>,
    query: Result<Query<BoardQuery>, QueryRejection>,
) -> Result<Json<Envelope<kanban_graph::GraphStoreStatus>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    Ok(Json(Envelope {
        data: kanban_sqlite::graph_store_status(state.db_path(), &query.board)?,
        meta: None,
    }))
}

async fn graph_neighbors(
    State(state): State<AppState>,
    query: Result<Query<GraphNeighborsQuery>, QueryRejection>,
) -> Result<Json<Envelope<Vec<kanban_entity::Relation>>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(query.limit, kanban_sqlite::MAX_TASK_LIST_LIMIT, 0)?;
    let entity_uri =
        EntityUri::new(query.entity_uri).map_err(|error| invalid_input(error.to_string()))?;
    let predicate = query
        .predicate
        .as_deref()
        .map(parse_predicate)
        .transpose()?;
    Ok(Json(Envelope {
        data: kanban_sqlite::graph_neighbors(state.db_path(), &entity_uri, predicate, query.limit)?,
        meta: Some(json!({ "limit": query.limit })),
    }))
}

async fn vector_status(
    State(state): State<AppState>,
    query: Result<Query<BoardQuery>, QueryRejection>,
) -> Result<Json<Envelope<kanban_vector::VectorStoreStatus>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    Ok(Json(Envelope {
        data: kanban_sqlite::vector_store_status(state.db_path(), &query.board)?,
        meta: None,
    }))
}

async fn create_task(
    State(state): State<AppState>,
    Path(board): Path<String>,
    headers: HeaderMap,
    body: Result<Json<CreateTaskBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Envelope<TaskDto>>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let input = kanban_sqlite::CreateTask {
        title: body.title,
        description: body.description,
        status: body.status,
        assignee: body.assignee,
        priority: body.priority,
        scheduled_at: body.scheduled_at,
        due_at: body.due_at,
        metadata_json: metadata_json(body.metadata)?,
    };
    let mut task = kanban_sqlite::create_task_with_dependencies(
        state.db_path(),
        &board,
        &actor,
        input,
        &body.depends_on,
    )?;
    if body.max_retries.is_some() {
        task = kanban_sqlite::set_task_retry_policy_by_id(
            state.db_path(),
            &actor,
            &task.id,
            body.max_retries,
        )?;
    }
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: TaskDto::from(task),
            meta: None,
        }),
    ))
}

async fn get_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::get_task_by_id_global(
            state.db_path(),
            &task_id,
        )?),
        meta: None,
    }))
}

async fn update_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<serde_json::Value>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let object = body
        .as_object()
        .ok_or_else(|| invalid_input("request body must be a JSON object"))?;
    for forbidden in ["status", "claim_token", "current_run_id", "completed_at"] {
        if object.contains_key(forbidden) {
            return Err(invalid_input(format!("{forbidden} cannot be patched")));
        }
    }
    let body_actor = object.get("actor").and_then(|value| value.as_str());
    let actor = actor(body_actor, &headers, &state);
    let retry_policy = retry_policy_from_value(object)?;
    let patch = patch_from_value(object)?;
    let mut task = kanban_sqlite::update_task_by_id(state.db_path(), &actor, &task_id, patch)?;
    if let Some(max_retries) = retry_policy {
        task = kanban_sqlite::set_task_retry_policy_by_id(
            state.db_path(),
            &actor,
            &task_id,
            max_retries,
        )?;
    }
    Ok(Json(Envelope {
        data: TaskDto::from(task),
        meta: None,
    }))
}

#[derive(Debug, Deserialize, Default)]
struct ActorBody {
    actor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommentBody {
    author: Option<String>,
    body: String,
    kind: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimBody {
    actor: Option<String>,
    #[serde(default = "default_claim_ttl_ms")]
    ttl_ms: i64,
    worker_profile: Option<String>,
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct SpecifyBody {
    actor: Option<String>,
    description: Option<String>,
    scheduled_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HeartbeatBody {
    actor: Option<String>,
    claim_token: String,
    #[serde(default = "default_claim_ttl_ms")]
    ttl_ms: i64,
    note: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct TokenBody {
    actor: Option<String>,
    claim_token: Option<String>,
    #[serde(default)]
    force: bool,
    summary: Option<String>,
    result: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct ReclaimBody {
    actor: Option<String>,
    #[serde(default)]
    force: bool,
    to_status: Option<TaskStatus>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BlockBody {
    actor: Option<String>,
    reason: String,
    claim_token: Option<String>,
    #[serde(default)]
    force: bool,
}

#[derive(Debug, Deserialize, Default)]
struct ArchiveBody {
    actor: Option<String>,
    #[serde(default)]
    force: bool,
}

#[derive(Debug, Deserialize)]
struct AddDependencyBody {
    parent_task_id: String,
    actor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EventsQuery {
    #[serde(default = "default_board")]
    board: String,
    task_id: Option<String>,
    #[serde(default)]
    after: i64,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize)]
struct StatsQuery {
    #[serde(default = "default_board")]
    board: String,
}

fn default_claim_ttl_ms() -> i64 {
    300_000
}

fn default_board() -> String {
    "default".to_owned()
}

async fn specify_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<SpecifyBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::specify_task(
            state.db_path(),
            &actor,
            &task_id,
            body.description,
            body.scheduled_at,
        )?),
        meta: None,
    }))
}

async fn promote_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<ActorBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let body = optional_json_body(body)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::promote_task(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
        )?),
        meta: None,
    }))
}

async fn claim_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<ClaimBody>, JsonRejection>,
) -> Result<Json<Envelope<ClaimDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    if body.ttl_ms <= 0 {
        return Err(invalid_input("ttl_ms must be positive"));
    }
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    let worker_profile = body.worker_profile.as_deref().unwrap_or("manual");
    let metadata_json = metadata_json(body.metadata)?;
    let claim = kanban_sqlite::claim_task_with_profile_and_metadata(
        state.db_path(),
        &task.board_id,
        &actor,
        &task_id,
        body.ttl_ms,
        worker_profile,
        &metadata_json,
    )?;
    let run = kanban_sqlite::list_runs(state.db_path(), &task.board_id, Some(&task_id))?
        .into_iter()
        .find(|run| run.id == claim.run_id)
        .ok_or_else(|| KanbanError::NotFound(format!("run {}", claim.run_id)))?;
    Ok(Json(Envelope {
        data: ClaimDto {
            claim_token: claim.claim_token,
            claim_expires_at: claim.task.claim_expires_at,
            task: TaskDto::from(claim.task),
            run: RunDto::from(run),
        },
        meta: None,
    }))
}

async fn reclaim_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<ReclaimBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let body = optional_json_body(body)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::reclaim_task_to(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            body.force,
            body.to_status.unwrap_or(TaskStatus::Ready),
            body.reason.as_deref(),
        )?),
        meta: None,
    }))
}

async fn heartbeat_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<HeartbeatBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    if body.ttl_ms <= 0 {
        return Err(invalid_input("ttl_ms must be positive"));
    }
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    if task.status == TaskStatus::Running
        && task.claim_token.as_deref() != Some(body.claim_token.as_str())
    {
        return Err(ApiError(KanbanError::InvalidTransition(
            "claim token mismatch".to_owned(),
        )));
    }
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::heartbeat_task_with_note(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            &body.claim_token,
            body.ttl_ms,
            body.note.as_deref(),
        )?),
        meta: None,
    }))
}

async fn complete_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<TokenBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    let result_json = body.result.map(|value| value.to_string());
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::complete_task_with_summary_and_result(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            body.claim_token.as_deref(),
            body.force,
            body.summary.as_deref(),
            result_json.as_deref(),
        )?),
        meta: None,
    }))
}

async fn submit_review_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<TokenBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    if body.result.is_some() {
        return Err(invalid_input("submit-review result is not supported yet"));
    }
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::submit_review_task_with_summary(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            body.claim_token.as_deref(),
            body.force,
            body.summary.as_deref(),
        )?),
        meta: None,
    }))
}

async fn block_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<BlockBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::block_task(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            &body.reason,
            body.claim_token.as_deref(),
            body.force,
        )?),
        meta: None,
    }))
}

async fn unblock_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<ActorBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let body = optional_json_body(body)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::unblock_task(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
        )?),
        meta: None,
    }))
}

async fn archive_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<ArchiveBody>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let body = optional_json_body(body)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(Envelope {
        data: TaskDto::from(kanban_sqlite::archive_task(
            state.db_path(),
            &task.board_id,
            &actor,
            &task_id,
            body.force,
        )?),
        meta: None,
    }))
}

async fn add_dependency(
    State(state): State<AppState>,
    Path(child_task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<AddDependencyBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Envelope<DependenciesDto>>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let child = kanban_sqlite::get_task_by_id_global(state.db_path(), &child_task_id)?;
    kanban_sqlite::add_dependency(
        state.db_path(),
        &child.board_id,
        &actor,
        &body.parent_task_id,
        &child_task_id,
    )?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: dependencies_dto(&state, &child_task_id)?,
            meta: None,
        }),
    ))
}

async fn remove_dependency(
    State(state): State<AppState>,
    Path((child_task_id, parent_task_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<Envelope<DependenciesDto>>, ApiError> {
    let actor = actor(None, &headers, &state);
    let child = kanban_sqlite::get_task_by_id_global(state.db_path(), &child_task_id)?;
    kanban_sqlite::remove_dependency(
        state.db_path(),
        &child.board_id,
        &actor,
        &parent_task_id,
        &child_task_id,
    )?;
    Ok(Json(Envelope {
        data: dependencies_dto(&state, &child_task_id)?,
        meta: None,
    }))
}

async fn list_dependencies(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Envelope<DependenciesDto>>, ApiError> {
    Ok(Json(Envelope {
        data: dependencies_dto(&state, &task_id)?,
        meta: None,
    }))
}

async fn list_runs(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Envelope<Vec<RunDto>>>, ApiError> {
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(Envelope {
        data: kanban_sqlite::list_runs(state.db_path(), &task.board_id, Some(&task_id))?
            .into_iter()
            .map(RunDto::from)
            .collect(),
        meta: None,
    }))
}

async fn get_run(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Result<Json<Envelope<RunDto>>, ApiError> {
    Ok(Json(Envelope {
        data: RunDto::from(kanban_sqlite::get_run_by_id_global(
            state.db_path(),
            &run_id,
        )?),
        meta: None,
    }))
}

async fn get_run_log(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Result<Json<Envelope<RunLogDto>>, ApiError> {
    const MAX_RUN_LOG_BYTES: usize = 256 * 1024;
    let run = kanban_sqlite::get_run_by_id_global(state.db_path(), &run_id)?;
    let log_path = run
        .log_path
        .as_deref()
        .ok_or_else(|| KanbanError::NotFound(format!("run log {run_id}")))?;
    let bytes = fs::read(log_path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => KanbanError::NotFound(format!("run log {run_id}")),
        _ => KanbanError::Storage(error.to_string()),
    })?;
    let truncated = bytes.len() > MAX_RUN_LOG_BYTES;
    let bytes = if truncated {
        &bytes[..MAX_RUN_LOG_BYTES]
    } else {
        &bytes[..]
    };
    let content = String::from_utf8_lossy(bytes).into_owned();
    Ok(Json(Envelope {
        data: RunLogDto {
            run_id,
            content,
            truncated,
        },
        meta: None,
    }))
}

async fn list_comments(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Envelope<Vec<CommentDto>>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::list_comments(state.db_path(), &task_id)?
            .into_iter()
            .map(CommentDto::from)
            .collect(),
        meta: None,
    }))
}

async fn create_comment(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<CommentBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Envelope<CommentDto>>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.author.as_deref(), &headers, &state);
    let comment = kanban_sqlite::create_comment(
        state.db_path(),
        &task_id,
        &actor,
        &body.body,
        body.kind.as_deref(),
    )?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: CommentDto::from(comment),
            meta: None,
        }),
    ))
}

async fn get_stats(
    State(state): State<AppState>,
    query: Result<Query<StatsQuery>, QueryRejection>,
) -> Result<Json<Envelope<kanban_sqlite::QueueStats>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    Ok(Json(Envelope {
        data: kanban_sqlite::queue_stats(state.db_path(), &query.board)?,
        meta: None,
    }))
}

async fn list_events(
    State(state): State<AppState>,
    query: Result<Query<EventsQuery>, QueryRejection>,
) -> Result<Json<Envelope<Vec<EventDto>>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let (data, next_after) = events_snapshot(&state, &query)?;
    Ok(Json(Envelope {
        data,
        meta: Some(json!({ "next_after": next_after })),
    }))
}

async fn stream_events(
    State(state): State<AppState>,
    query: Result<Query<EventsQuery>, QueryRejection>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let (data, _next_after) = events_snapshot(&state, &query)?;
    let mut frames = Vec::with_capacity(data.len());
    for event in data {
        let frame = Event::default()
            .event(event.kind.clone())
            .id(event.id.to_string())
            .data(serde_json::to_string(&event).map_err(|error| invalid_input(error.to_string()))?);
        frames.push(Ok::<_, Infallible>(frame));
    }
    Ok(Sse::new(stream::iter(frames)))
}

async fn doctor(
    State(state): State<AppState>,
) -> Result<Json<Envelope<kanban_sqlite::DoctorReport>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::doctor_database(state.db_path())?,
        meta: None,
    }))
}

async fn checkpoint(
    State(state): State<AppState>,
) -> Result<Json<Envelope<kanban_sqlite::CheckpointResult>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::checkpoint_database(state.db_path())?,
        meta: None,
    }))
}

fn events_snapshot(
    state: &AppState,
    query: &EventsQuery,
) -> Result<(Vec<EventDto>, i64), ApiError> {
    let board = kanban_sqlite::get_board(state.db_path(), &query.board)?;
    let limit = query.limit.min(1000);
    let events = kanban_sqlite::list_events_after(
        state.db_path(),
        &query.board,
        kanban_sqlite::EventListOptions {
            task_ref: query.task_id.clone(),
            after: query.after,
            limit,
        },
    )?;
    let next_after = events.last().map_or(query.after, |event| event.id);
    let data = events
        .into_iter()
        .map(|event| EventDto {
            id: event.id,
            event_id: event.event_id,
            board_id: board.id.clone(),
            task_id: event.task_id,
            run_id: event.run_id,
            kind: event.kind,
            actor: event.actor,
            payload: serde_json::from_str(&event.payload_json).unwrap_or_else(|_| json!({})),
            created_at: event.created_at,
        })
        .collect();
    Ok((data, next_after))
}

fn dependencies_dto(state: &AppState, task_id: &str) -> Result<DependenciesDto, ApiError> {
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), task_id)?;
    let edges = kanban_sqlite::list_dependencies(state.db_path(), &task.board_id, task_id)?;
    let mut parents = Vec::new();
    let mut children = Vec::new();
    for (parent_id, child_id) in edges {
        if child_id == task_id {
            parents.push(TaskDto::from(kanban_sqlite::get_task_by_id_global(
                state.db_path(),
                &parent_id,
            )?));
        }
        if parent_id == task_id {
            children.push(TaskDto::from(kanban_sqlite::get_task_by_id_global(
                state.db_path(),
                &child_id,
            )?));
        }
    }
    Ok(DependenciesDto { parents, children })
}

fn optional_json_body<T: Default>(body: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    match body {
        Ok(Json(body)) => Ok(body),
        Err(JsonRejection::MissingJsonContentType(_)) => Ok(T::default()),
        Err(error) => Err(extractor_error(error)),
    }
}

fn parse_task_sort(sort: Option<&str>) -> Result<kanban_sqlite::TaskListSort, ApiError> {
    let sort = match sort.unwrap_or("position") {
        "position" => kanban_sqlite::TaskListSort::Position,
        "-position" => kanban_sqlite::TaskListSort::PositionDesc,
        "priority" => kanban_sqlite::TaskListSort::Priority,
        "-priority" => kanban_sqlite::TaskListSort::PriorityDesc,
        "created_at" => kanban_sqlite::TaskListSort::CreatedAt,
        "-created_at" => kanban_sqlite::TaskListSort::CreatedAtDesc,
        "updated_at" => kanban_sqlite::TaskListSort::UpdatedAt,
        "-updated_at" => kanban_sqlite::TaskListSort::UpdatedAtDesc,
        "due_at" => kanban_sqlite::TaskListSort::DueAt,
        "-due_at" => kanban_sqlite::TaskListSort::DueAtDesc,
        value => return Err(invalid_input(format!("unsupported sort: {value}"))),
    };
    Ok(sort)
}

fn parse_status_filters(raw_query: Option<&str>) -> Result<Vec<TaskStatus>, ApiError> {
    let Some(raw_query) = raw_query else {
        return Ok(Vec::new());
    };
    let pairs = serde_urlencoded::from_str::<Vec<(String, String)>>(raw_query)
        .map_err(|error| invalid_input(error.to_string()))?;
    pairs
        .into_iter()
        .filter_map(|(key, value)| (key == "status").then_some(value))
        .map(|value| TaskStatus::from_str(value.trim()).map_err(ApiError::from))
        .collect()
}

fn parse_predicate(value: &str) -> Result<Predicate, ApiError> {
    match value.trim() {
        "belongs_to_board" => Ok(Predicate::BelongsToBoard),
        "belongs_to_task" => Ok(Predicate::BelongsToTask),
        "depends_on" => Ok(Predicate::DependsOn),
        "produced_by" => Ok(Predicate::ProducedBy),
        "generated_by" => Ok(Predicate::GeneratedBy),
        "references_artifact" => Ok(Predicate::ReferencesArtifact),
        "related_to" => Ok(Predicate::RelatedTo),
        "uses_skill" => Ok(Predicate::UsesSkill),
        "uses_context" => Ok(Predicate::UsesContext),
        "derived_from" => Ok(Predicate::DerivedFrom),
        "supersedes" => Ok(Predicate::Supersedes),
        "similar_to" => Ok(Predicate::SimilarTo),
        "requires_review" => Ok(Predicate::RequiresReview),
        "waiting_for_user" => Ok(Predicate::WaitingForUser),
        other => Err(invalid_input(format!("unsupported predicate: {other}"))),
    }
}

fn patch_from_value(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<kanban_sqlite::TaskPatch, ApiError> {
    const ALLOWED: &[&str] = &[
        "title",
        "description",
        "assignee",
        "priority",
        "scheduled_at",
        "due_at",
        "metadata_json",
        "metadata",
        "max_retries",
        "expected_lock_version",
        "actor",
    ];
    for key in object.keys() {
        if !ALLOWED.contains(&key.as_str()) {
            return Err(invalid_input(format!("unknown patch field: {key}")));
        }
    }

    let mut patch = kanban_sqlite::TaskPatch::default();
    if let Some(value) = object.get("title") {
        patch.title = Some(string_field(value, "title")?);
    }
    if object.contains_key("description") {
        patch.description = Some(optional_string_field(
            object.get("description"),
            "description",
        )?);
    }
    if object.contains_key("assignee") {
        patch.assignee = Some(optional_string_field(object.get("assignee"), "assignee")?);
    }
    if let Some(value) = object.get("priority") {
        patch.priority = Some(
            value
                .as_i64()
                .ok_or_else(|| invalid_input("priority must be an integer"))?,
        );
    }
    if object.contains_key("scheduled_at") {
        patch.scheduled_at = Some(optional_i64_field(
            object.get("scheduled_at"),
            "scheduled_at",
        )?);
    }
    if object.contains_key("due_at") {
        patch.due_at = Some(optional_i64_field(object.get("due_at"), "due_at")?);
    }
    if let Some(value) = object.get("metadata") {
        patch.metadata_json = Some(value.to_string());
    }
    if let Some(value) = object.get("metadata_json") {
        patch.metadata_json = Some(string_field(value, "metadata_json")?);
    }
    if let Some(value) = object.get("expected_lock_version") {
        patch.expected_lock_version = Some(
            value
                .as_i64()
                .ok_or_else(|| invalid_input("expected_lock_version must be an integer"))?,
        );
    }
    Ok(patch)
}

fn retry_policy_from_value(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<Option<i64>>, ApiError> {
    if !object.contains_key("max_retries") {
        return Ok(None);
    }
    optional_i64_field(object.get("max_retries"), "max_retries").map(Some)
}

fn actor(body_actor: Option<&str>, headers: &HeaderMap, state: &AppState) -> String {
    body_actor
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| {
            headers
                .get("X-KB-Actor")
                .and_then(|value| value.to_str().ok())
                .filter(|value| !value.trim().is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| state.default_actor().to_owned())
}

fn metadata_json(value: Option<serde_json::Value>) -> Result<String, ApiError> {
    Ok(value.unwrap_or_else(|| json!({})).to_string())
}

fn string_field(value: &serde_json::Value, field: &str) -> Result<String, ApiError> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid_input(format!("{field} must be a string")))
}

fn optional_string_field(
    value: Option<&serde_json::Value>,
    field: &str,
) -> Result<Option<String>, ApiError> {
    match value {
        Some(serde_json::Value::Null) => Ok(None),
        Some(value) => string_field(value, field).map(Some),
        None => Ok(None),
    }
}

fn optional_i64_field(
    value: Option<&serde_json::Value>,
    field: &str,
) -> Result<Option<i64>, ApiError> {
    match value {
        Some(serde_json::Value::Null) => Ok(None),
        Some(value) => value
            .as_i64()
            .map(Some)
            .ok_or_else(|| invalid_input(format!("{field} must be an integer epoch ms"))),
        None => Ok(None),
    }
}

fn extractor_error(error: impl std::fmt::Display) -> ApiError {
    invalid_input(error.to_string())
}

fn invalid_input(message: impl Into<String>) -> ApiError {
    ApiError(KanbanError::InvalidInput(message.into()))
}

fn validate_page_bounds(limit: usize, max_limit: usize, offset: usize) -> Result<(), ApiError> {
    if limit > max_limit {
        return Err(invalid_input(format!("limit must be <= {max_limit}")));
    }
    if offset > i64::MAX as usize {
        return Err(invalid_input(format!("offset must be <= {}", i64::MAX)));
    }
    Ok(())
}

#[derive(Debug)]
struct ApiError(KanbanError);

impl From<KanbanError> for ApiError {
    fn from(value: KanbanError) -> Self {
        Self(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let message = self.0.to_string();
        let (status, code) = match self.0 {
            KanbanError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            KanbanError::InvalidInput(_) if message.contains("dependency cycle") => {
                (StatusCode::CONFLICT, "dependency_cycle")
            }
            KanbanError::InvalidInput(_) | KanbanError::InvalidStatus(_) => {
                (StatusCode::BAD_REQUEST, "invalid_input")
            }
            KanbanError::InvalidTransition(_) if message.contains("claim token mismatch") => {
                (StatusCode::FORBIDDEN, "claim_token_mismatch")
            }
            KanbanError::InvalidTransition(_) if message.contains("dependency blocked") => {
                (StatusCode::CONFLICT, "dependency_blocked")
            }
            KanbanError::InvalidTransition(_) if message.contains("claim conflict") => {
                (StatusCode::CONFLICT, "claim_conflict")
            }
            KanbanError::InvalidTransition(_) => (StatusCode::CONFLICT, "invalid_transition"),
            KanbanError::Storage(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        (
            status,
            Json(ErrorEnvelope {
                error: ErrorBody { code, message },
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{SearchSyncConfig, search_sync_task_enabled};

    #[test]
    fn search_sync_zero_interval_is_disabled() {
        let config = SearchSyncConfig::disabled("default");

        assert_eq!(config.board(), "default");
        assert_eq!(config.interval(), Duration::ZERO);
        assert!(!search_sync_task_enabled(&config));
    }

    #[cfg(not(feature = "tantivy-backend"))]
    #[test]
    fn search_sync_is_disabled_without_tantivy_backend() {
        let config = SearchSyncConfig::new("default", Duration::from_millis(5_000));

        assert!(!search_sync_task_enabled(&config));
    }

    #[cfg(feature = "tantivy-backend")]
    #[test]
    fn search_sync_positive_interval_is_enabled_with_tantivy_backend() {
        let config = SearchSyncConfig::new("default", Duration::from_millis(5_000));

        assert!(search_sync_task_enabled(&config));
    }
}
