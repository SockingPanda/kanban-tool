use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::Duration,
};

use kanban_context::{
    ContextBrokerInput, ContextDiagnostic, ContextError, ContextItem, ContextPack, ContextPolicy,
};
use kanban_core::{
    Clock, KanbanError, Result, SystemClock, TaskStatus, new_event_id, new_run_id, new_task_id,
    new_typed_id,
};
use kanban_entity::{EntityUri, Predicate, Provenance, Relation};
use kanban_graph::GraphStoreStatus;
#[cfg(feature = "graph-oxigraph")]
use kanban_graph::{OxigraphStore, RelationGraph};
use kanban_indexer::LANCEDB_CHUNKS_STORE;
#[cfg(feature = "graph-oxigraph")]
use kanban_indexer::OXIGRAPH_RELATIONS_STORE;
#[cfg(feature = "tantivy-backend")]
use kanban_indexer::TANTIVY_TASKS_STORE;
use kanban_indexer::{
    DERIVED_STORE_SEEDS, DerivedStoreUpdate, OUTBOX_FANOUT_TARGETS, OutboxTarget,
    derived_store_for_name,
};
#[cfg(feature = "tantivy-backend")]
use kanban_search::TaskSearchDocument;
use kanban_search::{SearchHit, SearchIndexStatus, SearchMeta, SearchQuery, SearchResults};
use kanban_vector::{ChunkBuilder, TaskChunkSource, VectorStore, VectorStoreStatus};
#[cfg(feature = "vector-lancedb")]
use kanban_vector::{LanceDbConfig, LanceDbStore};
#[cfg(feature = "vector-lancedb")]
use kanban_vector::{VectorHit, VectorQuery};
use rusqlite::{
    Connection, OptionalExtension, Row, params, params_from_iter,
    types::{Value, ValueRef},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    connect_file, default_pragmas, maintenance_lock_blocks, maintenance_lock_path,
    runtime_lock_blocks, runtime_lock_path,
};

/// Maximum task-list page size accepted by CLI, API, and SQLite service calls.
pub const MAX_TASK_LIST_LIMIT: usize = 1000;
/// Maximum search page size accepted by CLI, API, and SQLite service calls.
pub const MAX_SEARCH_LIMIT: usize = 1000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub board_id: String,
    pub seq: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub status_reason: Option<String>,
    pub assignee: Option<String>,
    pub priority: i64,
    pub position: i64,
    pub scheduled_at: Option<i64>,
    pub due_at: Option<i64>,
    pub created_by: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub archived_at: Option<i64>,
    pub claim_token: Option<String>,
    pub claim_owner: Option<String>,
    pub claim_expires_at: Option<i64>,
    pub last_heartbeat_at: Option<i64>,
    pub current_run_id: Option<String>,
    pub retry_count: i64,
    pub max_retries: Option<i64>,
    pub result_summary: Option<String>,
    pub result_json: Option<String>,
    pub metadata_json: String,
    pub lock_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: i64,
    pub event_id: String,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub kind: String,
    pub actor: Option<String>,
    pub payload_json: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: String,
    pub task_id: String,
    pub status: String,
    pub worker_profile: Option<String>,
    pub worker_pid: Option<i64>,
    pub claim_token: String,
    pub claim_owner: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub exit_code: Option<i64>,
    pub summary: Option<String>,
    pub error: Option<String>,
    pub log_path: Option<String>,
    pub metadata_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardRecord {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardColumnRecord {
    pub id: String,
    pub board_id: String,
    pub status: TaskStatus,
    pub title: String,
    pub position: i64,
    pub hidden: bool,
    pub wip_limit: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentRecord {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub author: String,
    pub body: String,
    pub kind: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTask {
    pub title: String,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub assignee: Option<String>,
    pub priority: i64,
    pub scheduled_at: Option<i64>,
    pub due_at: Option<i64>,
    pub metadata_json: String,
}

impl CreateTask {
    pub fn ready(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: Some("ready spec".to_owned()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub assignee: Option<Option<String>>,
    pub priority: Option<i64>,
    pub scheduled_at: Option<Option<i64>>,
    pub due_at: Option<Option<i64>>,
    pub metadata_json: Option<String>,
    pub expected_lock_version: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimResult {
    pub task: TaskRecord,
    pub claim_token: String,
    pub run_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishPolicy {
    Done,
    Review,
    Blocked,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchOptions {
    pub actor: String,
    pub command: String,
    pub worker_profile: String,
    pub claim_ttl_ms: i64,
    pub heartbeat_interval_ms: i64,
    pub on_success: FinishPolicy,
    pub on_failure: FinishPolicy,
    pub log_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchResult {
    pub claimed: usize,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskListSort {
    Position,
    PositionDesc,
    Priority,
    PriorityDesc,
    CreatedAt,
    CreatedAtDesc,
    UpdatedAt,
    UpdatedAtDesc,
    DueAt,
    DueAtDesc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListOptions {
    pub statuses: Vec<TaskStatus>,
    pub include_archived: bool,
    pub assignee: Option<String>,
    pub search: Option<String>,
    pub sort: TaskListSort,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListPage {
    pub tasks: Vec<TaskRecord>,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventListOptions {
    pub task_ref: Option<String>,
    pub after: i64,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityListOptions {
    pub kind: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxListOptions {
    pub status: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRecord {
    pub uri: String,
    pub kind: String,
    pub source_table: String,
    pub source_id: String,
    pub board_id: Option<String>,
    pub task_id: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub content_hash: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexOutboxRecord {
    pub id: i64,
    pub source_event_id: Option<i64>,
    pub target: String,
    pub entity_uri: String,
    pub action: String,
    pub payload_json: String,
    pub status: String,
    pub attempts: i64,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedStoreStatusRecord {
    pub store_name: String,
    pub schema_version: i64,
    pub last_event_id: i64,
    pub dirty: bool,
    pub last_rebuild_at: Option<i64>,
    pub last_sync_at: Option<i64>,
    pub last_error: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorDerivedStoreReport {
    pub store_name: String,
    pub schema_version: i64,
    pub last_event_id: i64,
    pub dirty: bool,
    pub last_error: Option<String>,
    pub pending_outbox: i64,
    pub running_outbox: i64,
    pub failed_outbox: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub integrity_check: String,
    pub migration_version: Option<i64>,
    pub user_version: i64,
    pub expired_running_tasks: i64,
    pub running_tasks_without_active_run: i64,
    pub orphan_running_runs: i64,
    pub dependency_cycles: i64,
    pub archived_dependency_edges: i64,
    pub missing_run_logs: i64,
    pub suspicious_run_log_paths: i64,
    pub executable_dependency_violations: i64,
    pub executable_spec_violations: i64,
    pub executable_schedule_violations: i64,
    pub outbox_pending: i64,
    pub outbox_running: i64,
    pub outbox_failed: i64,
    pub derived_dirty_stores: i64,
    pub derived_error_stores: i64,
    pub derived_stores: Vec<DoctorDerivedStoreReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointResult {
    pub busy: i64,
    pub log_frames: i64,
    pub checkpointed_frames: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceResult {
    pub ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunLogPathStatus {
    Present(PathBuf),
    Missing(PathBuf),
    Suspicious { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupResult {
    pub out_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportResult {
    pub out_path: PathBuf,
    pub records: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportResult {
    pub input_path: PathBuf,
    pub records: usize,
}

#[derive(Debug)]
pub struct DatabaseReplaceGuard {
    lock_path: PathBuf,
}

impl Drop for DatabaseReplaceGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

#[derive(Debug)]
pub struct DatabaseRuntimeGuard {
    lock_path: PathBuf,
}

impl Drop for DatabaseRuntimeGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusCount {
    pub status: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaleClaimRecord {
    pub task_id: String,
    pub seq: i64,
    pub title: String,
    pub claim_owner: Option<String>,
    pub claim_expires_at: Option<i64>,
    pub last_heartbeat_at: Option<i64>,
    pub current_run_id: Option<String>,
    pub retry_count: i64,
    pub max_retries: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedReasonCount {
    pub reason: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueStats {
    pub board_id: String,
    pub generated_at: i64,
    pub status_counts: Vec<StatusCount>,
    pub stale_claims: Vec<StaleClaimRecord>,
    pub blocked_reasons: Vec<BlockedReasonCount>,
}

#[cfg(feature = "tantivy-backend")]
const SEARCH_TASKS_STATE_SCHEMA_VERSION: i64 = 1;
#[cfg(feature = "tantivy-backend")]
const SEARCH_TASKS_STATE_KEY_PREFIX: &str = "search.tasks.state";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchIndexState {
    pub schema_version: i64,
    pub index_version: String,
    pub backend: String,
    pub index_name: String,
    pub board_id: String,
    pub last_event_id: Option<i64>,
    pub dirty: bool,
    pub updated_at: i64,
    pub message: Option<String>,
}

pub fn list_boards(path: impl AsRef<Path>) -> Result<Vec<BoardRecord>> {
    let conn = connect_file(path.as_ref())?;
    let mut stmt = conn
        .prepare(
            "SELECT id,slug,name,description,created_at,updated_at,archived_at \
             FROM boards WHERE archived_at IS NULL ORDER BY created_at ASC, slug ASC",
        )
        .map_err(storage)?;
    let rows = stmt.query_map([], board_from_row).map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

pub fn list_entities(
    path: impl AsRef<Path>,
    options: EntityListOptions,
) -> Result<Vec<EntityRecord>> {
    validate_page_bounds(options.limit, MAX_TASK_LIST_LIMIT, 0)?;
    let conn = connect_file(path.as_ref())?;
    let mut params = Vec::new();
    let where_sql = if let Some(kind) = options
        .kind
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        params.push(Value::Text(kind.to_owned()));
        "WHERE kind=?1"
    } else {
        ""
    };
    params.push(Value::Integer(
        options.limit.try_into().expect("validated limit"),
    ));
    let sql = format!(
        "SELECT uri,kind,source_table,source_id,board_id,task_id,title,summary,content_hash,created_at,updated_at,archived_at \
         FROM entities {where_sql} ORDER BY updated_at DESC, uri ASC LIMIT ?"
    );
    let mut stmt = conn.prepare(&sql).map_err(storage)?;
    let rows = stmt
        .query_map(params_from_iter(params.iter()), entity_from_row)
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

pub fn get_entity(path: impl AsRef<Path>, uri: &str) -> Result<EntityRecord> {
    let conn = connect_file(path.as_ref())?;
    conn.query_row(
        "SELECT uri,kind,source_table,source_id,board_id,task_id,title,summary,content_hash,created_at,updated_at,archived_at \
         FROM entities WHERE uri=?1",
        [uri],
        entity_from_row,
    )
    .optional()
    .map_err(storage)?
    .ok_or_else(|| KanbanError::NotFound(format!("entity {uri}")))
}

pub fn list_outbox(
    path: impl AsRef<Path>,
    options: OutboxListOptions,
) -> Result<Vec<IndexOutboxRecord>> {
    validate_page_bounds(options.limit, MAX_TASK_LIST_LIMIT, 0)?;
    let conn = connect_file(path.as_ref())?;
    let mut params = Vec::new();
    let where_sql = if let Some(status) = options
        .status
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        params.push(Value::Text(status.to_owned()));
        "WHERE status=?1"
    } else {
        ""
    };
    params.push(Value::Integer(
        options.limit.try_into().expect("validated limit"),
    ));
    let sql = format!(
        "SELECT id,source_event_id,target,entity_uri,action,payload_json,status,attempts,last_error,created_at,updated_at \
         FROM index_outbox {where_sql} ORDER BY id ASC LIMIT ?"
    );
    let mut stmt = conn.prepare(&sql).map_err(storage)?;
    let rows = stmt
        .query_map(params_from_iter(params.iter()), outbox_from_row)
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

pub fn derived_store_statuses(path: impl AsRef<Path>) -> Result<Vec<DerivedStoreStatusRecord>> {
    let conn = connect_file(path.as_ref())?;
    derived_store_statuses_conn(&conn)
}

fn derived_store_statuses_conn(conn: &Connection) -> Result<Vec<DerivedStoreStatusRecord>> {
    let mut stmt = conn
        .prepare(
            "SELECT store_name,schema_version,last_event_id,dirty,last_rebuild_at,last_sync_at,last_error,updated_at \
             FROM derived_store_state ORDER BY store_name ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map([], derived_store_status_from_row)
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

pub fn graph_relation_snapshot(path: impl AsRef<Path>, board: &str) -> Result<Vec<Relation>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    graph_relation_snapshot_for_board(&conn, &board_id)
}

pub fn build_context_pack(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: &str,
    policy: ContextPolicy,
) -> Result<ContextPack> {
    build_context_pack_inner(path.as_ref(), board, task_ref, policy, None)
}

#[cfg(feature = "vector-lancedb")]
pub fn build_context_pack_with_vector_store(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: &str,
    policy: ContextPolicy,
    vector_store: &impl VectorStore,
) -> Result<ContextPack> {
    build_context_pack_inner(path.as_ref(), board, task_ref, policy, Some(vector_store))
}

fn build_context_pack_inner(
    path_ref: &Path,
    board: &str,
    task_ref: &str,
    policy: ContextPolicy,
    #[cfg_attr(not(feature = "vector-lancedb"), allow(unused_variables))] vector_store: Option<
        &dyn VectorStore,
    >,
) -> Result<ContextPack> {
    validate_page_bounds(policy.lexical_limit, MAX_SEARCH_LIMIT, 0)?;
    validate_page_bounds(policy.graph_limit, MAX_TASK_LIST_LIMIT, 0)?;
    validate_page_bounds(policy.vector_limit, MAX_TASK_LIST_LIMIT, 0)?;
    validate_page_bounds(policy.max_items, MAX_TASK_LIST_LIMIT, 0)?;
    validate_context_max_items(policy.max_items)?;

    let conn = connect_file(path_ref)?;
    let board_id = board_id(&conn, board)?;
    let mut degraded = context_derived_degraded_markers(&conn, &board_id)?;
    let mut diagnostics = Vec::new();
    let task = get_task(path_ref, board, task_ref)?;
    let subject = EntityUri::task(&task.id);
    let lexical = search_tasks(
        path_ref,
        SearchQuery {
            board: board.to_owned(),
            q: Some(task.title.clone()),
            statuses: vec![],
            assignee: None,
            include_archived: true,
            limit: policy.lexical_limit,
            offset: 0,
        },
    )?;
    let graph_status = match graph_store_status(path_ref, board) {
        Ok(status) => status,
        Err(error) => {
            push_degraded_marker(&mut degraded, "graph_error");
            push_context_diagnostic(&mut diagnostics, "graph", "graph_error", &error);
            GraphStoreStatus {
                backend: graph_backend_name(),
                enabled: cfg!(feature = "graph-oxigraph"),
                message: error.to_string(),
            }
        }
    };
    let graph = match context_graph_items(path_ref, &subject, policy.graph_limit) {
        Ok(items) => items,
        Err(error) => {
            push_degraded_marker(&mut degraded, "graph_error");
            push_context_diagnostic(&mut diagnostics, "graph", "graph_error", &error);
            Vec::new()
        }
    };
    let vector_status = context_vector_status(
        path_ref,
        &conn,
        &board_id,
        board,
        vector_store,
        &mut degraded,
        &mut diagnostics,
    );
    let vector = match context_vector_items(
        path_ref,
        &task,
        &vector_status,
        policy.vector_limit,
        vector_store,
    ) {
        Ok(items) => items,
        Err(error) => {
            push_degraded_marker(&mut degraded, "vector_error");
            push_context_diagnostic(&mut diagnostics, "vector", "vector_error", &error);
            Vec::new()
        }
    };

    kanban_context::build_context_pack(
        subject.clone(),
        policy,
        ContextBrokerInput {
            subject_item: ContextItem {
                entity_uri: subject,
                source: "subject".to_owned(),
                provenance: vec!["sqlite:tasks".to_owned()],
                score: None,
                title: Some(task.title),
                snippet: task.description,
            },
            lexical,
            graph,
            vector,
            graph_status,
            vector_status,
            degraded,
            diagnostics,
        },
    )
    .map_err(context_error)
}

fn validate_context_max_items(max_items: usize) -> Result<()> {
    if max_items == 0 {
        return Err(KanbanError::InvalidInput(
            "max_items must be >= 1 because the subject item is mandatory".to_owned(),
        ));
    }
    Ok(())
}

fn push_degraded_marker(degraded: &mut Vec<String>, marker: &str) {
    if !degraded.iter().any(|value| value == marker) {
        degraded.push(marker.to_owned());
    }
}

fn push_context_diagnostic(
    diagnostics: &mut Vec<ContextDiagnostic>,
    source: &str,
    code: &str,
    error: &impl std::fmt::Display,
) {
    diagnostics.push(ContextDiagnostic {
        source: source.to_owned(),
        code: code.to_owned(),
        message: bounded_diagnostic_message(error),
    });
}

fn bounded_diagnostic_message(error: &impl std::fmt::Display) -> String {
    const MAX_DIAGNOSTIC_MESSAGE_LEN: usize = 240;
    let mut message = error.to_string().replace(['\r', '\n'], " ");
    if message.len() > MAX_DIAGNOSTIC_MESSAGE_LEN {
        message.truncate(MAX_DIAGNOSTIC_MESSAGE_LEN);
        message.push_str("...");
    }
    message
}

fn context_error(error: ContextError) -> KanbanError {
    match error {
        ContextError::InvalidInput(message) => KanbanError::InvalidInput(message),
        ContextError::Retrieval(message) => KanbanError::Storage(message),
    }
}

fn context_derived_degraded_markers(
    #[cfg_attr(
        not(any(feature = "graph-oxigraph", feature = "vector-lancedb")),
        allow(unused_variables)
    )]
    conn: &Connection,
    #[cfg_attr(
        not(any(feature = "graph-oxigraph", feature = "vector-lancedb")),
        allow(unused_variables)
    )]
    board_id: &str,
) -> Result<Vec<String>> {
    #[cfg_attr(
        not(any(feature = "graph-oxigraph", feature = "vector-lancedb")),
        allow(unused_mut)
    )]
    let mut degraded = Vec::new();
    #[cfg(feature = "graph-oxigraph")]
    push_store_state_markers(
        conn,
        board_id,
        OXIGRAPH_RELATIONS_STORE,
        "graph",
        &mut degraded,
    )?;
    #[cfg(feature = "vector-lancedb")]
    push_store_state_markers(
        conn,
        board_id,
        LANCEDB_CHUNKS_STORE,
        "vector",
        &mut degraded,
    )?;
    Ok(degraded)
}

#[cfg(any(feature = "graph-oxigraph", feature = "vector-lancedb"))]
fn push_store_state_markers(
    conn: &Connection,
    board_id: &str,
    store_name: &str,
    marker_prefix: &str,
    degraded: &mut Vec<String>,
) -> Result<()> {
    let state = derived_status_by_name(conn, store_name)?;
    let current_last_event_id = current_last_event_id(conn, board_id)?;
    let target = store_target(store_name)?;
    let pending = has_pending_outbox_for_target(conn, target, board_id, current_last_event_id)?;
    if state.dirty {
        push_degraded_marker(degraded, &format!("{marker_prefix}_dirty"));
    }
    if pending {
        push_degraded_marker(degraded, &format!("{marker_prefix}_stale"));
    }
    if state.last_error.is_some() {
        push_degraded_marker(degraded, &format!("{marker_prefix}_error"));
    }
    Ok(())
}

#[cfg(feature = "graph-oxigraph")]
fn graph_backend_name() -> String {
    "oxigraph".to_owned()
}

#[cfg(not(feature = "graph-oxigraph"))]
fn graph_backend_name() -> String {
    "disabled".to_owned()
}

#[cfg(feature = "graph-oxigraph")]
pub fn graph_neighbors(
    path: impl AsRef<Path>,
    entity_uri: &EntityUri,
    predicate: Option<Predicate>,
    limit: usize,
) -> Result<Vec<Relation>> {
    validate_page_bounds(limit, MAX_TASK_LIST_LIMIT, 0)?;
    let graph = OxigraphStore::open(graph_store_path(path.as_ref())).map_err(graph_storage)?;
    graph
        .neighbors(entity_uri, predicate, limit)
        .map_err(graph_storage)
}

#[cfg(not(feature = "graph-oxigraph"))]
pub fn graph_neighbors(
    _path: impl AsRef<Path>,
    _entity_uri: &EntityUri,
    _predicate: Option<Predicate>,
    limit: usize,
) -> Result<Vec<Relation>> {
    validate_page_bounds(limit, MAX_TASK_LIST_LIMIT, 0)?;
    Ok(Vec::new())
}

#[cfg(feature = "graph-oxigraph")]
fn context_graph_items(path: &Path, subject: &EntityUri, limit: usize) -> Result<Vec<ContextItem>> {
    let relations = graph_neighbors(path, subject, None, limit)?;
    let conn = connect_file(path)?;
    Ok(relations
        .into_iter()
        .map(|relation| {
            let title = entity_title(&conn, relation.object_uri.as_str())
                .ok()
                .flatten();
            ContextItem {
                entity_uri: relation.object_uri,
                source: "graph".to_owned(),
                provenance: vec![format!("graph:{}", relation.predicate)],
                score: None,
                title,
                snippet: Some(relation.predicate.to_string()),
            }
        })
        .collect())
}

#[cfg(not(feature = "graph-oxigraph"))]
fn context_graph_items(
    _path: &Path,
    _subject: &EntityUri,
    _limit: usize,
) -> Result<Vec<ContextItem>> {
    Ok(Vec::new())
}

#[cfg(feature = "vector-lancedb")]
fn context_vector_items(
    path: &Path,
    task: &TaskRecord,
    status: &VectorStoreStatus,
    limit: usize,
    store: Option<&dyn VectorStore>,
) -> Result<Vec<ContextItem>> {
    if !status.enabled || limit == 0 {
        return Ok(Vec::new());
    }
    let owned_store;
    let store = match store {
        Some(store) => store,
        None => {
            owned_store = LanceDbStore::connect(LanceDbConfig::degraded(vector_store_path(path)))
                .map_err(vector_storage)?;
            &owned_store
        }
    };
    let hits = store
        .query(&VectorQuery {
            text: task_context_text(task),
            limit,
        })
        .map_err(vector_storage)?;
    vector_hits_to_context_items(path, hits)
}

#[cfg(not(feature = "vector-lancedb"))]
fn context_vector_items(
    _path: &Path,
    _task: &TaskRecord,
    _status: &VectorStoreStatus,
    _limit: usize,
    _store: Option<&dyn VectorStore>,
) -> Result<Vec<ContextItem>> {
    Ok(Vec::new())
}

#[cfg(feature = "vector-lancedb")]
fn context_vector_status(
    path: &Path,
    conn: &Connection,
    board_id: &str,
    board: &str,
    store: Option<&dyn VectorStore>,
    degraded: &mut Vec<String>,
    diagnostics: &mut Vec<ContextDiagnostic>,
) -> VectorStoreStatus {
    let status = match store {
        Some(store) => vector_store_status_with(conn, board_id, store),
        None => vector_store_status(path, board),
    };
    match status {
        Ok(status) => status,
        Err(error) => {
            push_degraded_marker(degraded, "vector_error");
            push_context_diagnostic(diagnostics, "vector", "vector_error", &error);
            VectorStoreStatus {
                backend: "lancedb".to_owned(),
                enabled: true,
                message: error.to_string(),
            }
        }
    }
}

#[cfg(not(feature = "vector-lancedb"))]
fn context_vector_status(
    path: &Path,
    _conn: &Connection,
    _board_id: &str,
    board: &str,
    _store: Option<&dyn VectorStore>,
    _degraded: &mut Vec<String>,
    _diagnostics: &mut Vec<ContextDiagnostic>,
) -> VectorStoreStatus {
    vector_store_status(path, board).expect("disabled vector status is infallible")
}

#[cfg(feature = "vector-lancedb")]
fn vector_hits_to_context_items(path: &Path, hits: Vec<VectorHit>) -> Result<Vec<ContextItem>> {
    let conn = connect_file(path)?;
    Ok(hits
        .into_iter()
        .map(|hit| {
            let title = entity_title(&conn, hit.chunk.entity_uri.as_str())
                .ok()
                .flatten();
            ContextItem {
                entity_uri: hit.chunk.entity_uri,
                source: "vector".to_owned(),
                provenance: vec!["vector:lancedb".to_owned()],
                score: Some(f64::from(hit.score)),
                title: title.or(hit.summary),
                snippet: hit.text,
            }
        })
        .collect())
}

#[cfg(any(feature = "graph-oxigraph", feature = "vector-lancedb"))]
fn entity_title(conn: &Connection, uri: &str) -> Result<Option<String>> {
    conn.query_row("SELECT title FROM entities WHERE uri=?1", [uri], |row| {
        row.get(0)
    })
    .optional()
    .map_err(storage)
}

#[cfg(feature = "vector-lancedb")]
fn task_context_text(task: &TaskRecord) -> String {
    match task.description.as_deref().map(str::trim) {
        Some(description) if !description.is_empty() => {
            format!("{}\n\n{}", task.title.trim(), description)
        }
        _ => task.title.trim().to_owned(),
    }
}

#[cfg(feature = "graph-oxigraph")]
pub fn graph_store_status(path: impl AsRef<Path>, board: &str) -> Result<GraphStoreStatus> {
    let path_ref = path.as_ref();
    let conn = connect_file(path_ref)?;
    let board_id = board_id(&conn, board)?;
    let graph = OxigraphStore::open(graph_store_path(path_ref)).map_err(graph_storage)?;
    let current_last_event_id = current_last_event_id(&conn, &board_id)?;
    let state = derived_status_by_name(&conn, OXIGRAPH_RELATIONS_STORE)?;
    let board_has_pending =
        has_pending_graph_outbox_for_board(&conn, &board_id, current_last_event_id)?;
    let mut status = graph.status();
    let lag = if board_has_pending {
        search_lag(current_last_event_id, Some(state.last_event_id))
    } else {
        0
    };
    status.message = format!(
        "{}; dirty={} last_event_id={} lag={}",
        status.message, state.dirty, state.last_event_id, lag
    );
    Ok(status)
}

#[cfg(not(feature = "graph-oxigraph"))]
pub fn graph_store_status(
    _path: impl AsRef<Path>,
    _board: &str,
) -> Result<kanban_graph::GraphStoreStatus> {
    let graph = kanban_graph::DisabledGraphStore;
    Ok(kanban_graph::RelationGraph::status(&graph))
}

#[cfg(feature = "graph-oxigraph")]
pub fn rebuild_graph_store(path: impl AsRef<Path>, board: &str) -> Result<GraphStoreStatus> {
    let path_ref = path.as_ref();
    let conn = connect_file(path_ref)?;
    let board_id = board_id(&conn, board)?;
    let last_event_id = current_last_event_id(&conn, &board_id)?;
    let relations = graph_relation_snapshot_for_board(&conn, &board_id)?;
    let entity_uris = graph_entity_uris_for_board(&conn, &board_id)?;
    let result = (|| -> Result<()> {
        let graph = OxigraphStore::open(graph_store_path(path_ref)).map_err(graph_storage)?;
        graph
            .replace_entities(&entity_uris, &relations)
            .map_err(graph_storage)?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            let now = SystemClock.now_ms();
            mark_derived_store_success(
                &conn,
                OXIGRAPH_RELATIONS_STORE,
                &board_id,
                last_event_id,
                true,
                now,
            )?;
            Ok(GraphStoreStatus {
                backend: "oxigraph".to_owned(),
                enabled: true,
                message: format!(
                    "Rebuilt Oxigraph relation store ({} relation(s))",
                    relations.len()
                ),
            })
        }
        Err(error) => {
            mark_derived_store_failure(
                &conn,
                OXIGRAPH_RELATIONS_STORE,
                &board_id,
                &error.to_string(),
                SystemClock.now_ms(),
            )?;
            Err(KanbanError::Storage(error.to_string()))
        }
    }
}

#[cfg(not(feature = "graph-oxigraph"))]
pub fn rebuild_graph_store(
    path: impl AsRef<Path>,
    board: &str,
) -> Result<kanban_graph::GraphStoreStatus> {
    graph_store_status(path, board)
}

#[cfg(feature = "graph-oxigraph")]
pub fn sync_graph_store(path: impl AsRef<Path>, board: &str) -> Result<GraphStoreStatus> {
    let path_ref = path.as_ref();
    let conn = connect_file(path_ref)?;
    let board_id = board_id(&conn, board)?;
    let last_event_id = current_last_event_id(&conn, &board_id)?;
    let state = derived_status_by_name(&conn, OXIGRAPH_RELATIONS_STORE)?;
    if !has_pending_graph_outbox_for_board(&conn, &board_id, last_event_id)? {
        return graph_store_status(path_ref, board);
    }
    let jobs = pending_graph_outbox_for_board(&conn, &board_id, last_event_id)?;
    let result = (|| -> Result<()> {
        let graph = OxigraphStore::open(graph_store_path(path_ref)).map_err(graph_storage)?;
        if state.last_event_id == 0 || jobs.iter().any(|job| job.action == "rebuild") {
            let relations = graph_relation_snapshot_for_board(&conn, &board_id)?;
            let entity_uris = graph_entity_uris_for_board(&conn, &board_id)?;
            graph
                .replace_entities(&entity_uris, &relations)
                .map_err(graph_storage)?;
        } else {
            let mut affected = jobs
                .iter()
                .map(|job| job.entity_uri.clone())
                .collect::<Vec<_>>();
            affected.sort();
            affected.dedup();
            for uri in affected {
                let entity_uri = EntityUri::new(uri).map_err(graph_storage)?;
                let relations = graph_relations_for_entity(&conn, &board_id, entity_uri.as_str())?;
                if relations.is_empty() {
                    graph.delete(&entity_uri).map_err(graph_storage)?;
                } else {
                    graph.upsert(&relations).map_err(graph_storage)?;
                }
            }
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            let now = SystemClock.now_ms();
            mark_derived_store_success(
                &conn,
                OXIGRAPH_RELATIONS_STORE,
                &board_id,
                last_event_id,
                false,
                now,
            )?;
            Ok(GraphStoreStatus {
                backend: "oxigraph".to_owned(),
                enabled: true,
                message: format!("Synced Oxigraph relation store ({} job(s))", jobs.len()),
            })
        }
        Err(error) => {
            mark_derived_store_failure(
                &conn,
                OXIGRAPH_RELATIONS_STORE,
                &board_id,
                &error.to_string(),
                SystemClock.now_ms(),
            )?;
            Err(KanbanError::Storage(error.to_string()))
        }
    }
}

#[cfg(not(feature = "graph-oxigraph"))]
pub fn sync_graph_store(
    path: impl AsRef<Path>,
    board: &str,
) -> Result<kanban_graph::GraphStoreStatus> {
    graph_store_status(path, board)
}

#[cfg(feature = "vector-lancedb")]
pub fn vector_store_status(path: impl AsRef<Path>, board: &str) -> Result<VectorStoreStatus> {
    let path_ref = path.as_ref();
    let conn = connect_file(path_ref)?;
    let board_id = board_id(&conn, board)?;
    vector_store_status_without_provider(&conn, &board_id)
}

#[cfg(not(feature = "vector-lancedb"))]
pub fn vector_store_status(_path: impl AsRef<Path>, _board: &str) -> Result<VectorStoreStatus> {
    Ok(kanban_vector::DisabledVectorStore.status())
}

#[cfg(feature = "vector-lancedb")]
pub fn rebuild_vector_store(path: impl AsRef<Path>, board: &str) -> Result<VectorStoreStatus> {
    let path_ref = path.as_ref();
    let store = LanceDbStore::connect(LanceDbConfig::degraded(vector_store_path(path_ref)))
        .map_err(vector_storage)?;
    rebuild_vector_store_with(path_ref, board, &store)
}

#[cfg(not(feature = "vector-lancedb"))]
pub fn rebuild_vector_store(path: impl AsRef<Path>, board: &str) -> Result<VectorStoreStatus> {
    vector_store_status(path, board)
}

#[cfg(feature = "vector-lancedb")]
pub fn sync_vector_store(path: impl AsRef<Path>, board: &str) -> Result<VectorStoreStatus> {
    let path_ref = path.as_ref();
    let store = LanceDbStore::connect(LanceDbConfig::degraded(vector_store_path(path_ref)))
        .map_err(vector_storage)?;
    sync_vector_store_with(path_ref, board, &store)
}

#[cfg(not(feature = "vector-lancedb"))]
pub fn sync_vector_store(path: impl AsRef<Path>, board: &str) -> Result<VectorStoreStatus> {
    vector_store_status(path, board)
}

pub fn rebuild_vector_store_with(
    path: impl AsRef<Path>,
    board: &str,
    store: &impl VectorStore,
) -> Result<VectorStoreStatus> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let last_event_id = current_last_event_id(&conn, &board_id)?;
    let chunks = vector_chunks_for_board(&conn, &board_id, store.chunk_embedding_model())?;
    match store
        .delete_board(&board_id)
        .and_then(|()| store.upsert(&chunks))
    {
        Ok(()) => {
            let now = SystemClock.now_ms();
            mark_derived_store_success(
                &conn,
                LANCEDB_CHUNKS_STORE,
                &board_id,
                last_event_id,
                true,
                now,
            )?;
            let mut status = store.status();
            status.message = format!(
                "{}; rebuilt {} chunk(s); dirty=false last_event_id={} lag=0",
                status.message,
                chunks.len(),
                last_event_id.unwrap_or(0)
            );
            Ok(status)
        }
        Err(error) => {
            mark_derived_store_failure(
                &conn,
                LANCEDB_CHUNKS_STORE,
                &board_id,
                &error.to_string(),
                SystemClock.now_ms(),
            )?;
            Err(vector_storage(error))
        }
    }
}

pub fn sync_vector_store_with(
    path: impl AsRef<Path>,
    board: &str,
    store: &impl VectorStore,
) -> Result<VectorStoreStatus> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let last_event_id = current_last_event_id(&conn, &board_id)?;
    let state = derived_status_by_name(&conn, LANCEDB_CHUNKS_STORE)?;
    if !has_pending_vector_outbox_for_board(&conn, &board_id, last_event_id)? {
        return vector_store_status_with(&conn, &board_id, store);
    }
    let jobs = pending_vector_outbox_for_board(&conn, &board_id, last_event_id)?;
    let full_rebuild = state.last_event_id == 0 || jobs.iter().any(|job| job.action == "rebuild");
    let chunks = if state.last_event_id == 0 || jobs.iter().any(|job| job.action == "rebuild") {
        vector_chunks_for_board(&conn, &board_id, store.chunk_embedding_model())?
    } else {
        let entity_uris = jobs
            .iter()
            .map(|job| job.entity_uri.clone())
            .collect::<Vec<_>>();
        vector_chunks_for_entity_uris(
            &conn,
            &board_id,
            &entity_uris,
            store.chunk_embedding_model(),
        )?
    };
    let entity_uris = if full_rebuild {
        Vec::new()
    } else {
        let mut entity_uris = jobs
            .iter()
            .map(|job| job.entity_uri.clone())
            .collect::<Vec<_>>();
        entity_uris.sort();
        entity_uris.dedup();
        entity_uris
    };
    let write_result = if full_rebuild {
        store.delete_board(&board_id)
    } else {
        store.delete_entities(&entity_uris)
    }
    .and_then(|()| store.upsert(&chunks));
    match write_result {
        Ok(()) => {
            let now = SystemClock.now_ms();
            mark_derived_store_success(
                &conn,
                LANCEDB_CHUNKS_STORE,
                &board_id,
                last_event_id,
                false,
                now,
            )?;
            let mut status = store.status();
            status.message = format!(
                "{}; synced {} chunk(s) from {} job(s); dirty=false last_event_id={} lag=0",
                status.message,
                chunks.len(),
                jobs.len(),
                last_event_id.unwrap_or(0)
            );
            Ok(status)
        }
        Err(error) => {
            mark_derived_store_failure(
                &conn,
                LANCEDB_CHUNKS_STORE,
                &board_id,
                &error.to_string(),
                SystemClock.now_ms(),
            )?;
            Err(vector_storage(error))
        }
    }
}

pub fn queue_stats(path: impl AsRef<Path>, board: &str) -> Result<QueueStats> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let generated_at = SystemClock.now_ms();
    let mut status_stmt = conn
        .prepare(
            "SELECT status, COUNT(*) FROM tasks WHERE board_id=?1 GROUP BY status ORDER BY status",
        )
        .map_err(storage)?;
    let status_counts = status_stmt
        .query_map([&board_id], |row| {
            Ok(StatusCount {
                status: row.get(0)?,
                count: row.get(1)?,
            })
        })
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;

    let mut stale_stmt = conn
        .prepare(
            "SELECT id,seq,title,claim_owner,claim_expires_at,last_heartbeat_at,current_run_id,retry_count,max_retries \
             FROM tasks WHERE board_id=?1 AND status='running' AND claim_expires_at <= ?2 \
             ORDER BY claim_expires_at ASC, updated_at ASC",
        )
        .map_err(storage)?;
    let stale_claims = stale_stmt
        .query_map(params![&board_id, generated_at], |row| {
            Ok(StaleClaimRecord {
                task_id: row.get(0)?,
                seq: row.get(1)?,
                title: row.get(2)?,
                claim_owner: row.get(3)?,
                claim_expires_at: row.get(4)?,
                last_heartbeat_at: row.get(5)?,
                current_run_id: row.get(6)?,
                retry_count: row.get(7)?,
                max_retries: row.get(8)?,
            })
        })
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;

    let mut blocked_stmt = conn
        .prepare(
            "SELECT COALESCE(NULLIF(status_reason, ''), 'unspecified') AS reason, COUNT(*) \
             FROM tasks WHERE board_id=?1 AND status='blocked' \
             GROUP BY reason ORDER BY COUNT(*) DESC, reason ASC",
        )
        .map_err(storage)?;
    let blocked_reasons = blocked_stmt
        .query_map([&board_id], |row| {
            Ok(BlockedReasonCount {
                reason: row.get(0)?,
                count: row.get(1)?,
            })
        })
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;

    Ok(QueueStats {
        board_id,
        generated_at,
        status_counts,
        stale_claims,
        blocked_reasons,
    })
}

pub fn doctor_database(path: impl AsRef<Path>) -> Result<DoctorReport> {
    let path = path.as_ref();
    let conn = connect_existing_file(path)?;
    doctor_report_conn(&conn, path.parent())
}

fn doctor_report_conn(conn: &Connection, db_dir: Option<&Path>) -> Result<DoctorReport> {
    let now = SystemClock.now_ms();
    let integrity_check: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(storage)?;
    let has_migrations_table = table_exists(conn, "schema_migrations")?;
    let migration_version = if has_migrations_table {
        conn.query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get::<_, Option<i64>>(0)
        })
        .optional()
        .map_err(storage)?
        .flatten()
    } else {
        None
    };
    let user_version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(storage)?;
    if migration_version != Some(user_version)
        || !doctor_tables_present(conn, migration_version, user_version)?
    {
        return Ok(DoctorReport {
            ok: false,
            integrity_check,
            migration_version,
            user_version,
            expired_running_tasks: 0,
            running_tasks_without_active_run: 0,
            orphan_running_runs: 0,
            dependency_cycles: 0,
            archived_dependency_edges: 0,
            missing_run_logs: 0,
            suspicious_run_log_paths: 0,
            executable_dependency_violations: 0,
            executable_spec_violations: 0,
            executable_schedule_violations: 0,
            outbox_pending: 0,
            outbox_running: 0,
            outbox_failed: 0,
            derived_dirty_stores: 0,
            derived_error_stores: 0,
            derived_stores: Vec::new(),
        });
    }
    let expired_running_tasks: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE status='running' AND claim_expires_at <= ?1",
            [now],
            |row| row.get(0),
        )
        .map_err(storage)?;
    let running_tasks_without_active_run: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks t WHERE t.status='running' AND (t.current_run_id IS NULL OR NOT EXISTS (SELECT 1 FROM task_runs r WHERE r.id=t.current_run_id AND r.task_id=t.id AND r.status='running' AND r.claim_token=t.claim_token))",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    let orphan_running_runs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM task_runs r WHERE r.status='running' AND NOT EXISTS (SELECT 1 FROM tasks t WHERE t.id=r.task_id AND t.status='running' AND t.current_run_id=r.id AND t.claim_token=r.claim_token)",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    let dependency_cycles = count_dependency_cycles(conn)?;
    let archived_dependency_edges: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM task_dependencies d \
             JOIN tasks p ON p.id=d.parent_task_id \
             JOIN tasks c ON c.id=d.child_task_id \
             WHERE (p.status='archived' AND c.status!='archived') OR (c.status='archived' AND p.status!='archived')",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    let (missing_run_logs, suspicious_run_log_paths) = count_run_log_path_findings(conn, db_dir)?;
    let executable_dependency_violations = count_executable_dependency_violations(conn)?;
    let executable_spec_violations = count_executable_spec_violations(conn)?;
    let executable_schedule_violations = count_executable_schedule_violations(conn, now)?;
    let derived_stores = doctor_derived_store_reports(conn)?;
    let outbox_pending = count_table_status(conn, "index_outbox", "pending")?;
    let outbox_running = count_table_status(conn, "index_outbox", "running")?;
    let outbox_failed = count_table_status(conn, "index_outbox", "failed")?;
    let derived_dirty_stores = derived_stores.iter().filter(|store| store.dirty).count() as i64;
    let derived_error_stores = derived_stores
        .iter()
        .filter(|store| store.last_error.is_some() || store.failed_outbox > 0)
        .count() as i64;
    let ok = integrity_check == "ok"
        && migration_version == Some(user_version)
        && expired_running_tasks == 0
        && running_tasks_without_active_run == 0
        && orphan_running_runs == 0
        && dependency_cycles == 0
        && archived_dependency_edges == 0
        && missing_run_logs == 0
        && suspicious_run_log_paths == 0
        && executable_dependency_violations == 0
        && executable_spec_violations == 0
        && executable_schedule_violations == 0
        && outbox_failed == 0
        && derived_error_stores == 0;
    Ok(DoctorReport {
        ok,
        integrity_check,
        migration_version,
        user_version,
        expired_running_tasks,
        running_tasks_without_active_run,
        orphan_running_runs,
        dependency_cycles,
        archived_dependency_edges,
        missing_run_logs,
        suspicious_run_log_paths,
        executable_dependency_violations,
        executable_spec_violations,
        executable_schedule_violations,
        outbox_pending,
        outbox_running,
        outbox_failed,
        derived_dirty_stores,
        derived_error_stores,
        derived_stores,
    })
}

fn doctor_derived_store_reports(conn: &Connection) -> Result<Vec<DoctorDerivedStoreReport>> {
    if !table_exists(conn, "derived_store_state")? {
        return Ok(Vec::new());
    }
    let stores = derived_store_statuses_conn(conn)?;
    stores
        .into_iter()
        .map(|store| {
            let seed = derived_store_for_name(&store.store_name).ok_or_else(|| {
                KanbanError::Storage(format!("unknown derived store: {}", store.store_name))
            })?;
            Ok(DoctorDerivedStoreReport {
                store_name: store.store_name,
                schema_version: store.schema_version,
                last_event_id: store.last_event_id,
                dirty: store.dirty,
                last_error: store.last_error,
                pending_outbox: count_outbox_for_target(conn, seed.target, "pending")?,
                running_outbox: count_outbox_for_target(conn, seed.target, "running")?,
                failed_outbox: count_outbox_for_target(conn, seed.target, "failed")?,
            })
        })
        .collect()
}

fn count_outbox_for_target(conn: &Connection, target: OutboxTarget, status: &str) -> Result<i64> {
    if !table_exists(conn, "index_outbox")? {
        return Ok(0);
    }
    conn.query_row(
        "SELECT COUNT(*) FROM index_outbox WHERE status=?1 AND target IN (?2, 'all')",
        params![status, target.as_str()],
        |row| row.get(0),
    )
    .map_err(storage)
}

pub fn checkpoint_database(path: impl AsRef<Path>) -> Result<CheckpointResult> {
    let conn = connect_existing_database(path.as_ref())?;
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
        Ok(CheckpointResult {
            busy: row.get(0)?,
            log_frames: row.get(1)?,
            checkpointed_frames: row.get(2)?,
        })
    })
    .map_err(storage)
}

pub fn vacuum_database(path: impl AsRef<Path>) -> Result<MaintenanceResult> {
    let conn = connect_existing_database(path.as_ref())?;
    conn.execute_batch("VACUUM").map_err(storage)?;
    Ok(MaintenanceResult { ok: true })
}

pub fn backup_database(path: impl AsRef<Path>, out_path: impl AsRef<Path>) -> Result<BackupResult> {
    let out_path = out_path.as_ref();
    if out_path.exists() {
        return Err(KanbanError::InvalidInput(format!(
            "backup target already exists: {}",
            out_path.display()
        )));
    }
    let conn = connect_existing_database(path.as_ref())?;
    if let Some(parent) = out_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| KanbanError::Storage(error.to_string()))?;
    }
    checkpoint_database(path.as_ref())?;
    conn.execute("VACUUM main INTO ?1", [out_path.to_string_lossy().as_ref()])
        .map_err(storage)?;
    Ok(BackupResult {
        out_path: out_path.to_path_buf(),
    })
}

pub fn begin_database_replace(path: impl AsRef<Path>) -> Result<DatabaseReplaceGuard> {
    let path = path.as_ref();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| KanbanError::Storage(error.to_string()))?;
    }
    let lock_path = maintenance_lock_path(path);
    if maintenance_lock_blocks(&lock_path)? {
        return Err(KanbanError::InvalidInput(format!(
            "database is locked for maintenance: {}",
            path.display()
        )));
    }
    let runtime_lock = runtime_lock_path(path);
    if runtime_lock_blocks(&runtime_lock)? {
        return Err(KanbanError::InvalidInput(format!(
            "database has active serve/dispatch runtime; stop kb serve/dispatch before import --replace: {}",
            path.display()
        )));
    }
    create_lock_file(&lock_path, "maintenance", path)?;
    let guard = DatabaseReplaceGuard { lock_path };
    if path.exists() && !path.is_file() {
        drop(guard);
        return Err(KanbanError::InvalidInput(format!(
            "database path is not a file: {}",
            path.display()
        )));
    }
    if path.exists()
        && path.is_file()
        && let Err(error) = assert_database_idle_for_replace(path)
    {
        drop(guard);
        return Err(error);
    }
    Ok(guard)
}

pub fn begin_database_runtime(path: impl AsRef<Path>) -> Result<DatabaseRuntimeGuard> {
    let path = path.as_ref();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| KanbanError::Storage(error.to_string()))?;
    }
    let lock_path = runtime_lock_path(path);
    if runtime_lock_blocks(&lock_path)? {
        return Err(KanbanError::InvalidInput(format!(
            "database already has an active serve/dispatch runtime: {}",
            path.display()
        )));
    }
    create_lock_file(&lock_path, "runtime", path)?;
    Ok(DatabaseRuntimeGuard { lock_path })
}

fn create_lock_file(lock_path: &Path, kind: &str, db_path: &Path) -> Result<()> {
    let lock_result = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock_path);
    let mut lock_file = match lock_result {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(KanbanError::InvalidInput(format!(
                "database is locked for {kind}: {}",
                db_path.display()
            )));
        }
        Err(error) => return Err(KanbanError::Storage(error.to_string())),
    };
    writeln!(lock_file, "pid={}", std::process::id())
        .map_err(|error| KanbanError::Storage(error.to_string()))?;
    writeln!(lock_file, "kind={kind}").map_err(|error| KanbanError::Storage(error.to_string()))
}

pub fn export_jsonl(
    path: impl AsRef<Path>,
    board: &str,
    out_path: impl AsRef<Path>,
) -> Result<ExportResult> {
    let conn = connect_existing_database(path.as_ref())?;
    let out_path = out_path.as_ref();
    if out_path.exists() {
        return Err(KanbanError::InvalidInput(format!(
            "export target already exists: {}",
            out_path.display()
        )));
    }
    if let Some(parent) = out_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| KanbanError::Storage(error.to_string()))?;
    }
    let export_now = SystemClock.now_ms();
    let (records, temp_path) = with_read_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let (temp_path, mut file) = create_temp_export_file(out_path)?;
        let mut records = 0;
        records += write_jsonl_table(
            &conn,
            &mut file,
            "board",
            "boards",
            "WHERE id=?",
            vec![Value::Text(board_id.clone())],
            export_now,
        )?;
        for (record_type, table) in BOARD_SCOPED_EXPORT_TABLES {
            records += write_jsonl_table(
                &conn,
                &mut file,
                record_type,
                table,
                "WHERE board_id=?",
                vec![Value::Text(board_id.clone())],
                export_now,
            )?;
        }
        records += write_export_sanitized_events(&conn, &mut file, &board_id, export_now)?;
        records += write_jsonl_table(
            &conn,
            &mut file,
            "setting",
            "app_settings",
            "",
            Vec::new(),
            export_now,
        )?;
        file.sync_all()
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
        Ok((records, temp_path))
    })?;
    if let Err(error) = fs::rename(&temp_path, out_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(KanbanError::Storage(error.to_string()));
    }
    Ok(ExportResult {
        out_path: out_path.to_path_buf(),
        records,
    })
}

pub fn import_jsonl(
    path: impl AsRef<Path>,
    input_path: impl AsRef<Path>,
    replace: bool,
) -> Result<ImportResult> {
    let db_path = path.as_ref();
    let conn = connect_file(db_path)?;
    if !replace && database_has_user_records(&conn)? {
        return Err(KanbanError::InvalidInput(
            "import requires --replace when the database already has records".into(),
        ));
    }
    let input_path = input_path.as_ref();
    let file = File::open(input_path).map_err(|error| KanbanError::Storage(error.to_string()))?;
    with_immediate_tx(&conn, || {
        if replace {
            for table in IMPORT_DELETE_ORDER {
                conn.execute(&format!("DELETE FROM {table}"), [])
                    .map_err(storage)?;
            }
        }
        let mut records = 0;
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|error| KanbanError::Storage(error.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(&line)
                .map_err(|error| KanbanError::InvalidInput(error.to_string()))?;
            insert_jsonl_record(&conn, &value)?;
            records += 1;
        }
        reject_imported_active_claims(&conn)?;
        validate_imported_snapshot(&conn)?;
        let report = doctor_report_conn(&conn, db_path.parent())?;
        if !report.ok {
            return Err(KanbanError::InvalidInput(
                "imported data failed doctor checks".into(),
            ));
        }
        Ok(ImportResult {
            input_path: input_path.to_path_buf(),
            records,
        })
    })
}

pub fn get_board(path: impl AsRef<Path>, slug_or_id: &str) -> Result<BoardRecord> {
    let conn = connect_file(path.as_ref())?;
    get_board_conn(&conn, slug_or_id)
}

pub fn list_board_columns(
    path: impl AsRef<Path>,
    board_slug_or_id: &str,
) -> Result<Vec<BoardColumnRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board_slug_or_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT id,board_id,status,title,position,hidden,wip_limit,created_at,updated_at \
             FROM board_columns WHERE board_id=?1 ORDER BY position ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map([board_id], board_column_from_row)
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

pub fn create_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    input: CreateTask,
) -> Result<TaskRecord> {
    create_task_with_dependencies(path, board, actor, input, &[])
}

pub fn create_task_with_dependencies(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    input: CreateTask,
    depends_on: &[String],
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let title = input.title.trim().to_owned();
    if title.is_empty() {
        return Err(KanbanError::InvalidInput("title is required".into()));
    }
    if !json_valid(&conn, &input.metadata_json)? {
        return Err(KanbanError::InvalidInput(
            "metadata_json must be valid JSON".into(),
        ));
    }
    let status = initial_status(
        input.status,
        input.description.as_deref(),
        input.scheduled_at,
        now,
    )?;
    let id = new_task_id();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let seq: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(seq), 0) + 1 FROM tasks WHERE board_id=?1",
                [&board_id],
                |r| r.get(0),
            )
            .map_err(storage)?;
        conn.execute(
        "INSERT INTO tasks(id, board_id, seq, title, description, status, assignee, priority, position, scheduled_at, due_at, created_by, created_at, updated_at, metadata_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?3 * 1024, ?9, ?10, ?11, ?12, ?12, ?13)",
        params![id, board_id, seq, title, input.description, status.as_str(), input.assignee, input.priority, input.scheduled_at, input.due_at, actor, now, input.metadata_json],
        ).map_err(storage)?;
        let payload = json!({ "status": status.as_str() }).to_string();
        insert_event(
            &conn,
            &board_id,
            Some(&id),
            None,
            "task.created",
            actor,
            &payload,
            now,
        )?;
        for parent_ref in depends_on {
            let parent = resolve_task(&conn, &board_id, parent_ref)?;
            let child = get_task_by_id(&conn, &board_id, &id)?;
            add_dependency_in_current_tx(&conn, &board_id, actor, &parent, &child, now)?;
        }
        get_task_by_id(&conn, &board_id, &id)
    })
}

pub fn update_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    patch: TaskPatch,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    with_immediate_tx(&conn, || {
        let mut task = resolve_task(&conn, &board_id, task_ref)?;
        let recompute_needed =
            patch.title.is_some() || patch.description.is_some() || patch.scheduled_at.is_some();
        if patch
            .expected_lock_version
            .is_some_and(|expected| task.lock_version != expected)
        {
            return Err(KanbanError::InvalidInput("lock_version mismatch".into()));
        }
        if let Some(title) = patch.title {
            if title.trim().is_empty() {
                return Err(KanbanError::InvalidInput("title is required".into()));
            }
            task.title = title;
        }
        if let Some(description) = patch.description {
            task.description = description;
        }
        if let Some(assignee) = patch.assignee {
            task.assignee = assignee;
        }
        if let Some(priority) = patch.priority {
            task.priority = priority;
        }
        if let Some(scheduled_at) = patch.scheduled_at {
            task.scheduled_at = scheduled_at;
        }
        if let Some(due_at) = patch.due_at {
            task.due_at = due_at;
        }
        if let Some(metadata_json) = patch.metadata_json {
            if !json_valid(&conn, &metadata_json)? {
                return Err(KanbanError::InvalidInput(
                    "metadata_json must be valid JSON".into(),
                ));
            }
            task.metadata_json = metadata_json;
        }
        if recompute_needed && is_active_recomputable_status(task.status) {
            task.status = recompute_ready_status(&conn, &task, now)?;
        }
        let changed = conn.execute(
        "UPDATE tasks SET title=?1, description=?2, status=?3, assignee=?4, priority=?5, scheduled_at=?6, due_at=?7, metadata_json=?8, updated_at=?9, lock_version=lock_version+1 WHERE id=?10 AND board_id=?11",
        params![task.title, task.description, task.status.as_str(), task.assignee, task.priority, task.scheduled_at, task.due_at, task.metadata_json, now, task.id, board_id],
        ).map_err(storage)?;
        if changed != 1 {
            return Err(KanbanError::InvalidTransition("task update failed".into()));
        }
        insert_event(
            &conn,
            &board_id,
            Some(&task.id),
            None,
            "task.updated",
            actor,
            "{}",
            now,
        )?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn specify_task(
    path: impl AsRef<Path>,
    actor: &str,
    task_id: &str,
    description: Option<String>,
    scheduled_at: Option<i64>,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id_for_task(&conn, task_id)?;
    let mut task = get_task_by_id(&conn, &board_id, task_id)?;
    if task.status != TaskStatus::Triage {
        return Err(KanbanError::InvalidTransition(format!(
            "cannot specify from {}",
            task.status.as_str()
        )));
    }
    if let Some(description) = description {
        task.description = Some(description);
    }
    if let Some(scheduled_at) = scheduled_at {
        task.scheduled_at = Some(scheduled_at);
    }
    if matches!(
        task.status,
        TaskStatus::Triage | TaskStatus::Todo | TaskStatus::Scheduled | TaskStatus::Ready
    ) {
        task.status = recompute_ready_status(&conn, &task, now)?;
    }
    with_immediate_tx(&conn, || {
        conn.execute(
            "UPDATE tasks SET description=?1, scheduled_at=?2, status=?3, updated_at=?4, lock_version=lock_version+1 WHERE id=?5 AND board_id=?6",
            params![task.description, task.scheduled_at, task.status.as_str(), now, task.id, board_id],
        )
        .map_err(storage)?;
        insert_event(
            &conn,
            &board_id,
            Some(&task.id),
            None,
            "task.specified",
            actor,
            &json!({ "to_status": task.status.as_str() }).to_string(),
            now,
        )?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn list_tasks(
    path: impl AsRef<Path>,
    board: &str,
    statuses: &[TaskStatus],
    include_archived: bool,
) -> Result<Vec<TaskRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let mut tasks = query_tasks(&conn, &board_id)?;
    if !include_archived {
        tasks.retain(|t| t.status != TaskStatus::Archived);
    }
    if !statuses.is_empty() {
        tasks.retain(|t| statuses.contains(&t.status));
    }
    Ok(tasks)
}

pub fn list_tasks_page(
    path: impl AsRef<Path>,
    board: &str,
    options: TaskListOptions,
) -> Result<TaskListPage> {
    validate_page_bounds(options.limit, MAX_TASK_LIST_LIMIT, options.offset)?;
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let (where_sql, params) = task_query_where(&board_id, &options);
    let total_sql = format!("SELECT COUNT(*) FROM tasks {where_sql}");
    let total: i64 = conn
        .query_row(&total_sql, params_from_iter(params.iter()), |row| {
            row.get(0)
        })
        .map_err(storage)?;

    let mut page_params = params;
    page_params.push(Value::Integer(
        options.limit.try_into().expect("validated limit"),
    ));
    page_params.push(Value::Integer(
        options.offset.try_into().expect("validated offset"),
    ));
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks {where_sql} ORDER BY {} LIMIT ? OFFSET ?",
        task_order_by(options.sort)
    );
    let mut stmt = conn.prepare(&sql).map_err(storage)?;
    let rows = stmt
        .query_map(params_from_iter(page_params.iter()), task_from_row)
        .map_err(storage)?;
    let tasks = rows
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    Ok(TaskListPage {
        tasks,
        total: total as usize,
    })
}

pub fn search_tasks(path: impl AsRef<Path>, query: SearchQuery) -> Result<SearchResults> {
    validate_page_bounds(query.limit, MAX_SEARCH_LIMIT, query.offset)?;
    #[cfg(feature = "tantivy-backend")]
    {
        let path_ref = path.as_ref();
        let conn = connect_file(path_ref)?;
        let board_id = board_id(&conn, &query.board)?;
        let last_event_id = current_last_event_id(&conn, &board_id)?;
        let state = read_search_index_state(&conn, &board_id)?;
        let indexed_last_event_id = state.as_ref().and_then(|state| state.last_event_id);
        let index_path = task_index_path(path_ref);
        if kanban_search::tantivy_backend::task_index_exists(&index_path) {
            let metadata =
                match kanban_search::tantivy_backend::validate_task_index(&index_path, &board_id) {
                    Ok(metadata) => metadata,
                    Err(err) if err.is_fallback_eligible() => {
                        return sqlite_search_tasks(path_ref, query, true, indexed_last_event_id);
                    }
                    Err(err) => return Err(search_storage(err)),
                };
            let contract = search_index_contract(indexed_last_event_id, &metadata);
            let validated_indexed_last_event_id = contract.indexed_last_event_id;
            if contract.mismatch {
                return sqlite_search_tasks(path_ref, query, true, validated_indexed_last_event_id);
            }
            if search_index_ahead(last_event_id, validated_indexed_last_event_id) {
                return sqlite_search_tasks(path_ref, query, true, validated_indexed_last_event_id);
            }
            if tantivy_literal_sqlite_fallback_required(&query) {
                return sqlite_search_tasks(path_ref, query, true, validated_indexed_last_event_id);
            }
            match kanban_search::tantivy_backend::search_task_index(
                &index_path,
                &board_id,
                &query,
                last_event_id,
            ) {
                Ok(results) => {
                    let indexed = search_index_contract(indexed_last_event_id, &metadata)
                        .indexed_last_event_id;
                    let lag = search_lag(last_event_id, indexed);
                    if search_index_ahead(last_event_id, indexed) {
                        return sqlite_search_tasks(path_ref, query, true, indexed);
                    }
                    if state.as_ref().is_some_and(|state| state.dirty) || lag > 0 || results.1.stale
                    {
                        return sqlite_search_tasks(path_ref, query, true, indexed);
                    }
                    return Ok(SearchResults {
                        hits: results.0,
                        meta: SearchMeta {
                            last_event_id: indexed,
                            index_lag_events: Some(lag),
                            stale: false,
                            ..results.1
                        },
                    });
                }
                Err(err) if err.is_fallback_eligible() => {
                    return sqlite_search_tasks(
                        path_ref,
                        query,
                        true,
                        validated_indexed_last_event_id,
                    );
                }
                Err(err) => return Err(search_storage(err)),
            }
        }
    }
    sqlite_search_tasks(path, query, false, None)
}

fn sqlite_search_tasks(
    path: impl AsRef<Path>,
    query: SearchQuery,
    stale: bool,
    indexed_last_event_id: Option<i64>,
) -> Result<SearchResults> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, &query.board)?;
    let (where_sql, mut params) = search_task_where(&board_id, &query);
    let needle = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{}%", sqlite_like_literal(&value.to_lowercase())));
    let (score_sql, snippet_sql, outer_filter, mut search_params) = if let Some(needle) =
        needle.as_deref()
    {
        (
            "CASE WHEN lower(t.title) LIKE ? ESCAPE '\\' THEN 100.0 ELSE 0.0 END \
                 + CASE WHEN lower(COALESCE(t.description, '')) LIKE ? ESCAPE '\\' THEN 60.0 ELSE 0.0 END \
                 + CASE WHEN EXISTS (SELECT 1 FROM task_comments c WHERE c.task_id=t.id AND lower(c.body) LIKE ? ESCAPE '\\') THEN 40.0 ELSE 0.0 END \
                 + CASE WHEN EXISTS (SELECT 1 FROM task_runs r WHERE r.task_id=t.id AND (lower(COALESCE(r.summary, '')) LIKE ? ESCAPE '\\' OR lower(COALESCE(r.error, '')) LIKE ? ESCAPE '\\')) THEN 30.0 ELSE 0.0 END \
                 + CASE WHEN EXISTS (SELECT 1 FROM task_events e WHERE e.task_id=t.id AND (lower(e.kind) LIKE ? ESCAPE '\\' OR lower(e.payload_json) LIKE ? ESCAPE '\\')) THEN 20.0 ELSE 0.0 END",
            "COALESCE(\
                    CASE WHEN lower(t.title) LIKE ? ESCAPE '\\' THEN t.title END,\
                    CASE WHEN lower(COALESCE(t.description, '')) LIKE ? ESCAPE '\\' THEN t.description END,\
                    (SELECT c.body FROM task_comments c WHERE c.task_id=t.id AND lower(c.body) LIKE ? ESCAPE '\\' ORDER BY c.created_at ASC, c.id ASC LIMIT 1),\
                    (SELECT CASE WHEN lower(COALESCE(r.summary, '')) LIKE ? ESCAPE '\\' THEN r.summary ELSE r.error END FROM task_runs r WHERE r.task_id=t.id AND (lower(COALESCE(r.summary, '')) LIKE ? ESCAPE '\\' OR lower(COALESCE(r.error, '')) LIKE ? ESCAPE '\\') ORDER BY r.started_at DESC, r.id ASC LIMIT 1),\
                    (SELECT e.kind || ' ' || e.payload_json FROM task_events e WHERE e.task_id=t.id AND (lower(e.kind) LIKE ? ESCAPE '\\' OR lower(e.payload_json) LIKE ? ESCAPE '\\') ORDER BY e.id ASC LIMIT 1)\
                )",
            "WHERE score > 0.0",
            vec![
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
                Value::Text(needle.to_owned()),
            ],
        )
    } else {
        ("0.0", "NULL", "", Vec::new())
    };
    search_params.append(&mut params);
    search_params.push(Value::Integer(
        query.limit.try_into().expect("validated limit"),
    ));
    search_params.push(Value::Integer(
        query.offset.try_into().expect("validated offset"),
    ));
    let sql = format!(
        "SELECT task_id, seq, score, snippet FROM (\
             SELECT t.id AS task_id, t.seq AS seq, ({score_sql}) AS score, {snippet_sql} AS snippet, t.updated_at AS updated_at \
             FROM tasks t {where_sql}\
         ) {outer_filter} ORDER BY score DESC, updated_at DESC, seq ASC LIMIT ? OFFSET ?"
    );
    let mut stmt = conn.prepare(&sql).map_err(storage)?;
    let rows = stmt
        .query_map(params_from_iter(search_params.iter()), |row| {
            Ok(SearchHit {
                task_id: row.get(0)?,
                seq: row.get(1)?,
                score: row.get(2)?,
                snippet: row.get(3)?,
            })
        })
        .map_err(storage)?;
    let hits = rows
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    let last_event_id = current_last_event_id(&conn, &board_id)?;
    Ok(SearchResults {
        hits,
        meta: sqlite_search_meta(last_event_id, stale, indexed_last_event_id),
    })
}

pub fn search_index_status(path: impl AsRef<Path>, board: &str) -> Result<SearchIndexStatus> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let last_event_id = current_last_event_id(&conn, &board_id)?;
    #[cfg(feature = "tantivy-backend")]
    {
        let index_path = task_index_path(path.as_ref());
        let state = read_search_index_state(&conn, &board_id)?;
        let indexed_last_event_id = state.as_ref().and_then(|state| state.last_event_id);
        if kanban_search::tantivy_backend::task_index_exists(&index_path) {
            let metadata =
                match kanban_search::tantivy_backend::validate_task_index(&index_path, &board_id) {
                    Ok(metadata) => metadata,
                    Err(err) if err.is_fallback_eligible() => {
                        return Ok(degraded_search_index_status(
                            last_event_id,
                            indexed_last_event_id,
                            &index_path,
                            &err,
                        ));
                    }
                    Err(err) => return Err(search_storage(err)),
                };
            let contract = search_index_contract(indexed_last_event_id, &metadata);
            let indexed = contract.indexed_last_event_id;
            let lag = search_lag(last_event_id, indexed);
            let dirty = state.as_ref().is_some_and(|state| state.dirty);
            if contract.mismatch {
                return Ok(mismatched_search_index_status(
                    last_event_id,
                    indexed,
                    &index_path,
                    Some(metadata.index_version),
                    indexed_last_event_id,
                    metadata.last_event_id,
                ));
            }
            if search_index_ahead(last_event_id, indexed) {
                return Ok(search_index_ahead_status(
                    last_event_id,
                    indexed,
                    &index_path,
                    Some(metadata.index_version),
                ));
            }
            return Ok(SearchIndexStatus {
                backend: "tantivy".to_owned(),
                derived_index: true,
                stale: dirty || lag > 0,
                index_version: Some(metadata.index_version),
                last_event_id: indexed,
                index_lag_events: Some(lag),
                message: state
                    .and_then(|state| state.message)
                    .unwrap_or_else(|| format!("Tantivy task index at {}", index_path.display())),
            });
        }
    }
    Ok(SearchIndexStatus {
        backend: "sqlite".to_owned(),
        derived_index: false,
        stale: false,
        index_version: None,
        last_event_id,
        index_lag_events: Some(0),
        message: "SQLite fallback search is active; no derived index exists yet".to_owned(),
    })
}

pub fn rebuild_search_index(path: impl AsRef<Path>, board: &str) -> Result<SearchIndexStatus> {
    #[cfg(feature = "tantivy-backend")]
    {
        let path_ref = path.as_ref();
        let conn = connect_file(path_ref)?;
        let board_id = board_id(&conn, board)?;
        let last_event_id = current_last_event_id(&conn, &board_id)?;
        let documents = task_search_documents(&conn, &board_id)?;
        let index_path = task_index_path(path_ref);
        let metadata = match kanban_search::tantivy_backend::rebuild_task_index(
            &index_path,
            &board_id,
            last_event_id,
            &documents,
        ) {
            Ok(metadata) => metadata,
            Err(error) => {
                mark_derived_store_failure(
                    &conn,
                    TANTIVY_TASKS_STORE,
                    &board_id,
                    &error.to_string(),
                    SystemClock.now_ms(),
                )?;
                return Err(search_storage(error));
            }
        };
        let now = SystemClock.now_ms();
        write_search_index_state(
            &conn,
            &default_search_index_state(
                &board_id,
                metadata.last_event_id,
                false,
                Some(format!(
                    "Rebuilt Tantivy task index at {}",
                    index_path.display()
                )),
                now,
            ),
        )?;
        mark_derived_store_success(
            &conn,
            TANTIVY_TASKS_STORE,
            &board_id,
            metadata.last_event_id,
            true,
            now,
        )?;
        Ok(SearchIndexStatus {
            backend: "tantivy".to_owned(),
            derived_index: true,
            stale: false,
            index_version: Some(metadata.index_version),
            last_event_id: metadata.last_event_id,
            index_lag_events: Some(0),
            message: format!("Rebuilt Tantivy task index at {}", index_path.display()),
        })
    }
    #[cfg(not(feature = "tantivy-backend"))]
    {
        search_index_status(path, board)
    }
}

pub fn sync_search_index(path: impl AsRef<Path>, board: &str) -> Result<SearchIndexStatus> {
    #[cfg(feature = "tantivy-backend")]
    {
        let path_ref = path.as_ref();
        let conn = connect_file(path_ref)?;
        let board_id = board_id(&conn, board)?;
        let index_path = task_index_path(path_ref);
        if !kanban_search::tantivy_backend::task_index_exists(&index_path) {
            return rebuild_search_index(path_ref, board);
        }
        let metadata =
            match kanban_search::tantivy_backend::validate_task_index(&index_path, &board_id) {
                Ok(metadata) => metadata,
                Err(error) => {
                    mark_derived_store_failure(
                        &conn,
                        TANTIVY_TASKS_STORE,
                        &board_id,
                        &error.to_string(),
                        SystemClock.now_ms(),
                    )?;
                    return Err(search_storage(error));
                }
            };
        let state = read_search_index_state(&conn, &board_id)?;
        let state_last_event_id = state.as_ref().and_then(|state| state.last_event_id);
        let contract = search_index_contract(state_last_event_id, &metadata);
        let indexed_last_event_id = contract.indexed_last_event_id;
        let current_last_event_id = current_last_event_id(&conn, &board_id)?;
        if contract.mismatch || search_index_ahead(current_last_event_id, indexed_last_event_id) {
            return rebuild_search_index(path_ref, board);
        }
        let lag = search_lag(current_last_event_id, indexed_last_event_id);
        if lag == 0 && state.as_ref().is_some_and(|state| !state.dirty) {
            return search_index_status(path_ref, board);
        }

        let now = SystemClock.now_ms();
        write_search_index_state(
            &conn,
            &default_search_index_state(
                &board_id,
                indexed_last_event_id,
                true,
                Some("Tantivy task index sync is in progress".to_owned()),
                now,
            ),
        )?;
        mark_derived_store_dirty(&conn, TANTIVY_TASKS_STORE, now)?;

        let affected_task_ids =
            affected_task_ids_since(&conn, &board_id, indexed_last_event_id.unwrap_or(0))?;
        let documents = task_search_documents_for_task_ids(&conn, &board_id, &affected_task_ids)?;
        let metadata = match kanban_search::tantivy_backend::sync_task_index(
            &index_path,
            &board_id,
            current_last_event_id,
            &documents,
            &affected_task_ids,
        ) {
            Ok(metadata) => metadata,
            Err(error) => {
                mark_derived_store_failure(
                    &conn,
                    TANTIVY_TASKS_STORE,
                    &board_id,
                    &error.to_string(),
                    SystemClock.now_ms(),
                )?;
                return Err(search_storage(error));
            }
        };
        write_search_index_state(
            &conn,
            &default_search_index_state(
                &board_id,
                metadata.last_event_id,
                false,
                Some(format!(
                    "Synced Tantivy task index at {} ({} affected task(s))",
                    index_path.display(),
                    affected_task_ids.len()
                )),
                SystemClock.now_ms(),
            ),
        )?;
        mark_derived_store_success(
            &conn,
            TANTIVY_TASKS_STORE,
            &board_id,
            metadata.last_event_id,
            false,
            SystemClock.now_ms(),
        )?;
        Ok(SearchIndexStatus {
            backend: "tantivy".to_owned(),
            derived_index: true,
            stale: false,
            index_version: Some(metadata.index_version),
            last_event_id: metadata.last_event_id,
            index_lag_events: Some(0),
            message: format!(
                "Synced Tantivy task index at {} ({} affected task(s))",
                index_path.display(),
                affected_task_ids.len()
            ),
        })
    }
    #[cfg(not(feature = "tantivy-backend"))]
    {
        search_index_status(path, board)
    }
}

#[cfg(feature = "tantivy-backend")]
fn degraded_search_index_status(
    current_last_event_id: Option<i64>,
    indexed_last_event_id: Option<i64>,
    index_path: &Path,
    err: &kanban_search::tantivy_backend::TantivyTaskIndexError,
) -> SearchIndexStatus {
    SearchIndexStatus {
        backend: "sqlite".to_owned(),
        derived_index: true,
        stale: true,
        index_version: None,
        last_event_id: indexed_last_event_id,
        index_lag_events: Some(search_lag(current_last_event_id, indexed_last_event_id)),
        message: format!(
            "Tantivy task index at {} is degraded ({:?}: {}); SQLite fallback search is active",
            index_path.display(),
            err.kind(),
            err
        ),
    }
}

#[cfg(feature = "tantivy-backend")]
fn search_index_ahead_status(
    current_last_event_id: Option<i64>,
    indexed_last_event_id: Option<i64>,
    index_path: &Path,
    index_version: Option<String>,
) -> SearchIndexStatus {
    SearchIndexStatus {
        backend: "sqlite".to_owned(),
        derived_index: true,
        stale: true,
        index_version,
        last_event_id: indexed_last_event_id,
        index_lag_events: Some(search_lag(current_last_event_id, indexed_last_event_id)),
        message: format!(
            "Tantivy task index at {} is ahead of the database (indexed last_event_id {:?}, current last_event_id {:?}); SQLite fallback search is active until rebuild",
            index_path.display(),
            indexed_last_event_id,
            current_last_event_id
        ),
    }
}

#[cfg(feature = "tantivy-backend")]
fn mismatched_search_index_status(
    current_last_event_id: Option<i64>,
    indexed_last_event_id: Option<i64>,
    index_path: &Path,
    index_version: Option<String>,
    state_last_event_id: Option<i64>,
    metadata_last_event_id: Option<i64>,
) -> SearchIndexStatus {
    SearchIndexStatus {
        backend: "sqlite".to_owned(),
        derived_index: true,
        stale: true,
        index_version,
        last_event_id: indexed_last_event_id,
        index_lag_events: Some(search_lag(current_last_event_id, indexed_last_event_id)),
        message: format!(
            "Tantivy task index at {} has mismatched search state and metadata watermarks (state last_event_id {:?}, metadata last_event_id {:?}); SQLite fallback search is active until rebuild",
            index_path.display(),
            state_last_event_id,
            metadata_last_event_id
        ),
    }
}

#[cfg(feature = "tantivy-backend")]
#[derive(Debug, Clone, Copy)]
struct SearchIndexContract {
    indexed_last_event_id: Option<i64>,
    mismatch: bool,
}

#[cfg(feature = "tantivy-backend")]
fn search_index_contract(
    state_last_event_id: Option<i64>,
    metadata: &kanban_search::tantivy_backend::TantivyIndexMetadata,
) -> SearchIndexContract {
    let metadata_last_event_id = metadata.last_event_id;
    let mismatch = matches!(
        (state_last_event_id, metadata_last_event_id),
        (Some(state), Some(metadata)) if state != metadata
    );
    SearchIndexContract {
        indexed_last_event_id: max_event_id(state_last_event_id, metadata_last_event_id),
        mismatch,
    }
}

#[cfg(feature = "tantivy-backend")]
fn max_event_id(left: Option<i64>, right: Option<i64>) -> Option<i64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

#[cfg(feature = "tantivy-backend")]
fn tantivy_literal_sqlite_fallback_required(query: &SearchQuery) -> bool {
    query.q.as_deref().map(str::trim).is_some_and(|q| {
        q.chars().any(|ch| {
            matches!(
                ch,
                '"' | '+'
                    | '-'
                    | '!'
                    | '('
                    | ')'
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | '^'
                    | '~'
                    | '*'
                    | '?'
                    | ':'
                    | '\\'
                    | '/'
                    | '&'
                    | '|'
                    | '%'
                    | '_'
            )
        })
    })
}

fn sqlite_search_meta(
    current_last_event_id: Option<i64>,
    stale: bool,
    indexed_last_event_id: Option<i64>,
) -> SearchMeta {
    SearchMeta {
        backend: "sqlite".to_owned(),
        stale,
        index_version: None,
        last_event_id: if stale {
            indexed_last_event_id
        } else {
            current_last_event_id
        },
        index_lag_events: Some(if stale {
            search_lag(current_last_event_id, indexed_last_event_id)
        } else {
            0
        }),
    }
}

pub fn get_task(path: impl AsRef<Path>, board: &str, task_ref: &str) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    resolve_task(&conn, &board_id, task_ref)
}

pub fn get_task_by_id_global(path: impl AsRef<Path>, task_id: &str) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id_for_task(&conn, task_id)?;
    get_task_by_id(&conn, &board_id, task_id)
}

pub fn update_task_by_id(
    path: impl AsRef<Path>,
    actor: &str,
    task_id: &str,
    patch: TaskPatch,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id_for_task(&conn, task_id)?;
    drop(conn);
    update_task(path, &board_id, actor, task_id, patch)
}

pub fn set_task_retry_policy_by_id(
    path: impl AsRef<Path>,
    actor: &str,
    task_id: &str,
    max_retries: Option<i64>,
) -> Result<TaskRecord> {
    if max_retries.is_some_and(|value| value <= 0) {
        return Err(KanbanError::InvalidInput(
            "max_retries must be a positive integer".into(),
        ));
    }
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id_for_task(&conn, task_id)?;
    with_immediate_tx(&conn, || {
        let changed = conn
            .execute(
                "UPDATE tasks SET max_retries=?1, updated_at=?2, lock_version=lock_version+1 WHERE id=?3 AND board_id=?4",
                params![max_retries, now, task_id, board_id],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(KanbanError::InvalidTransition(
                "retry policy update failed".into(),
            ));
        }
        insert_event(
            &conn,
            &board_id,
            Some(task_id),
            None,
            "task.retry_policy.updated",
            actor,
            &json!({ "max_retries": max_retries }).to_string(),
            now,
        )?;
        get_task_by_id(&conn, &board_id, task_id)
    })
}

pub fn promote_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    with_immediate_tx(&conn, || {
        let task = resolve_task(&conn, &board_id, task_ref)?;
        if !matches!(task.status, TaskStatus::Todo | TaskStatus::Scheduled) {
            return Err(KanbanError::InvalidTransition(format!(
                "cannot promote from {}",
                task.status.as_str()
            )));
        }
        if task.status == TaskStatus::Scheduled && task.scheduled_at.is_some_and(|t| t > now) {
            return Err(KanbanError::InvalidTransition(
                "scheduled_at is in the future".into(),
            ));
        }
        let target = recompute_ready_status(&conn, &task, now)?;
        if target != TaskStatus::Ready {
            return Err(KanbanError::InvalidTransition(match target {
                TaskStatus::Todo => "dependency blocked".into(),
                TaskStatus::Scheduled => "scheduled_at is in the future".into(),
                TaskStatus::Triage => "task spec is incomplete".into(),
                _ => format!("cannot promote to {}", target.as_str()),
            }));
        }
        guarded_set_status(
            &conn,
            &board_id,
            &task,
            TaskStatus::Ready,
            actor,
            "task.promoted",
            now,
        )?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn claim_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    ttl_ms: i64,
) -> Result<ClaimResult> {
    claim_task_with_profile(path, board, actor, task_ref, ttl_ms, "manual")
}

pub fn claim_task_with_profile(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    ttl_ms: i64,
    worker_profile: &str,
) -> Result<ClaimResult> {
    claim_task_with_profile_and_metadata(path, board, actor, task_ref, ttl_ms, worker_profile, "{}")
}

pub fn claim_task_with_profile_and_metadata(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    ttl_ms: i64,
    worker_profile: &str,
    metadata_json: &str,
) -> Result<ClaimResult> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    if !json_valid(&conn, metadata_json)? {
        return Err(KanbanError::InvalidInput(
            "metadata_json must be valid JSON".into(),
        ));
    }
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    claim_task_conn(
        &conn,
        &board_id,
        actor,
        &task.id,
        ttl_ms,
        worker_profile,
        metadata_json,
        now,
    )
}

#[allow(clippy::too_many_arguments)]
fn claim_task_conn(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    task_id: &str,
    ttl_ms: i64,
    profile: &str,
    metadata_json: &str,
    now: i64,
) -> Result<ClaimResult> {
    conn.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
    match claim_task_in_current_tx(
        conn,
        board_id,
        actor,
        task_id,
        ttl_ms,
        profile,
        metadata_json,
        now,
    ) {
        Ok(claim) => {
            conn.execute_batch("COMMIT").map_err(storage)?;
            Ok(claim)
        }
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

fn claim_next_ready_conn(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    worker_profile: &str,
    ttl_ms: i64,
    now: i64,
) -> Result<Option<ClaimResult>> {
    conn.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
    let selected = conn
        .query_row(
            "SELECT id FROM tasks WHERE board_id=?1 AND status='ready' AND claim_token IS NULL AND (assignee IS NULL OR assignee=?2) AND NOT EXISTS (SELECT 1 FROM task_dependencies d JOIN tasks p ON p.id=d.parent_task_id WHERE d.child_task_id=tasks.id AND p.status != 'done') ORDER BY priority DESC, created_at ASC LIMIT 1",
            params![board_id, worker_profile],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage);
    let result = match selected {
        Ok(Some(task_id)) => claim_task_in_current_tx(
            conn,
            board_id,
            actor,
            &task_id,
            ttl_ms,
            worker_profile,
            "{}",
            now,
        )
        .map(Some),
        Ok(None) => Ok(None),
        Err(err) => Err(err),
    };
    match result {
        Ok(claim) => {
            conn.execute_batch("COMMIT").map_err(storage)?;
            Ok(claim)
        }
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn claim_task_in_current_tx(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    task_id: &str,
    ttl_ms: i64,
    profile: &str,
    metadata_json: &str,
    now: i64,
) -> Result<ClaimResult> {
    let task = get_task_by_id(conn, board_id, task_id)?;
    if task.status != TaskStatus::Ready || task.claim_token.is_some() {
        return Err(KanbanError::InvalidTransition(
            "task is not claimable".into(),
        ));
    }
    ensure_dependencies_done(conn, task_id)?;
    let token = new_typed_id("claim");
    let run_id = new_run_id();
    let expires = now + ttl_ms;
    let changed = conn.execute(
        "UPDATE tasks SET status='running', claim_token=?1, claim_owner=?2, claim_expires_at=?3, last_heartbeat_at=?4, started_at=COALESCE(started_at, ?4), updated_at=?4, lock_version=lock_version+1 WHERE id=?5 AND board_id=?6 AND status='ready' AND claim_token IS NULL AND NOT EXISTS (SELECT 1 FROM task_dependencies d JOIN tasks p ON p.id=d.parent_task_id WHERE d.child_task_id=tasks.id AND p.status != 'done')",
        params![token, actor, expires, now, task_id, board_id],
    ).map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition("claim conflict".into()));
    }
    conn.execute(
        "INSERT INTO task_runs(id, board_id, task_id, status, worker_profile, claim_token, claim_owner, claim_expires_at, started_at, last_heartbeat_at, metadata_json) VALUES (?1, ?2, ?3, 'running', ?4, ?5, ?6, ?7, ?8, ?8, ?9)",
        params![run_id, board_id, task_id, profile, token, actor, expires, now, metadata_json],
    ).map_err(storage)?;
    conn.execute(
        "UPDATE tasks SET current_run_id=?1 WHERE id=?2",
        params![run_id, task_id],
    )
    .map_err(storage)?;
    insert_event(
        conn,
        board_id,
        Some(task_id),
        Some(&run_id),
        "task.claimed",
        actor,
        &json!({
            "claim_owner": actor,
            "metadata": serde_json::from_str::<serde_json::Value>(metadata_json)
                .unwrap_or_else(|_| json!({})),
        })
        .to_string(),
        now,
    )?;
    Ok(ClaimResult {
        task: get_task_by_id(conn, board_id, task_id)?,
        claim_token: token,
        run_id,
    })
}

pub fn heartbeat_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: &str,
    ttl_ms: i64,
) -> Result<TaskRecord> {
    heartbeat_task_with_note(path, board, actor, task_ref, token, ttl_ms, None)
}

pub fn heartbeat_task_with_note(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: &str,
    ttl_ms: i64,
    note: Option<&str>,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    with_immediate_tx(&conn, || {
        heartbeat_task_conn(&conn, &board_id, actor, &task, token, ttl_ms, note, now)?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

#[allow(clippy::too_many_arguments)]
fn heartbeat_task_conn(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    task: &TaskRecord,
    token: &str,
    ttl_ms: i64,
    note: Option<&str>,
    now: i64,
) -> Result<()> {
    if task.status != TaskStatus::Running || task.claim_token.as_deref() != Some(token) {
        return Err(KanbanError::InvalidTransition(
            "heartbeat requires matching running claim".into(),
        ));
    }
    let expires = now + ttl_ms;
    let changed = conn
        .execute(
            "UPDATE tasks SET claim_expires_at=?1, last_heartbeat_at=?2, updated_at=?2, lock_version=lock_version+1 WHERE id=?3 AND board_id=?4 AND status='running' AND claim_token=?5 AND current_run_id IS ?6",
            params![expires, now, task.id, board_id, token, task.current_run_id],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition(
            "heartbeat requires matching running claim".into(),
        ));
    }
    if let Some(run_id) = &task.current_run_id {
        let changed = conn
            .execute(
                "UPDATE task_runs SET claim_expires_at=?1, last_heartbeat_at=?2 WHERE id=?3 AND board_id=?4 AND task_id=?5 AND status='running' AND claim_token=?6",
                params![expires, now, run_id, board_id, task.id, token],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(KanbanError::InvalidTransition(
                "heartbeat requires matching running run".into(),
            ));
        }
        insert_event(
            conn,
            board_id,
            Some(&task.id),
            Some(run_id),
            "task.heartbeat",
            actor,
            &json!({ "note": note }).to_string(),
            now,
        )?;
    }
    Ok(())
}

pub fn complete_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: Option<&str>,
    force: bool,
) -> Result<TaskRecord> {
    complete_task_with_summary_and_result(path, board, actor, task_ref, token, force, None, None)
}

pub fn complete_task_with_summary(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: Option<&str>,
    force: bool,
    summary: Option<&str>,
) -> Result<TaskRecord> {
    complete_task_with_summary_and_result(path, board, actor, task_ref, token, force, summary, None)
}

#[allow(clippy::too_many_arguments)]
pub fn complete_task_with_summary_and_result(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: Option<&str>,
    force: bool,
    summary: Option<&str>,
    result_json: Option<&str>,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    let result_json_is_invalid = match result_json {
        Some(value) => !json_valid(&conn, value)?,
        None => false,
    };
    if result_json_is_invalid {
        return Err(KanbanError::InvalidInput(
            "result_json must be valid JSON".into(),
        ));
    }
    if task.status == TaskStatus::Running && !force && task.claim_token.as_deref() != token {
        return Err(KanbanError::InvalidTransition(
            "claim token mismatch".into(),
        ));
    }
    if !matches!(task.status, TaskStatus::Running | TaskStatus::Review) {
        return Err(KanbanError::InvalidTransition(
            "complete requires running or review".into(),
        ));
    }
    with_immediate_tx(&conn, || {
        finish_running(
            &conn,
            &board_id,
            &task,
            TaskStatus::Done,
            actor,
            "task.completed",
            "succeeded",
            0,
            None,
            None,
            summary,
            result_json,
            now,
        )?;
        promote_children(&conn, &board_id, actor, &task.id, now)?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn submit_review_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: Option<&str>,
    force: bool,
) -> Result<TaskRecord> {
    submit_review_task_with_summary(path, board, actor, task_ref, token, force, None)
}

pub fn submit_review_task_with_summary(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: Option<&str>,
    force: bool,
    summary: Option<&str>,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    if task.status != TaskStatus::Running {
        return Err(KanbanError::InvalidTransition(
            "review requires running".into(),
        ));
    }
    if !force && task.claim_token.as_deref() != token {
        return Err(KanbanError::InvalidTransition(
            "claim token mismatch".into(),
        ));
    }
    with_immediate_tx(&conn, || {
        finish_running(
            &conn,
            &board_id,
            &task,
            TaskStatus::Review,
            actor,
            "task.submitted_for_review",
            "succeeded",
            0,
            None,
            None,
            summary,
            None,
            now,
        )?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn block_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    reason: &str,
    token: Option<&str>,
    force: bool,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    if reason.trim().is_empty() {
        return Err(KanbanError::InvalidInput("block reason is required".into()));
    }
    if task.status == TaskStatus::Running && !force && task.claim_token.as_deref() != token {
        return Err(KanbanError::InvalidTransition(
            "claim token mismatch".into(),
        ));
    }
    if !matches!(
        task.status,
        TaskStatus::Triage
            | TaskStatus::Todo
            | TaskStatus::Scheduled
            | TaskStatus::Ready
            | TaskStatus::Running
            | TaskStatus::Review
    ) {
        return Err(KanbanError::InvalidTransition("cannot block task".into()));
    }
    with_immediate_tx(&conn, || {
        if task.status == TaskStatus::Running {
            finish_running(
                &conn,
                &board_id,
                &task,
                TaskStatus::Blocked,
                actor,
                "task.blocked",
                "failed",
                1,
                Some(reason),
                None,
                None,
                None,
                now,
            )?;
        } else {
            let changed = conn
                .execute(
                    "UPDATE tasks SET status='blocked', status_reason=?1, updated_at=?2, lock_version=lock_version+1 WHERE id=?3 AND board_id=?4 AND status=?5",
                    params![reason, now, task.id, board_id, task.status.as_str()],
                )
                .map_err(storage)?;
            if changed != 1 {
                return Err(KanbanError::InvalidTransition("cannot block task".into()));
            }
            let payload = json!({ "reason": reason }).to_string();
            insert_event(
                &conn,
                &board_id,
                Some(&task.id),
                None,
                "task.blocked",
                actor,
                &payload,
                now,
            )?;
        }
        Ok(())
    })?;
    get_task_by_id(&conn, &board_id, &task.id)
}

pub fn unblock_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    with_immediate_tx(&conn, || {
        let task = resolve_task(&conn, &board_id, task_ref)?;
        if task.status != TaskStatus::Blocked {
            return Err(KanbanError::InvalidTransition(
                "unblock requires blocked".into(),
            ));
        }
        let target = recompute_ready_status(&conn, &task, now)?;
        guarded_set_status_with_reason(
            &conn,
            &board_id,
            &task,
            StatusUpdate {
                status: target,
                status_reason: None,
                actor,
                event: "task.unblocked",
                now,
            },
        )?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn reclaim_expired(path: impl AsRef<Path>, board: &str, actor: &str) -> Result<usize> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let expired: Vec<TaskRecord> = query_tasks(&conn, &board_id)?
        .into_iter()
        .filter(|t| t.status == TaskStatus::Running && t.claim_expires_at.is_some_and(|x| x <= now))
        .collect();
    let mut count = 0;
    for task in expired {
        let reclaimed = with_immediate_tx(&conn, || {
            let fresh = get_task_by_id(&conn, &board_id, &task.id)?;
            let tx_now = SystemClock.now_ms();
            if fresh.status != TaskStatus::Running
                || fresh
                    .claim_expires_at
                    .is_none_or(|expires| expires > tx_now)
            {
                return Ok(false);
            }
            retry_running_task(
                &conn,
                &board_id,
                &fresh,
                actor,
                "expired",
                None,
                "claim expired",
                tx_now,
                Some(tx_now),
            )?;
            Ok(true)
        })?;
        if reclaimed {
            count += 1;
        }
    }
    Ok(count)
}

pub fn reclaim_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    force: bool,
) -> Result<TaskRecord> {
    reclaim_task_to(path, board, actor, task_ref, force, TaskStatus::Ready, None)
}

pub fn reclaim_task_to(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    force: bool,
    to_status: TaskStatus,
    reason: Option<&str>,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    if !matches!(to_status, TaskStatus::Ready | TaskStatus::Blocked) {
        return Err(KanbanError::InvalidInput(
            "reclaim to_status must be ready or blocked".into(),
        ));
    }
    if to_status == TaskStatus::Blocked && reason.is_none_or(|value| value.trim().is_empty()) {
        return Err(KanbanError::InvalidInput(
            "reclaim reason is required when to_status is blocked".into(),
        ));
    }
    let task = resolve_task(&conn, &board_id, task_ref)?;
    with_immediate_tx(&conn, || {
        let fresh = get_task_by_id(&conn, &board_id, &task.id)?;
        let tx_now = SystemClock.now_ms();
        if fresh.status != TaskStatus::Running {
            return Err(KanbanError::InvalidTransition(
                "reclaim requires running".into(),
            ));
        }
        if !force
            && fresh
                .claim_expires_at
                .is_none_or(|expires| expires > tx_now)
        {
            return Err(KanbanError::InvalidTransition(
                "reclaim requires expired claim or force".into(),
            ));
        }
        let new_retry_count = fresh.retry_count + 1;
        let max_retries_reached = fresh
            .max_retries
            .is_some_and(|max_retries| new_retry_count >= max_retries);
        let effective_status = if max_retries_reached {
            TaskStatus::Blocked
        } else {
            to_status
        };
        let default_reason = if max_retries_reached {
            "max retries reached"
        } else if force {
            "force reclaimed"
        } else {
            "claim expired"
        };
        let effective_reason = reason.unwrap_or(default_reason);
        reclaim_running_task(
            &conn,
            &board_id,
            &fresh,
            actor,
            if force { "canceled" } else { "expired" },
            effective_reason,
            effective_status,
            tx_now,
            (!force).then_some(tx_now),
        )?;
        get_task_by_id(&conn, &board_id, &fresh.id)
    })
}

pub fn archive_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    force: bool,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    if task.status == TaskStatus::Running && !force {
        return Err(KanbanError::InvalidTransition(
            "cannot archive running without force".into(),
        ));
    }
    with_immediate_tx(&conn, || {
        if task.status == TaskStatus::Running {
            let run_id = task.current_run_id.as_deref().ok_or_else(|| {
                KanbanError::InvalidTransition("force archive requires active run".into())
            })?;
            let changed = conn
            .execute(
                "UPDATE task_runs SET status='canceled', finished_at=?1, error=COALESCE(error, ?2) WHERE id=?3 AND board_id=?4 AND task_id=?5 AND status='running'",
                params![now, "force archived", run_id, board_id, task.id],
            )
            .map_err(storage)?;
            if changed != 1 {
                return Err(KanbanError::InvalidTransition(
                    "force archive requires active running run".into(),
                ));
            }
        }
        let changed = conn
            .execute(
                "UPDATE tasks SET status='archived', archived_at=?1, claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, updated_at=?1, lock_version=lock_version+1 WHERE id=?2 AND board_id=?3 AND status=?4",
                params![now, task.id, board_id, task.status.as_str()],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(KanbanError::InvalidTransition("cannot archive task".into()));
        }
        insert_event(
            &conn,
            &board_id,
            Some(&task.id),
            task.current_run_id.as_deref(),
            "task.archived",
            actor,
            "{}",
            now,
        )?;
        Ok(())
    })?;
    get_task_by_id(&conn, &board_id, &task.id)
}

pub fn add_dependency(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    child_ref: &str,
) -> Result<()> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    with_immediate_tx(&conn, || {
        let parent = resolve_task(&conn, &board_id, parent_ref)?;
        let child = resolve_task(&conn, &board_id, child_ref)?;
        add_dependency_in_current_tx(&conn, &board_id, actor, &parent, &child, now)
    })
}

fn add_dependency_in_current_tx(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    parent: &TaskRecord,
    child: &TaskRecord,
    now: i64,
) -> Result<()> {
    if parent.id == child.id {
        return Err(KanbanError::InvalidInput(
            "dependency cannot point to itself".into(),
        ));
    }
    if has_path(conn, &child.id, &parent.id)? {
        return Err(KanbanError::InvalidInput(
            "dependency cycle detected".into(),
        ));
    }
    if child.status == TaskStatus::Running && parent.status != TaskStatus::Done {
        return Err(KanbanError::InvalidTransition(
            "cannot add incomplete dependency to running task".into(),
        ));
    }
    conn.execute(
        "INSERT OR IGNORE INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![board_id, parent.id, child.id, now],
    )
    .map_err(storage)?;
    upsert_dependency_relation(conn, &parent.id, &child.id, now)?;
    let fresh_child = get_task_by_id(conn, board_id, &child.id)?;
    if is_active_recomputable_status(fresh_child.status) {
        let target = recompute_ready_status(conn, &fresh_child, now)?;
        if target != fresh_child.status {
            guarded_set_status(
                conn,
                board_id,
                &fresh_child,
                target,
                actor,
                if target == TaskStatus::Ready {
                    "task.promoted"
                } else {
                    "task.recomputed"
                },
                now,
            )?;
        }
    }
    let payload = json!({ "parent_task_id": parent.id }).to_string();
    insert_event(
        conn,
        board_id,
        Some(&child.id),
        None,
        "dependency.added",
        actor,
        &payload,
        now,
    )
}

pub fn remove_dependency(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    parent_ref: &str,
    child_ref: &str,
) -> Result<()> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let parent = resolve_task(&conn, &board_id, parent_ref)?;
    let child = resolve_task(&conn, &board_id, child_ref)?;
    with_immediate_tx(&conn, || {
        conn.execute(
            "DELETE FROM task_dependencies WHERE parent_task_id=?1 AND child_task_id=?2",
            params![parent.id, child.id],
        )
        .map_err(storage)?;
        delete_dependency_relation(&conn, &parent.id, &child.id)?;
        let fresh_child = get_task_by_id(&conn, &board_id, &child.id)?;
        if matches!(
            fresh_child.status,
            TaskStatus::Triage | TaskStatus::Todo | TaskStatus::Scheduled | TaskStatus::Ready
        ) {
            let target = recompute_ready_status(&conn, &fresh_child, now)?;
            if target != fresh_child.status {
                guarded_set_status(
                    &conn,
                    &board_id,
                    &fresh_child,
                    target,
                    actor,
                    if target == TaskStatus::Ready {
                        "task.promoted"
                    } else {
                        "task.recomputed"
                    },
                    now,
                )?;
            }
        }
        let payload = json!({ "parent_task_id": parent.id }).to_string();
        insert_event(
            &conn,
            &board_id,
            Some(&child.id),
            None,
            "dependency.removed",
            actor,
            &payload,
            now,
        )
    })
}

pub fn list_dependencies(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: &str,
) -> Result<Vec<(String, String)>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    let mut stmt = conn.prepare("SELECT parent_task_id, child_task_id FROM task_dependencies WHERE parent_task_id=?1 OR child_task_id=?1 ORDER BY created_at ASC").map_err(storage)?;
    let rows = stmt
        .query_map([task.id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

pub fn list_events(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: Option<&str>,
) -> Result<Vec<EventRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task_id = task_ref
        .map(|r| resolve_task(&conn, &board_id, r).map(|t| t.id))
        .transpose()?;
    let sql = if task_id.is_some() {
        "SELECT id,event_id,task_id,run_id,kind,actor,payload_json,created_at FROM task_events WHERE board_id=?1 AND task_id=?2 ORDER BY id ASC"
    } else {
        "SELECT id,event_id,task_id,run_id,kind,actor,payload_json,created_at FROM task_events WHERE board_id=?1 ORDER BY id ASC"
    };
    let mut stmt = conn.prepare(sql).map_err(storage)?;
    let mut out = Vec::new();
    if let Some(task_id) = task_id {
        let rows = stmt
            .query_map(params![board_id, task_id], event_from_row)
            .map_err(storage)?;
        for row in rows {
            out.push(row.map_err(storage)?);
        }
    } else {
        let rows = stmt
            .query_map(params![board_id], event_from_row)
            .map_err(storage)?;
        for row in rows {
            out.push(row.map_err(storage)?);
        }
    }
    Ok(out)
}

pub fn create_comment(
    path: impl AsRef<Path>,
    task_ref: &str,
    author: &str,
    body: &str,
    kind: Option<&str>,
) -> Result<CommentRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = board_id_for_task(&conn, task_ref)?;
        let task = resolve_task(&conn, &board_id, task_ref)?;
        let author = author.trim();
        if author.is_empty() {
            return Err(KanbanError::InvalidInput(
                "comment author is required".into(),
            ));
        }
        let body = body.trim();
        if body.is_empty() {
            return Err(KanbanError::InvalidInput("comment body is required".into()));
        }
        let kind = kind.unwrap_or("text").trim();
        if !matches!(kind, "text" | "system" | "worker") {
            return Err(KanbanError::InvalidInput("invalid comment kind".into()));
        }
        let id = new_typed_id("c");
        conn.execute(
            "INSERT INTO task_comments(id, board_id, task_id, author, body, kind, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, board_id, task.id, author, body, kind, now],
        )
        .map_err(storage)?;
        insert_event(
            &conn,
            &board_id,
            Some(&task.id),
            None,
            "task.comment.created",
            author,
            &json!({"comment_id": id, "kind": kind}).to_string(),
            now,
        )?;
        Ok(CommentRecord {
            id,
            board_id,
            task_id: task.id,
            author: author.to_owned(),
            body: body.to_owned(),
            kind: kind.to_owned(),
            created_at: now,
        })
    })
}

pub fn list_comments(path: impl AsRef<Path>, task_ref: &str) -> Result<Vec<CommentRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id_for_task(&conn, task_ref)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, board_id, task_id, author, body, kind, created_at \
             FROM task_comments WHERE board_id=?1 AND task_id=?2 ORDER BY created_at ASC, id ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map(params![board_id, task.id], comment_from_row)
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

pub fn list_events_after(
    path: impl AsRef<Path>,
    board: &str,
    options: EventListOptions,
) -> Result<Vec<EventRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task_id = options
        .task_ref
        .as_deref()
        .map(|r| resolve_task(&conn, &board_id, r).map(|t| t.id))
        .transpose()?;
    let mut params = vec![Value::Text(board_id), Value::Integer(options.after)];
    let mut where_sql = "WHERE board_id=? AND id>?".to_owned();
    if let Some(task_id) = task_id {
        where_sql.push_str(" AND task_id=?");
        params.push(Value::Text(task_id));
    }
    params.push(Value::Integer(options.limit as i64));
    let sql = format!(
        "SELECT id,event_id,task_id,run_id,kind,actor,payload_json,created_at FROM task_events {where_sql} ORDER BY id ASC LIMIT ?"
    );
    let mut stmt = conn.prepare(&sql).map_err(storage)?;
    let rows = stmt
        .query_map(params_from_iter(params.iter()), event_from_row)
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

pub fn list_runs(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: Option<&str>,
) -> Result<Vec<RunRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task_id = task_ref
        .map(|r| resolve_task(&conn, &board_id, r).map(|t| t.id))
        .transpose()?;
    let sql = if task_id.is_some() {
        "SELECT id,task_id,status,worker_profile,worker_pid,claim_token,claim_owner,started_at,finished_at,exit_code,summary,error,log_path,metadata_json FROM task_runs WHERE board_id=?1 AND task_id=?2 ORDER BY started_at DESC"
    } else {
        "SELECT id,task_id,status,worker_profile,worker_pid,claim_token,claim_owner,started_at,finished_at,exit_code,summary,error,log_path,metadata_json FROM task_runs WHERE board_id=?1 ORDER BY started_at DESC"
    };
    let mut stmt = conn.prepare(sql).map_err(storage)?;
    let mut out = Vec::new();
    if let Some(task_id) = task_id {
        let rows = stmt
            .query_map(params![board_id, task_id], run_from_row)
            .map_err(storage)?;
        for row in rows {
            out.push(row.map_err(storage)?);
        }
    } else {
        let rows = stmt
            .query_map(params![board_id], run_from_row)
            .map_err(storage)?;
        for row in rows {
            out.push(row.map_err(storage)?);
        }
    }
    Ok(out)
}

pub fn get_run_by_id_global(path: impl AsRef<Path>, run_id: &str) -> Result<RunRecord> {
    let conn = connect_file(path.as_ref())?;
    conn.query_row(
        "SELECT id,task_id,status,worker_profile,worker_pid,claim_token,claim_owner,started_at,finished_at,exit_code,summary,error,log_path,metadata_json FROM task_runs WHERE id=?1",
        [run_id],
        run_from_row,
    )
    .optional()
    .map_err(storage)?
    .ok_or_else(|| KanbanError::NotFound(format!("run {run_id}")))
}

pub fn dispatch_once(
    path: impl AsRef<Path>,
    board: &str,
    options: DispatchOptions,
) -> Result<DispatchResult> {
    validate_dispatch_options(&options)?;
    let path = path.as_ref();
    reclaim_expired(path, board, &options.actor)?;
    let conn = connect_file(path)?;
    let board_id = board_id(&conn, board)?;
    let now = SystemClock.now_ms();
    promote_due_tasks(&conn, &board_id, &options.actor, now)?;
    let Some(claim) = claim_next_ready_conn(
        &conn,
        &board_id,
        &options.actor,
        &options.worker_profile,
        options.claim_ttl_ms,
        now,
    )?
    else {
        return Ok(DispatchResult {
            claimed: 0,
            task_id: None,
            run_id: None,
            exit_code: None,
        });
    };
    std::fs::create_dir_all(&options.log_dir).map_err(|e| KanbanError::Storage(e.to_string()))?;
    let log_path = options.log_dir.join(format!("{}.log", claim.run_id));
    let output = run_worker_with_heartbeat(path, board, &options, &claim, &log_path)?;
    let exit = output.status.code().unwrap_or(1);
    let fresh = get_task_by_id(&conn, &board_id, &claim.task.id)?;
    let target = if output.status.success() {
        options.on_success
    } else {
        options.on_failure
    };
    with_immediate_tx(&conn, || {
        match target {
            FinishPolicy::Done => {
                finish_running(
                    &conn,
                    &board_id,
                    &fresh,
                    TaskStatus::Done,
                    &options.actor,
                    "task.completed",
                    "succeeded",
                    exit,
                    None,
                    Some(&log_path),
                    None,
                    None,
                    SystemClock.now_ms(),
                )?;
                promote_children(
                    &conn,
                    &board_id,
                    &options.actor,
                    &fresh.id,
                    SystemClock.now_ms(),
                )?;
            }
            FinishPolicy::Review => {
                finish_running(
                    &conn,
                    &board_id,
                    &fresh,
                    TaskStatus::Review,
                    &options.actor,
                    "task.submitted_for_review",
                    "succeeded",
                    exit,
                    None,
                    Some(&log_path),
                    None,
                    None,
                    SystemClock.now_ms(),
                )?;
            }
            FinishPolicy::Blocked => {
                finish_running(
                    &conn,
                    &board_id,
                    &fresh,
                    TaskStatus::Blocked,
                    &options.actor,
                    "task.blocked",
                    "failed",
                    exit,
                    Some("worker failed"),
                    Some(&log_path),
                    None,
                    None,
                    SystemClock.now_ms(),
                )?;
            }
            FinishPolicy::Ready => {
                retry_running_task(
                    &conn,
                    &board_id,
                    &fresh,
                    &options.actor,
                    "failed",
                    Some(exit),
                    "worker failed",
                    SystemClock.now_ms(),
                    None,
                )?;
                conn.execute(
                    "UPDATE task_runs SET log_path=?1 WHERE id=?2",
                    params![log_path.to_string_lossy(), claim.run_id],
                )
                .map_err(storage)?;
            }
        }
        Ok(())
    })?;
    Ok(DispatchResult {
        claimed: 1,
        task_id: Some(claim.task.id),
        run_id: Some(claim.run_id),
        exit_code: Some(exit),
    })
}

fn promote_due_tasks(conn: &Connection, board_id: &str, actor: &str, now: i64) -> Result<usize> {
    let candidates = query_tasks(conn, board_id)?
        .into_iter()
        .filter(|task| matches!(task.status, TaskStatus::Todo | TaskStatus::Scheduled))
        .collect::<Vec<_>>();
    let mut promoted = 0;
    for task in candidates {
        let was_promoted = with_immediate_tx(conn, || {
            let fresh = get_task_by_id(conn, board_id, &task.id)?;
            if !matches!(fresh.status, TaskStatus::Todo | TaskStatus::Scheduled) {
                return Ok(false);
            }
            if recompute_ready_status(conn, &fresh, now)? != TaskStatus::Ready {
                return Ok(false);
            }
            guarded_set_status(
                conn,
                board_id,
                &fresh,
                TaskStatus::Ready,
                actor,
                "task.promoted",
                now,
            )?;
            Ok(true)
        })?;
        if was_promoted {
            promoted += 1;
        }
    }
    Ok(promoted)
}

struct WorkerOutput {
    status: ExitStatus,
}

fn validate_dispatch_options(options: &DispatchOptions) -> Result<()> {
    if options.claim_ttl_ms <= 0 {
        return Err(KanbanError::InvalidInput(
            "claim_ttl_ms must be positive".into(),
        ));
    }
    if options.heartbeat_interval_ms <= 0 {
        return Err(KanbanError::InvalidInput(
            "heartbeat_interval_ms must be positive".into(),
        ));
    }
    if options.heartbeat_interval_ms >= options.claim_ttl_ms {
        return Err(KanbanError::InvalidInput(
            "heartbeat_interval_ms must be less than claim_ttl_ms".into(),
        ));
    }
    Ok(())
}

fn run_worker_with_heartbeat(
    path: &Path,
    board: &str,
    options: &DispatchOptions,
    claim: &ClaimResult,
    log_path: &Path,
) -> Result<WorkerOutput> {
    let stdout = File::create(log_path).map_err(|e| KanbanError::Storage(e.to_string()))?;
    let stderr = stdout
        .try_clone()
        .map_err(|e| KanbanError::Storage(e.to_string()))?;
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&options.command)
        .env("KB_DB_PATH", path)
        .env("KB_BOARD_ID", &claim.task.board_id)
        .env("KB_BOARD_SLUG", board)
        .env("KB_TASK_ID", &claim.task.id)
        .env("KB_TASK_SEQ", claim.task.seq.to_string())
        .env("KB_TASK_TITLE", &claim.task.title)
        .env("KB_CLAIM_TOKEN", &claim.claim_token)
        .env("KB_RUN_ID", &claim.run_id)
        .env("KB_ACTOR", &options.actor)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|e| KanbanError::Storage(e.to_string()))?;

    let heartbeat_interval = Duration::from_millis(options.heartbeat_interval_ms as u64);
    let poll_interval = heartbeat_interval.min(Duration::from_millis(10));
    let mut elapsed_since_heartbeat = Duration::ZERO;
    loop {
        match child
            .try_wait()
            .map_err(|e| KanbanError::Storage(e.to_string()))?
        {
            Some(status) => return Ok(WorkerOutput { status }),
            None => {
                thread::sleep(poll_interval);
                elapsed_since_heartbeat += poll_interval;
                if elapsed_since_heartbeat < heartbeat_interval {
                    continue;
                }
                elapsed_since_heartbeat = Duration::ZERO;
                let conn = connect_file(path)?;
                let board_id = board_id(&conn, board)?;
                let task = get_task_by_id(&conn, &board_id, &claim.task.id)?;
                if let Err(err) = with_immediate_tx(&conn, || {
                    heartbeat_task_conn(
                        &conn,
                        &board_id,
                        &options.actor,
                        &task,
                        &claim.claim_token,
                        options.claim_ttl_ms,
                        None,
                        SystemClock.now_ms(),
                    )
                }) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(err);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn reclaim_running_task(
    conn: &Connection,
    board_id: &str,
    task: &TaskRecord,
    actor: &str,
    run_status: &str,
    reason: &str,
    target: TaskStatus,
    now: i64,
    expiry_guard: Option<i64>,
) -> Result<()> {
    if task.status != TaskStatus::Running
        || task.claim_token.is_none()
        || task.current_run_id.is_none()
    {
        return Err(KanbanError::InvalidTransition(
            "reclaim requires matching running claim".into(),
        ));
    }
    if let (Some(run_id), Some(token)) = (&task.current_run_id, &task.claim_token) {
        let changed = conn
            .execute(
                "UPDATE task_runs SET status=?1, finished_at=?2, error=?3 WHERE id=?4 AND board_id=?5 AND task_id=?6 AND status='running' AND claim_token=?7",
                params![run_status, now, reason, run_id, board_id, task.id, token],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(KanbanError::InvalidTransition(
                "reclaim requires matching running run".into(),
            ));
        }
    }
    let changed = conn
        .execute(
            "UPDATE tasks SET status=?1, status_reason=?2, claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, retry_count=retry_count+1, updated_at=?3, lock_version=lock_version+1 WHERE id=?4 AND board_id=?5 AND status='running' AND claim_token=?6 AND current_run_id=?7 AND (?8 IS NULL OR claim_expires_at <= ?8)",
            params![target.as_str(), (target == TaskStatus::Blocked).then_some(reason), now, task.id, board_id, task.claim_token, task.current_run_id, expiry_guard],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition(
            "reclaim requires matching running claim".into(),
        ));
    }
    let payload = json!({
        "retry_count": task.retry_count + 1,
        "max_retries": task.max_retries,
        "to_status": target.as_str(),
        "reason": reason,
    })
    .to_string();
    insert_event(
        conn,
        board_id,
        Some(&task.id),
        task.current_run_id.as_deref(),
        "task.reclaimed",
        actor,
        &payload,
        now,
    )
}

#[allow(clippy::too_many_arguments)]
fn retry_running_task(
    conn: &Connection,
    board_id: &str,
    task: &TaskRecord,
    actor: &str,
    run_status: &str,
    exit_code: Option<i32>,
    reason: &str,
    now: i64,
    expiry_guard: Option<i64>,
) -> Result<()> {
    if task.status != TaskStatus::Running
        || task.claim_token.is_none()
        || task.current_run_id.is_none()
    {
        return Err(KanbanError::InvalidTransition(
            "retry requires matching running claim".into(),
        ));
    }
    let new_retry_count = task.retry_count + 1;
    let blocked = task
        .max_retries
        .is_some_and(|max_retries| new_retry_count >= max_retries);
    let target = if blocked {
        TaskStatus::Blocked
    } else {
        TaskStatus::Ready
    };
    if let (Some(run_id), Some(token)) = (&task.current_run_id, &task.claim_token) {
        let changed = conn
            .execute(
                "UPDATE task_runs SET status=?1, finished_at=?2, exit_code=?3, error=?4 WHERE id=?5 AND board_id=?6 AND task_id=?7 AND status='running' AND claim_token=?8",
                params![run_status, now, exit_code, reason, run_id, board_id, task.id, token],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(KanbanError::InvalidTransition(
                "retry requires matching running run".into(),
            ));
        }
    }
    let changed = conn
        .execute(
            "UPDATE tasks SET status=?1, status_reason=?2, claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, retry_count=?3, updated_at=?4, lock_version=lock_version+1 WHERE id=?5 AND board_id=?6 AND status='running' AND claim_token=?7 AND current_run_id=?8 AND (?9 IS NULL OR claim_expires_at <= ?9)",
            params![target.as_str(), if blocked { Some(reason) } else { None }, new_retry_count, now, task.id, board_id, task.claim_token, task.current_run_id, expiry_guard],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition(
            "retry requires matching running claim".into(),
        ));
    }
    let payload = json!({
        "retry_count": new_retry_count,
        "max_retries": task.max_retries,
    })
    .to_string();
    insert_event(
        conn,
        board_id,
        Some(&task.id),
        task.current_run_id.as_deref(),
        if blocked {
            "task.blocked"
        } else {
            "task.reclaimed"
        },
        actor,
        &payload,
        now,
    )?;
    if blocked && reason == "claim expired" {
        insert_event(
            conn,
            board_id,
            Some(&task.id),
            task.current_run_id.as_deref(),
            "task.reclaimed",
            actor,
            &payload,
            now,
        )?;
    }
    Ok(())
}

fn initial_status(
    explicit: Option<TaskStatus>,
    description: Option<&str>,
    scheduled_at: Option<i64>,
    now: i64,
) -> Result<TaskStatus> {
    if let Some(status) = explicit {
        if !status.can_be_created() {
            return Err(KanbanError::InvalidInput(
                "initial status must be triage/todo/scheduled/ready".into(),
            ));
        }
        match status {
            TaskStatus::Scheduled if scheduled_at.is_none() => {
                return Err(KanbanError::InvalidInput(
                    "scheduled initial status requires scheduled_at".into(),
                ));
            }
            TaskStatus::Ready
                if description.is_none_or(|description| description.trim().is_empty()) =>
            {
                return Err(KanbanError::InvalidInput(
                    "ready requires description".into(),
                ));
            }
            TaskStatus::Ready if scheduled_at.is_some_and(|scheduled| scheduled > now) => {
                return Err(KanbanError::InvalidInput(
                    "ready requires scheduled_at to be due".into(),
                ));
            }
            _ => {
                return Ok(status);
            }
        }
    }
    if description.is_none_or(|d| d.trim().is_empty()) {
        return Ok(TaskStatus::Triage);
    }
    if scheduled_at.is_some_and(|t| t > now) {
        return Ok(TaskStatus::Scheduled);
    }
    Ok(TaskStatus::Ready)
}

#[allow(clippy::too_many_arguments)]
fn finish_running(
    conn: &Connection,
    board_id: &str,
    task: &TaskRecord,
    target: TaskStatus,
    actor: &str,
    event: &str,
    run_status: &str,
    exit_code: i32,
    reason: Option<&str>,
    log_path: Option<&Path>,
    summary: Option<&str>,
    result_json: Option<&str>,
    now: i64,
) -> Result<()> {
    let completed = if target == TaskStatus::Done {
        Some(now)
    } else {
        task.completed_at
    };
    if task.status != TaskStatus::Running
        && !(task.status == TaskStatus::Review && target == TaskStatus::Done)
    {
        return Err(KanbanError::InvalidTransition(
            "finish requires matching running claim".into(),
        ));
    }
    let changed = if task.status == TaskStatus::Running {
        if task.claim_token.is_none() || task.current_run_id.is_none() {
            return Err(KanbanError::InvalidTransition(
                "finish requires matching running claim".into(),
            ));
        }
        conn.execute(
            "UPDATE tasks SET status=?1, status_reason=?2, completed_at=?3, result_summary=COALESCE(?4, result_summary), result_json=COALESCE(?5, result_json), claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, updated_at=?6, lock_version=lock_version+1 WHERE id=?7 AND board_id=?8 AND status='running' AND claim_token=?9 AND current_run_id=?10",
            params![target.as_str(), reason, completed, summary, result_json, now, task.id, board_id, task.claim_token, task.current_run_id],
        )
    } else {
        conn.execute(
            "UPDATE tasks SET status=?1, status_reason=?2, completed_at=?3, result_summary=COALESCE(?4, result_summary), result_json=COALESCE(?5, result_json), claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, updated_at=?6, lock_version=lock_version+1 WHERE id=?7 AND board_id=?8 AND status='review'",
            params![target.as_str(), reason, completed, summary, result_json, now, task.id, board_id],
        )
    }
    .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition(
            "finish requires matching running claim".into(),
        ));
    }
    let event_payload = json!({
        "result": result_json.and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok()),
    })
    .to_string();
    if let Some(run_id) = &task.current_run_id {
        let changed = conn.execute(
            "UPDATE task_runs SET status=?1, finished_at=?2, exit_code=?3, error=?4, log_path=COALESCE(?5, log_path), summary=COALESCE(?6, summary) WHERE id=?7 AND board_id=?8 AND task_id=?9 AND status='running' AND claim_token IS ?10",
            params![run_status, now, exit_code, reason, log_path.map(|p| p.to_string_lossy().to_string()), summary, run_id, board_id, task.id, task.claim_token],
        ).map_err(storage)?;
        if task.status == TaskStatus::Running && changed != 1 {
            return Err(KanbanError::InvalidTransition(
                "finish requires matching running run".into(),
            ));
        }
        insert_event(
            conn,
            board_id,
            Some(&task.id),
            Some(run_id),
            event,
            actor,
            &event_payload,
            now,
        )?;
    } else {
        insert_event(
            conn,
            board_id,
            Some(&task.id),
            None,
            event,
            actor,
            &event_payload,
            now,
        )?;
    }
    Ok(())
}

fn guarded_set_status(
    conn: &Connection,
    board_id: &str,
    task: &TaskRecord,
    status: TaskStatus,
    actor: &str,
    event: &str,
    now: i64,
) -> Result<()> {
    guarded_set_status_with_reason(
        conn,
        board_id,
        task,
        StatusUpdate {
            status,
            status_reason: None,
            actor,
            event,
            now,
        },
    )
}

struct StatusUpdate<'a> {
    status: TaskStatus,
    status_reason: Option<&'a str>,
    actor: &'a str,
    event: &'a str,
    now: i64,
}

fn guarded_set_status_with_reason(
    conn: &Connection,
    board_id: &str,
    task: &TaskRecord,
    update: StatusUpdate<'_>,
) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE tasks SET status=?1, status_reason=?2, updated_at=?3, lock_version=lock_version+1 WHERE id=?4 AND board_id=?5 AND status=?6 AND lock_version=?7",
            params![
                update.status.as_str(),
                update.status_reason,
                update.now,
                task.id,
                board_id,
                task.status.as_str(),
                task.lock_version
            ],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition(
            "status update requires matching fresh task".into(),
        ));
    }
    let payload = json!({ "to_status": update.status.as_str() }).to_string();
    insert_event(
        conn,
        board_id,
        Some(&task.id),
        None,
        update.event,
        update.actor,
        &payload,
        update.now,
    )
}

fn promote_children(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    parent_id: &str,
    now: i64,
) -> Result<()> {
    let mut stmt = conn
        .prepare("SELECT child_task_id FROM task_dependencies WHERE parent_task_id=?1")
        .map_err(storage)?;
    let child_ids = stmt
        .query_map([parent_id], |r| r.get::<_, String>(0))
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    for child_id in child_ids {
        let child = get_task_by_id(conn, board_id, &child_id)?;
        if matches!(child.status, TaskStatus::Todo | TaskStatus::Scheduled)
            && recompute_ready_status(conn, &child, now)? == TaskStatus::Ready
        {
            guarded_set_status(
                conn,
                board_id,
                &child,
                TaskStatus::Ready,
                actor,
                "task.promoted",
                now,
            )?;
        }
    }
    Ok(())
}

fn recompute_ready_status(conn: &Connection, task: &TaskRecord, now: i64) -> Result<TaskStatus> {
    if task.title.trim().is_empty()
        || task
            .description
            .as_deref()
            .is_none_or(|description| description.trim().is_empty())
    {
        return Ok(TaskStatus::Triage);
    }
    if task.scheduled_at.is_some_and(|t| t > now) {
        return Ok(TaskStatus::Scheduled);
    }
    if !dependencies_done(conn, &task.id)? {
        return Ok(TaskStatus::Todo);
    }
    Ok(TaskStatus::Ready)
}

fn is_active_recomputable_status(status: TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Triage | TaskStatus::Todo | TaskStatus::Scheduled | TaskStatus::Ready
    )
}

fn ensure_dependencies_done(conn: &Connection, task_id: &str) -> Result<()> {
    if dependencies_done(conn, task_id)? {
        Ok(())
    } else {
        Err(KanbanError::InvalidTransition("dependency blocked".into()))
    }
}

fn dependencies_done(conn: &Connection, task_id: &str) -> Result<bool> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM task_dependencies d JOIN tasks p ON p.id=d.parent_task_id WHERE d.child_task_id=?1 AND p.status != 'done'", [task_id], |r| r.get(0)).map_err(storage)?;
    Ok(count == 0)
}

fn count_dependency_cycles(conn: &Connection) -> Result<i64> {
    let mut stmt = conn
        .prepare("SELECT parent_task_id, child_task_id FROM task_dependencies")
        .map_err(storage)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(storage)?;
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    let mut nodes: HashSet<String> = HashSet::new();
    for row in rows {
        let (parent, child) = row.map_err(storage)?;
        nodes.insert(parent.clone());
        nodes.insert(child.clone());
        graph.entry(parent).or_default().push(child);
    }
    Ok(count_cyclic_components(&nodes, &graph))
}

fn count_cyclic_components(nodes: &HashSet<String>, graph: &HashMap<String, Vec<String>>) -> i64 {
    struct Tarjan<'a> {
        graph: &'a HashMap<String, Vec<String>>,
        index: usize,
        stack: Vec<String>,
        indices: HashMap<String, usize>,
        lowlinks: HashMap<String, usize>,
        on_stack: HashSet<String>,
        cycles: i64,
    }

    impl Tarjan<'_> {
        fn visit(&mut self, node: &str) {
            self.indices.insert(node.to_owned(), self.index);
            self.lowlinks.insert(node.to_owned(), self.index);
            self.index += 1;
            self.stack.push(node.to_owned());
            self.on_stack.insert(node.to_owned());

            for next in self.graph.get(node).into_iter().flatten() {
                if !self.indices.contains_key(next) {
                    self.visit(next);
                    let node_low = self.lowlinks[node].min(self.lowlinks[next]);
                    self.lowlinks.insert(node.to_owned(), node_low);
                } else if self.on_stack.contains(next) {
                    let node_low = self.lowlinks[node].min(self.indices[next]);
                    self.lowlinks.insert(node.to_owned(), node_low);
                }
            }

            if self.lowlinks[node] == self.indices[node] {
                let mut component_len = 0;
                while let Some(member) = self.stack.pop() {
                    self.on_stack.remove(&member);
                    component_len += 1;
                    if member == node {
                        break;
                    }
                }
                if component_len > 1
                    || self
                        .graph
                        .get(node)
                        .is_some_and(|edges| edges.iter().any(|next| next == node))
                {
                    self.cycles += 1;
                }
            }
        }
    }

    let mut tarjan = Tarjan {
        graph,
        index: 0,
        stack: Vec::new(),
        indices: HashMap::new(),
        lowlinks: HashMap::new(),
        on_stack: HashSet::new(),
        cycles: 0,
    };
    for node in nodes {
        if !tarjan.indices.contains_key(node) {
            tarjan.visit(node);
        }
    }
    tarjan.cycles
}

fn count_run_log_path_findings(conn: &Connection, db_dir: Option<&Path>) -> Result<(i64, i64)> {
    let mut stmt = conn
        .prepare("SELECT id, log_path FROM task_runs WHERE log_path IS NOT NULL")
        .map_err(storage)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(storage)?;
    let mut missing = 0;
    let mut suspicious = 0;
    for row in rows {
        let (run_id, path) = row.map_err(storage)?;
        match run_log_path_status_for_db_dir(db_dir, &run_id, &path) {
            RunLogPathStatus::Present(_) => {}
            RunLogPathStatus::Missing(_) => missing += 1,
            RunLogPathStatus::Suspicious { .. } => suspicious += 1,
        }
    }
    Ok((missing, suspicious))
}

fn count_executable_dependency_violations(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(DISTINCT t.id) \
         FROM tasks t \
         JOIN task_dependencies d ON d.child_task_id=t.id \
         JOIN tasks p ON p.id=d.parent_task_id \
         WHERE t.status IN ('ready', 'running') AND p.status!='done'",
        [],
        |row| row.get(0),
    )
    .map_err(storage)
}

fn count_executable_spec_violations(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM tasks \
         WHERE status IN ('ready', 'running') \
           AND (description IS NULL OR length(trim(description)) = 0)",
        [],
        |row| row.get(0),
    )
    .map_err(storage)
}

fn count_executable_schedule_violations(conn: &Connection, now: i64) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM tasks \
         WHERE status IN ('ready', 'running') AND scheduled_at IS NOT NULL AND scheduled_at > ?1",
        [now],
        |row| row.get(0),
    )
    .map_err(storage)
}

pub fn resolve_run_log_path(
    db_path: impl AsRef<Path>,
    run_id: &str,
    log_path: &str,
) -> Result<PathBuf> {
    match run_log_path_status(db_path, run_id, log_path) {
        RunLogPathStatus::Present(path) => Ok(path),
        RunLogPathStatus::Missing(_) => Err(KanbanError::NotFound(format!("run log {run_id}"))),
        RunLogPathStatus::Suspicious { reason } => Err(KanbanError::InvalidInput(format!(
            "suspicious run log path for {run_id}: {reason}"
        ))),
    }
}

pub fn run_log_path_status(
    db_path: impl AsRef<Path>,
    run_id: &str,
    log_path: &str,
) -> RunLogPathStatus {
    run_log_path_status_for_db_dir(db_path.as_ref().parent(), run_id, log_path)
}

fn run_log_path_status_for_db_dir(
    db_dir: Option<&Path>,
    run_id: &str,
    log_path: &str,
) -> RunLogPathStatus {
    let expected_name = format!("{run_id}.log");
    let stored_path = Path::new(log_path);
    if stored_path.file_name().and_then(|name| name.to_str()) != Some(expected_name.as_str()) {
        return RunLogPathStatus::Suspicious {
            reason: format!("expected log file name {expected_name}"),
        };
    }

    let Some(db_dir) = db_dir else {
        return RunLogPathStatus::Suspicious {
            reason: "database path has no parent directory".to_owned(),
        };
    };
    let candidate = if stored_path.is_absolute() {
        stored_path.to_path_buf()
    } else {
        db_dir.join(stored_path)
    };
    let normalized_candidate = normalize_existing_aware(&candidate);
    let allowed_roots = [
        kanban_local::default_log_dir().join("runs"),
        db_dir.join("logs"),
        db_dir.join(".kb").join("logs"),
    ];
    let allowed = allowed_roots
        .iter()
        .map(|root| normalize_existing_aware(root))
        .any(|root| normalized_candidate.starts_with(root));
    if !allowed {
        return RunLogPathStatus::Suspicious {
            reason: "path is outside allowed run log roots".to_owned(),
        };
    }
    if normalized_candidate.exists() {
        RunLogPathStatus::Present(normalized_candidate)
    } else {
        RunLogPathStatus::Missing(normalized_candidate)
    }
}

fn normalize_existing_aware(path: &Path) -> PathBuf {
    if let Ok(canonical) = fs::canonicalize(path) {
        return canonical;
    }
    let mut missing = Vec::new();
    let mut ancestor = path;
    while let Some(parent) = ancestor.parent() {
        if let Some(name) = ancestor.file_name() {
            missing.push(name.to_owned());
        }
        if let Ok(canonical_parent) = fs::canonicalize(parent) {
            let mut normalized = canonical_parent;
            for component in missing.iter().rev() {
                normalized.push(component);
            }
            return lexical_normalize(&normalized);
        }
        ancestor = parent;
    }
    lexical_normalize(path)
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn assert_database_idle_for_replace(path: &Path) -> Result<()> {
    let conn = match Connection::open(path) {
        Ok(conn) => conn,
        Err(_) => return Ok(()),
    };
    if default_pragmas(&conn).is_err() {
        return Ok(());
    }
    conn.busy_timeout(Duration::from_millis(0))
        .map_err(storage)?;
    if !table_exists(&conn, "schema_migrations").unwrap_or(false) {
        return Ok(());
    }
    let running_tasks = count_table_status(&conn, "tasks", "running")?;
    let running_runs = count_table_status(&conn, "task_runs", "running")?;
    if running_tasks > 0 || running_runs > 0 {
        return Err(KanbanError::InvalidInput(format!(
            "database has running work; stop kb serve/dispatch before import --replace: {}",
            path.display()
        )));
    }
    let checkpoint = conn
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok(CheckpointResult {
                busy: row.get(0)?,
                log_frames: row.get(1)?,
                checkpointed_frames: row.get(2)?,
            })
        })
        .map_err(storage)?;
    if checkpoint.busy != 0 {
        return Err(KanbanError::InvalidInput(format!(
            "database is busy; stop kb serve/dispatch before import --replace: {}",
            path.display()
        )));
    }
    conn.execute_batch("BEGIN IMMEDIATE; COMMIT;")
        .map_err(|error| {
            KanbanError::InvalidInput(format!(
                "database is busy; stop kb serve/dispatch before import --replace: {} ({error})",
                path.display()
            ))
        })?;
    Ok(())
}

fn count_table_status(conn: &Connection, table: &str, status: &str) -> Result<i64> {
    if !table_exists(conn, table).unwrap_or(false) {
        return Ok(0);
    }
    conn.query_row(
        &format!("SELECT COUNT(*) FROM {table} WHERE status=?1"),
        [status],
        |row| row.get(0),
    )
    .map_err(storage)
}

fn connect_existing_file(path: &Path) -> Result<Connection> {
    if !path.exists() {
        return Err(KanbanError::InvalidInput(format!(
            "database does not exist: {}",
            path.display()
        )));
    }
    if !path.is_file() {
        return Err(KanbanError::InvalidInput(format!(
            "database path is not a file: {}",
            path.display()
        )));
    }
    connect_file(path)
}

fn connect_existing_database(path: &Path) -> Result<Connection> {
    let conn = connect_existing_file(path)?;
    if !table_exists(&conn, "schema_migrations")? {
        return Err(KanbanError::InvalidInput(format!(
            "database is not initialized: {}",
            path.display()
        )));
    }
    let migration_version = conn
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get::<_, Option<i64>>(0)
        })
        .optional()
        .map_err(storage)?
        .flatten();
    let user_version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(storage)?;
    if migration_version.is_none() || user_version == 0 {
        return Err(KanbanError::InvalidInput(format!(
            "database is not initialized: {}",
            path.display()
        )));
    }
    Ok(conn)
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |row| row.get(0),
    )
    .map_err(storage)
}

fn doctor_tables_present(
    conn: &Connection,
    migration_version: Option<i64>,
    user_version: i64,
) -> Result<bool> {
    let mut required_tables = vec!["tasks", "task_dependencies", "task_runs"];
    if migration_version.unwrap_or(0) >= 2 || user_version >= 2 {
        required_tables.extend([
            "entities",
            "relation_predicates",
            "entity_relations",
            "index_outbox",
            "derived_store_state",
        ]);
    }
    for table in required_tables {
        if !table_exists(conn, table)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn has_path(conn: &Connection, start: &str, goal: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "WITH RECURSIVE walk(id) AS (SELECT child_task_id FROM task_dependencies WHERE parent_task_id=?1 UNION SELECT d.child_task_id FROM task_dependencies d JOIN walk w ON d.parent_task_id=w.id) SELECT COUNT(*) FROM walk WHERE id=?2",
        params![start, goal], |r| r.get(0)).map_err(storage)?;
    Ok(count > 0)
}

const BOARD_SCOPED_EXPORT_TABLES: &[(&str, &str)] = &[
    ("column", "board_columns"),
    ("task", "tasks"),
    ("dependency", "task_dependencies"),
    ("run", "task_runs"),
    ("comment", "task_comments"),
    ("event", "task_events"),
    ("attachment", "task_attachments"),
    ("label", "labels"),
    ("task_label", "task_labels"),
];

const IMPORT_DELETE_ORDER: &[&str] = &[
    "task_labels",
    "labels",
    "task_attachments",
    "task_events",
    "task_comments",
    "task_runs",
    "task_dependencies",
    "tasks",
    "board_columns",
    "boards",
    "app_settings",
];

fn write_jsonl_table(
    conn: &Connection,
    writer: &mut impl Write,
    record_type: &str,
    table: &str,
    where_sql: &str,
    params: Vec<Value>,
    export_now: i64,
) -> Result<usize> {
    let sql = format!("SELECT * FROM {table} {where_sql}");
    let mut stmt = conn.prepare(&sql).map_err(storage)?;
    let columns = stmt
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut rows = stmt
        .query(params_from_iter(params.iter()))
        .map_err(storage)?;
    let mut count = 0;
    while let Some(row) = rows.next().map_err(storage)? {
        let mut data = serde_json::Map::new();
        for (index, column) in columns.iter().enumerate() {
            data.insert(
                column.clone(),
                value_ref_to_json(row.get_ref(index).map_err(storage)?),
            );
        }
        scrub_jsonl_export_record(record_type, &mut data, export_now);
        let record = json!({ "type": record_type, "data": data });
        writeln!(writer, "{record}").map_err(|error| KanbanError::Storage(error.to_string()))?;
        count += 1;
    }
    Ok(count)
}

fn write_export_sanitized_events(
    conn: &Connection,
    writer: &mut impl Write,
    board_id: &str,
    export_now: i64,
) -> Result<usize> {
    let mut stmt = conn
        .prepare(
            "SELECT id,current_run_id,claim_owner,claim_expires_at \
             FROM tasks WHERE board_id=?1 AND status='running' ORDER BY id ASC",
        )
        .map_err(storage)?;
    let tasks = stmt
        .query_map([board_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    if tasks.is_empty() {
        return Ok(0);
    }

    let mut next_id: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(id), 0) + 1 FROM task_events WHERE board_id=?1",
            [board_id],
            |row| row.get(0),
        )
        .map_err(storage)?;
    let first_id = next_id;
    for (task_id, run_id, claim_owner, claim_expires_at) in tasks {
        let payload = json!({
            "from_status": "running",
            "to_status": "ready",
            "run_status": "canceled",
            "original_run_id": run_id,
            "claim_owner": claim_owner,
            "claim_expires_at": claim_expires_at,
            "reason": "jsonl export clears non-portable live claim"
        })
        .to_string();
        let record = json!({
            "type": "event",
            "data": {
                "id": next_id,
                "event_id": new_event_id(),
                "board_id": board_id,
                "task_id": task_id,
                "run_id": run_id,
                "kind": "task.export_sanitized",
                "actor": "kb export",
                "payload_json": payload,
                "created_at": export_now
            }
        });
        writeln!(writer, "{record}").map_err(|error| KanbanError::Storage(error.to_string()))?;
        next_id += 1;
    }
    Ok((next_id - first_id) as usize)
}

fn scrub_jsonl_export_record(
    record_type: &str,
    data: &mut serde_json::Map<String, serde_json::Value>,
    export_now: i64,
) {
    if record_type == "task"
        && data
            .get("status")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|status| status == "running")
    {
        data.insert("status".into(), json!("ready"));
        data.insert("claim_token".into(), serde_json::Value::Null);
        data.insert("claim_owner".into(), serde_json::Value::Null);
        data.insert("claim_expires_at".into(), serde_json::Value::Null);
        data.insert("last_heartbeat_at".into(), serde_json::Value::Null);
        data.insert("current_run_id".into(), serde_json::Value::Null);
        data.insert("started_at".into(), serde_json::Value::Null);
    }

    if record_type == "run" {
        data.insert("log_path".into(), serde_json::Value::Null);
        if data
            .get("status")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|status| status == "running")
        {
            data.insert("status".into(), json!("canceled"));
            data.insert("finished_at".into(), json!(export_now));
            data.insert(
                "error".into(),
                json!("canceled by jsonl export; claim is not portable"),
            );
        }
    }
}

fn validate_imported_snapshot(conn: &Connection) -> Result<()> {
    let board_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM boards", [], |row| row.get(0))
        .map_err(storage)?;
    if board_count == 0 {
        return Err(KanbanError::InvalidInput(
            "imported data must contain at least one board".into(),
        ));
    }

    let boards_without_columns: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM boards b \
             WHERE NOT EXISTS (SELECT 1 FROM board_columns c WHERE c.board_id=b.id)",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if boards_without_columns > 0 {
        return Err(KanbanError::InvalidInput(
            "imported data must contain columns for every board".into(),
        ));
    }
    Ok(())
}

fn create_temp_export_file(out_path: &Path) -> Result<(PathBuf, File)> {
    let file_name = out_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("export.jsonl");
    let parent = out_path.parent().unwrap_or_else(|| Path::new("."));
    for attempt in 0..100 {
        let temp_path = parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            SystemClock.now_ms() + attempt
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(KanbanError::Storage(error.to_string())),
        }
    }
    Err(KanbanError::Storage(format!(
        "failed to create temporary export file next to {}",
        out_path.display()
    )))
}

fn reject_imported_active_claims(conn: &Connection) -> Result<()> {
    let now = SystemClock.now_ms();
    let active_running_tasks: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE status='running' AND claim_expires_at > ?1",
            [now],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if active_running_tasks > 0 {
        return Err(KanbanError::InvalidInput(
            "imported data contains active running claims".into(),
        ));
    }
    Ok(())
}

fn value_ref_to_json(value: ValueRef<'_>) -> serde_json::Value {
    match value {
        ValueRef::Null => serde_json::Value::Null,
        ValueRef::Integer(value) => json!(value),
        ValueRef::Real(value) => json!(value),
        ValueRef::Text(value) => json!(String::from_utf8_lossy(value).into_owned()),
        ValueRef::Blob(value) => json!(format!("hex:{}", hex_bytes(value))),
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn database_has_user_records(conn: &Connection) -> Result<bool> {
    for table in [
        "boards",
        "board_columns",
        "tasks",
        "task_dependencies",
        "task_runs",
        "task_comments",
        "task_events",
        "task_attachments",
        "labels",
        "task_labels",
        "app_settings",
    ] {
        let count: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .map_err(storage)?;
        if count > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

fn insert_jsonl_record(conn: &Connection, record: &serde_json::Value) -> Result<()> {
    let record_type = record
        .get("type")
        .and_then(|value| value.as_str())
        .ok_or_else(|| KanbanError::InvalidInput("export record type is required".into()))?;
    let table = import_table_for_type(record_type)?;
    let data = record
        .get("data")
        .and_then(|value| value.as_object())
        .ok_or_else(|| KanbanError::InvalidInput("export record data is required".into()))?;
    if data.is_empty() {
        return Err(KanbanError::InvalidInput(
            "export record data cannot be empty".into(),
        ));
    }
    let columns = data.keys().map(String::as_str).collect::<Vec<_>>();
    if columns.iter().any(|column| !is_sql_identifier(column)) {
        return Err(KanbanError::InvalidInput(
            "export record contains an invalid column name".into(),
        ));
    }
    let placeholders = std::iter::repeat_n("?", columns.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "INSERT OR REPLACE INTO {table} ({}) VALUES ({placeholders})",
        columns.join(",")
    );
    let values = columns
        .iter()
        .map(|column| json_to_sql_value(&data[*column]))
        .collect::<Result<Vec<_>>>()?;
    conn.execute(&sql, params_from_iter(values.iter()))
        .map_err(storage)?;
    Ok(())
}

fn is_sql_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn import_table_for_type(record_type: &str) -> Result<&'static str> {
    match record_type {
        "board" => Ok("boards"),
        "column" => Ok("board_columns"),
        "task" => Ok("tasks"),
        "dependency" => Ok("task_dependencies"),
        "run" => Ok("task_runs"),
        "comment" => Ok("task_comments"),
        "event" => Ok("task_events"),
        "attachment" => Ok("task_attachments"),
        "label" => Ok("labels"),
        "task_label" => Ok("task_labels"),
        "setting" => Ok("app_settings"),
        _ => Err(KanbanError::InvalidInput(format!(
            "unsupported export record type: {record_type}"
        ))),
    }
}

fn json_to_sql_value(value: &serde_json::Value) -> Result<Value> {
    match value {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(value) => Ok(Value::Integer(i64::from(*value))),
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                Ok(Value::Integer(value))
            } else if let Some(value) = value.as_f64() {
                Ok(Value::Real(value))
            } else {
                Err(KanbanError::InvalidInput(
                    "unsupported numeric export value".into(),
                ))
            }
        }
        serde_json::Value::String(value) => Ok(Value::Text(value.clone())),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Ok(Value::Text(value.to_string()))
        }
    }
}

const TASK_COLUMNS: &str = "id,board_id,seq,title,description,status,status_reason,assignee,priority,position,scheduled_at,due_at,created_by,created_at,updated_at,started_at,completed_at,archived_at,claim_token,claim_owner,claim_expires_at,last_heartbeat_at,current_run_id,retry_count,max_retries,result_summary,result_json,metadata_json,lock_version";

fn task_query_where(board_id: &str, options: &TaskListOptions) -> (String, Vec<Value>) {
    let mut clauses = vec!["WHERE board_id=?".to_owned()];
    let mut params = vec![Value::Text(board_id.to_owned())];
    if !options.include_archived {
        clauses.push("status != 'archived'".to_owned());
    }
    if !options.statuses.is_empty() {
        let placeholders = std::iter::repeat_n("?", options.statuses.len())
            .collect::<Vec<_>>()
            .join(",");
        clauses.push(format!("status IN ({placeholders})"));
        params.extend(
            options
                .statuses
                .iter()
                .map(|status| Value::Text(status.as_str().to_owned())),
        );
    }
    if let Some(assignee) = options
        .assignee
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        clauses.push("assignee=?".to_owned());
        params.push(Value::Text(assignee.to_owned()));
    }
    if let Some(search) = options
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let needle = format!("%{}%", sqlite_like_literal(&search.to_lowercase()));
        clauses.push(
            "(lower(title) LIKE ? ESCAPE '\\' OR lower(COALESCE(description, '')) LIKE ? ESCAPE '\\')"
                .to_owned(),
        );
        params.push(Value::Text(needle.clone()));
        params.push(Value::Text(needle));
    }
    (clauses.join(" AND "), params)
}

fn validate_page_bounds(limit: usize, max_limit: usize, offset: usize) -> Result<()> {
    if limit > max_limit {
        return Err(KanbanError::InvalidInput(format!(
            "limit must be <= {max_limit}"
        )));
    }
    if offset > i64::MAX as usize {
        return Err(KanbanError::InvalidInput(format!(
            "offset must be <= {}",
            i64::MAX
        )));
    }
    Ok(())
}

fn sqlite_like_literal(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '%' | '_' | '\\' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn search_task_where(board_id: &str, query: &SearchQuery) -> (String, Vec<Value>) {
    let mut clauses = vec!["WHERE t.board_id=?".to_owned()];
    let mut params = vec![Value::Text(board_id.to_owned())];
    if !query.include_archived {
        clauses.push("t.status != 'archived'".to_owned());
    }
    if !query.statuses.is_empty() {
        let placeholders = std::iter::repeat_n("?", query.statuses.len())
            .collect::<Vec<_>>()
            .join(",");
        clauses.push(format!("t.status IN ({placeholders})"));
        params.extend(
            query
                .statuses
                .iter()
                .map(|status| Value::Text(status.as_str().to_owned())),
        );
    }
    if let Some(assignee) = query
        .assignee
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        clauses.push("t.assignee=?".to_owned());
        params.push(Value::Text(assignee.to_owned()));
    }
    (clauses.join(" AND "), params)
}

fn task_order_by(sort: TaskListSort) -> &'static str {
    match sort {
        TaskListSort::Position => "position ASC, created_at ASC, seq ASC",
        TaskListSort::PositionDesc => "position DESC, created_at DESC, seq DESC",
        TaskListSort::Priority => "priority ASC, created_at ASC, seq ASC",
        TaskListSort::PriorityDesc => "priority DESC, created_at DESC, seq DESC",
        TaskListSort::CreatedAt => "created_at ASC, seq ASC",
        TaskListSort::CreatedAtDesc => "created_at DESC, seq DESC",
        TaskListSort::UpdatedAt => "updated_at ASC, seq ASC",
        TaskListSort::UpdatedAtDesc => "updated_at DESC, seq DESC",
        TaskListSort::DueAt => "COALESCE(due_at, 9223372036854775807) ASC, created_at ASC, seq ASC",
        TaskListSort::DueAtDesc => {
            "COALESCE(due_at, -9223372036854775808) DESC, created_at DESC, seq DESC"
        }
    }
}

fn current_last_event_id(conn: &Connection, board_id: &str) -> Result<Option<i64>> {
    conn.query_row(
        "SELECT MAX(id) FROM task_events WHERE board_id=?1",
        params![board_id],
        |row| row.get(0),
    )
    .map_err(storage)
}

#[cfg(feature = "tantivy-backend")]
fn search_state_key(board_id: &str) -> String {
    format!("{SEARCH_TASKS_STATE_KEY_PREFIX}.{board_id}")
}

#[cfg(feature = "tantivy-backend")]
fn default_search_index_state(
    board_id: &str,
    last_event_id: Option<i64>,
    dirty: bool,
    message: Option<String>,
    now: i64,
) -> SearchIndexState {
    SearchIndexState {
        schema_version: SEARCH_TASKS_STATE_SCHEMA_VERSION,
        index_version: kanban_search::tantivy_backend::INDEX_VERSION.to_owned(),
        backend: "tantivy".to_owned(),
        index_name: "tasks".to_owned(),
        board_id: board_id.to_owned(),
        last_event_id,
        dirty,
        updated_at: now,
        message,
    }
}

#[cfg(feature = "tantivy-backend")]
fn read_search_index_state(conn: &Connection, board_id: &str) -> Result<Option<SearchIndexState>> {
    let key = search_state_key(board_id);
    let value = conn
        .query_row(
            "SELECT value_json FROM app_settings WHERE key=?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage)?;
    value
        .map(|value| {
            serde_json::from_str(&value).map_err(|err| KanbanError::Storage(err.to_string()))
        })
        .transpose()
}

#[cfg(feature = "tantivy-backend")]
fn write_search_index_state(conn: &Connection, state: &SearchIndexState) -> Result<()> {
    let key = search_state_key(&state.board_id);
    conn.execute(
        "INSERT INTO app_settings(key,value_json,updated_at) VALUES (?1,?2,?3) \
         ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json, updated_at=excluded.updated_at",
        params![
            key,
            serde_json::to_string(state).map_err(|err| KanbanError::Storage(err.to_string()))?,
            state.updated_at
        ],
    )
    .map_err(storage)?;
    Ok(())
}

fn search_lag(current_last_event_id: Option<i64>, indexed_last_event_id: Option<i64>) -> i64 {
    match (current_last_event_id, indexed_last_event_id) {
        (Some(current), Some(indexed)) => current.abs_diff(indexed).try_into().unwrap_or(i64::MAX),
        (Some(current), None) => current,
        _ => 0,
    }
}

#[cfg(feature = "tantivy-backend")]
fn search_index_ahead(
    current_last_event_id: Option<i64>,
    indexed_last_event_id: Option<i64>,
) -> bool {
    matches!(
        (current_last_event_id, indexed_last_event_id),
        (Some(current), Some(indexed)) if indexed > current
    )
}

#[cfg(feature = "tantivy-backend")]
fn task_index_path(db_path: &Path) -> PathBuf {
    kanban_local::task_index_path(db_path.to_path_buf())
}

#[cfg(feature = "tantivy-backend")]
fn task_search_documents(conn: &Connection, board_id: &str) -> Result<Vec<TaskSearchDocument>> {
    let mut stmt = conn
        .prepare(
            "SELECT t.id,t.board_id,t.seq,t.status,t.assignee,t.priority,t.created_at,t.updated_at,t.due_at,t.title,t.description,\
                    COALESCE((SELECT group_concat(c.body, char(10)) FROM task_comments c WHERE c.board_id=t.board_id AND c.task_id=t.id ORDER BY c.created_at ASC, c.id ASC), '') AS comments,\
                    COALESCE((SELECT group_concat(COALESCE(r.summary, '') || ' ' || COALESCE(r.error, ''), char(10)) FROM task_runs r WHERE r.board_id=t.board_id AND r.task_id=t.id ORDER BY r.started_at ASC, r.id ASC), '') AS run_text,\
                    COALESCE((SELECT group_concat(e.kind || ' ' || e.payload_json, char(10)) FROM task_events e WHERE e.board_id=t.board_id AND e.task_id=t.id ORDER BY e.id ASC), '') AS event_text \
             FROM tasks t WHERE t.board_id=?1 ORDER BY t.seq ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map([board_id], |row| {
            let status: String = row.get(3)?;
            Ok(TaskSearchDocument {
                task_id: row.get(0)?,
                board_id: row.get(1)?,
                seq: row.get(2)?,
                status: TaskStatus::try_from(status.as_str())
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
                assignee: row.get(4)?,
                priority: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                due_at: row.get(8)?,
                title: row.get(9)?,
                description: row.get(10)?,
                comments: row.get(11)?,
                run_text: row.get(12)?,
                event_text: row.get(13)?,
            })
        })
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

#[cfg(feature = "tantivy-backend")]
fn task_search_documents_for_task_ids(
    conn: &Connection,
    board_id: &str,
    task_ids: &[String],
) -> Result<Vec<TaskSearchDocument>> {
    if task_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat_n("?", task_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT t.id,t.board_id,t.seq,t.status,t.assignee,t.priority,t.created_at,t.updated_at,t.due_at,t.title,t.description,\
                COALESCE((SELECT group_concat(c.body, char(10)) FROM task_comments c WHERE c.board_id=t.board_id AND c.task_id=t.id ORDER BY c.created_at ASC, c.id ASC), '') AS comments,\
                COALESCE((SELECT group_concat(COALESCE(r.summary, '') || ' ' || COALESCE(r.error, ''), char(10)) FROM task_runs r WHERE r.board_id=t.board_id AND r.task_id=t.id ORDER BY r.started_at ASC, r.id ASC), '') AS run_text,\
                COALESCE((SELECT group_concat(e.kind || ' ' || e.payload_json, char(10)) FROM task_events e WHERE e.board_id=t.board_id AND e.task_id=t.id ORDER BY e.id ASC), '') AS event_text \
         FROM tasks t WHERE t.board_id=? AND t.id IN ({placeholders}) ORDER BY t.seq ASC"
    );
    let mut params = vec![Value::Text(board_id.to_owned())];
    params.extend(task_ids.iter().cloned().map(Value::Text));
    let mut stmt = conn.prepare(&sql).map_err(storage)?;
    let rows = stmt
        .query_map(params_from_iter(params.iter()), |row| {
            let status: String = row.get(3)?;
            Ok(TaskSearchDocument {
                task_id: row.get(0)?,
                board_id: row.get(1)?,
                seq: row.get(2)?,
                status: TaskStatus::try_from(status.as_str())
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
                assignee: row.get(4)?,
                priority: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                due_at: row.get(8)?,
                title: row.get(9)?,
                description: row.get(10)?,
                comments: row.get(11)?,
                run_text: row.get(12)?,
                event_text: row.get(13)?,
            })
        })
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

#[cfg(feature = "tantivy-backend")]
fn affected_task_ids_since(
    conn: &Connection,
    board_id: &str,
    last_event_id: i64,
) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare(
            "SELECT task_id, run_id FROM task_events \
             WHERE board_id=?1 AND id>?2 ORDER BY id ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map(params![board_id, last_event_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
            ))
        })
        .map_err(storage)?;
    let mut ids = Vec::new();
    let mut seen = HashSet::new();
    for row in rows {
        let (task_id, run_id) = row.map_err(storage)?;
        let task_id = match task_id {
            Some(task_id) => Some(task_id),
            None => run_id
                .as_deref()
                .map(|run_id| {
                    conn.query_row(
                        "SELECT task_id FROM task_runs WHERE board_id=?1 AND id=?2",
                        params![board_id, run_id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()
                    .map_err(storage)
                })
                .transpose()?
                .flatten(),
        };
        if let Some(task_id) = task_id
            && seen.insert(task_id.clone())
        {
            ids.push(task_id);
        }
    }
    Ok(ids)
}

fn query_tasks(conn: &Connection, board_id: &str) -> Result<Vec<TaskRecord>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {TASK_COLUMNS} FROM tasks WHERE board_id=?1 ORDER BY CASE status WHEN 'triage' THEN 10 WHEN 'todo' THEN 20 WHEN 'scheduled' THEN 30 WHEN 'ready' THEN 40 WHEN 'running' THEN 50 WHEN 'blocked' THEN 60 WHEN 'review' THEN 70 WHEN 'done' THEN 80 ELSE 90 END, position ASC, priority DESC, created_at ASC"
        ))
        .map_err(storage)?;
    let rows = stmt.query_map([board_id], task_from_row).map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn get_task_by_id(conn: &Connection, board_id: &str, task_id: &str) -> Result<TaskRecord> {
    conn.query_row(
        &format!("SELECT {TASK_COLUMNS} FROM tasks WHERE board_id=?1 AND id=?2"),
        params![board_id, task_id],
        task_from_row,
    )
    .optional()
    .map_err(storage)?
    .ok_or_else(|| KanbanError::NotFound(format!("task {task_id}")))
}

fn resolve_task(conn: &Connection, board_id: &str, task_ref: &str) -> Result<TaskRecord> {
    if let Some(seq) = task_ref.strip_prefix('#') {
        let seq: i64 = seq
            .parse()
            .map_err(|_| KanbanError::InvalidInput("invalid task seq".into()))?;
        conn.query_row("SELECT id,board_id,seq,title,description,status,status_reason,assignee,priority,position,scheduled_at,due_at,created_by,created_at,updated_at,started_at,completed_at,archived_at,claim_token,claim_owner,claim_expires_at,last_heartbeat_at,current_run_id,retry_count,max_retries,result_summary,result_json,metadata_json,lock_version FROM tasks WHERE board_id=?1 AND seq=?2", params![board_id, seq], task_from_row).optional().map_err(storage)?.ok_or_else(|| KanbanError::NotFound(format!("task #{seq}")))
    } else {
        get_task_by_id(conn, board_id, task_ref)
    }
}

fn task_from_row(row: &Row<'_>) -> rusqlite::Result<TaskRecord> {
    let status: String = row.get(5)?;
    Ok(TaskRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        seq: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        status: TaskStatus::try_from(status.as_str())
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        status_reason: row.get(6)?,
        assignee: row.get(7)?,
        priority: row.get(8)?,
        position: row.get(9)?,
        scheduled_at: row.get(10)?,
        due_at: row.get(11)?,
        created_by: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        started_at: row.get(15)?,
        completed_at: row.get(16)?,
        archived_at: row.get(17)?,
        claim_token: row.get(18)?,
        claim_owner: row.get(19)?,
        claim_expires_at: row.get(20)?,
        last_heartbeat_at: row.get(21)?,
        current_run_id: row.get(22)?,
        retry_count: row.get(23)?,
        max_retries: row.get(24)?,
        result_summary: row.get(25)?,
        result_json: row.get(26)?,
        metadata_json: row.get(27)?,
        lock_version: row.get(28)?,
    })
}

fn event_from_row(row: &Row<'_>) -> rusqlite::Result<EventRecord> {
    Ok(EventRecord {
        id: row.get(0)?,
        event_id: row.get(1)?,
        task_id: row.get(2)?,
        run_id: row.get(3)?,
        kind: row.get(4)?,
        actor: row.get(5)?,
        payload_json: row.get(6)?,
        created_at: row.get(7)?,
    })
}
fn run_from_row(row: &Row<'_>) -> rusqlite::Result<RunRecord> {
    Ok(RunRecord {
        id: row.get(0)?,
        task_id: row.get(1)?,
        status: row.get(2)?,
        worker_profile: row.get(3)?,
        worker_pid: row.get(4)?,
        claim_token: row.get(5)?,
        claim_owner: row.get(6)?,
        started_at: row.get(7)?,
        finished_at: row.get(8)?,
        exit_code: row.get(9)?,
        summary: row.get(10)?,
        error: row.get(11)?,
        log_path: row.get(12)?,
        metadata_json: row.get(13)?,
    })
}

fn entity_from_row(row: &Row<'_>) -> rusqlite::Result<EntityRecord> {
    Ok(EntityRecord {
        uri: row.get(0)?,
        kind: row.get(1)?,
        source_table: row.get(2)?,
        source_id: row.get(3)?,
        board_id: row.get(4)?,
        task_id: row.get(5)?,
        title: row.get(6)?,
        summary: row.get(7)?,
        content_hash: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        archived_at: row.get(11)?,
    })
}

fn outbox_from_row(row: &Row<'_>) -> rusqlite::Result<IndexOutboxRecord> {
    Ok(IndexOutboxRecord {
        id: row.get(0)?,
        source_event_id: row.get(1)?,
        target: row.get(2)?,
        entity_uri: row.get(3)?,
        action: row.get(4)?,
        payload_json: row.get(5)?,
        status: row.get(6)?,
        attempts: row.get(7)?,
        last_error: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn derived_store_status_from_row(row: &Row<'_>) -> rusqlite::Result<DerivedStoreStatusRecord> {
    let dirty: i64 = row.get(3)?;
    Ok(DerivedStoreStatusRecord {
        store_name: row.get(0)?,
        schema_version: row.get(1)?,
        last_event_id: row.get(2)?,
        dirty: dirty != 0,
        last_rebuild_at: row.get(4)?,
        last_sync_at: row.get(5)?,
        last_error: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn relation_from_row(row: &Row<'_>) -> rusqlite::Result<Relation> {
    let predicate: String = row.get(1)?;
    Ok(Relation {
        subject_uri: EntityUri::new(row.get::<_, String>(0)?).map_err(sql_from_display)?,
        predicate: predicate_from_str(&predicate).map_err(sql_from_display)?,
        object_uri: EntityUri::new(row.get::<_, String>(2)?).map_err(sql_from_display)?,
        graph_uri: EntityUri::new(row.get::<_, String>(3)?).map_err(sql_from_display)?,
        provenance: Provenance {
            authoritative_store: row.get(4)?,
            source_table: row.get(5)?,
            source_id: row.get(6)?,
            source_event_id: row.get(7)?,
        },
        metadata_json: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn graph_relation_snapshot_for_board(conn: &Connection, board_id: &str) -> Result<Vec<Relation>> {
    let mut stmt = conn
        .prepare(
            "SELECT r.subject_uri,r.predicate,r.object_uri,r.graph_uri,r.authoritative_store,r.source_table,r.source_id,r.source_event_id,r.metadata_json,r.created_at,r.updated_at \
             FROM entity_relations r \
             JOIN entities s ON s.uri=r.subject_uri \
             WHERE s.board_id=?1 \
             ORDER BY r.subject_uri ASC, r.predicate ASC, r.object_uri ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map([board_id], relation_from_row)
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

#[cfg(feature = "graph-oxigraph")]
fn graph_entity_uris_for_board(conn: &Connection, board_id: &str) -> Result<Vec<EntityUri>> {
    let mut stmt = conn
        .prepare("SELECT uri FROM entities WHERE board_id=?1 ORDER BY uri ASC")
        .map_err(storage)?;
    let rows = stmt
        .query_map([board_id], |row| {
            EntityUri::new(row.get::<_, String>(0)?).map_err(sql_from_display)
        })
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

#[cfg(feature = "graph-oxigraph")]
fn graph_relations_for_entity(
    conn: &Connection,
    board_id: &str,
    entity_uri: &str,
) -> Result<Vec<Relation>> {
    let mut stmt = conn
        .prepare(
            "SELECT r.subject_uri,r.predicate,r.object_uri,r.graph_uri,r.authoritative_store,r.source_table,r.source_id,r.source_event_id,r.metadata_json,r.created_at,r.updated_at \
             FROM entity_relations r \
             JOIN entities s ON s.uri=r.subject_uri \
             WHERE s.board_id=?1 AND r.subject_uri=?2 \
             ORDER BY r.predicate ASC, r.object_uri ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map(params![board_id, entity_uri], relation_from_row)
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn predicate_from_str(value: &str) -> Result<Predicate> {
    match value {
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
        _ => Err(KanbanError::Storage(format!("unknown predicate: {value}"))),
    }
}

fn sql_from_display(error: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(KanbanError::Storage(error.to_string())))
}

#[cfg(feature = "graph-oxigraph")]
fn graph_storage(error: impl std::fmt::Display) -> KanbanError {
    KanbanError::Storage(error.to_string())
}

fn derived_status_by_name(conn: &Connection, store_name: &str) -> Result<DerivedStoreStatusRecord> {
    conn.query_row(
        "SELECT store_name,schema_version,last_event_id,dirty,last_rebuild_at,last_sync_at,last_error,updated_at \
         FROM derived_store_state WHERE store_name=?1",
        [store_name],
        derived_store_status_from_row,
    )
    .optional()
    .map_err(storage)?
    .ok_or_else(|| KanbanError::Storage(format!("missing derived store state: {store_name}")))
}

#[cfg(feature = "graph-oxigraph")]
fn pending_graph_outbox_for_board(
    conn: &Connection,
    board_id: &str,
    last_event_id: Option<i64>,
) -> Result<Vec<IndexOutboxRecord>> {
    let mut stmt = conn
        .prepare(
            "SELECT o.id,o.source_event_id,o.target,o.entity_uri,o.action,o.payload_json,o.status,o.attempts,o.last_error,o.created_at,o.updated_at \
             FROM index_outbox o \
             JOIN task_events e ON e.id=o.source_event_id \
             WHERE o.target IN ('oxigraph', 'all') \
               AND o.status IN ('pending', 'running', 'failed') \
               AND e.board_id=?1 \
               AND e.id <= ?2 \
             ORDER BY o.id ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map(
            params![board_id, last_event_id.unwrap_or(i64::MAX)],
            outbox_from_row,
        )
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

#[cfg(feature = "graph-oxigraph")]
fn has_pending_graph_outbox_for_board(
    conn: &Connection,
    board_id: &str,
    last_event_id: Option<i64>,
) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS( \
             SELECT 1 FROM index_outbox o \
             JOIN task_events e ON e.id=o.source_event_id \
             WHERE o.target IN ('oxigraph', 'all') \
               AND o.status IN ('pending', 'running', 'failed') \
               AND e.board_id=?1 \
               AND e.id <= ?2 \
         )",
        params![board_id, last_event_id.unwrap_or(i64::MAX)],
        |row| row.get::<_, bool>(0),
    )
    .map_err(storage)
}

#[cfg(feature = "graph-oxigraph")]
fn graph_store_path(db_path: &Path) -> PathBuf {
    kanban_local::graph_store_path(db_path.to_path_buf())
}

fn vector_store_status_with(
    conn: &Connection,
    board_id: &str,
    store: &(impl VectorStore + ?Sized),
) -> Result<VectorStoreStatus> {
    vector_store_status_from_base(conn, board_id, store.status())
}

#[cfg(feature = "vector-lancedb")]
fn vector_store_status_without_provider(
    conn: &Connection,
    board_id: &str,
) -> Result<VectorStoreStatus> {
    vector_store_status_from_base(
        conn,
        board_id,
        VectorStoreStatus {
            backend: "lancedb".to_owned(),
            enabled: false,
            message: "LanceDB configured without an embedding provider; vector retrieval degraded"
                .to_owned(),
        },
    )
}

fn vector_store_status_from_base(
    conn: &Connection,
    board_id: &str,
    mut status: VectorStoreStatus,
) -> Result<VectorStoreStatus> {
    let state = derived_status_by_name(conn, LANCEDB_CHUNKS_STORE)?;
    let current_last_event_id = current_last_event_id(conn, board_id)?;
    let board_has_pending =
        has_pending_vector_outbox_for_board(conn, board_id, current_last_event_id)?;
    let lag = if board_has_pending {
        search_lag(current_last_event_id, Some(state.last_event_id))
    } else {
        0
    };
    status.message = format!(
        "{}; dirty={} last_event_id={} lag={} last_error={}",
        status.message,
        state.dirty,
        state.last_event_id,
        lag,
        state.last_error.as_deref().unwrap_or("none")
    );
    Ok(status)
}

#[cfg(any(feature = "graph-oxigraph", feature = "vector-lancedb"))]
fn has_pending_outbox_for_target(
    conn: &Connection,
    target: OutboxTarget,
    board_id: &str,
    last_event_id: Option<i64>,
) -> Result<bool> {
    let target = target.as_str();
    conn.query_row(
        "SELECT EXISTS( \
             SELECT 1 FROM index_outbox o \
             JOIN task_events e ON e.id=o.source_event_id \
             WHERE o.target IN (?1, 'all') \
               AND o.status IN ('pending', 'running', 'failed') \
               AND e.board_id=?2 \
               AND e.id <= ?3 \
         )",
        params![target, board_id, last_event_id.unwrap_or(i64::MAX)],
        |row| row.get::<_, bool>(0),
    )
    .map_err(storage)
}

fn pending_vector_outbox_for_board(
    conn: &Connection,
    board_id: &str,
    last_event_id: Option<i64>,
) -> Result<Vec<IndexOutboxRecord>> {
    let mut stmt = conn
        .prepare(
            "SELECT o.id,o.source_event_id,o.target,o.entity_uri,o.action,o.payload_json,o.status,o.attempts,o.last_error,o.created_at,o.updated_at \
             FROM index_outbox o \
             JOIN task_events e ON e.id=o.source_event_id \
             WHERE o.target IN ('lancedb', 'all') \
               AND o.status IN ('pending', 'running', 'failed') \
               AND e.board_id=?1 \
               AND e.id <= ?2 \
             ORDER BY o.id ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map(
            params![board_id, last_event_id.unwrap_or(i64::MAX)],
            outbox_from_row,
        )
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn has_pending_vector_outbox_for_board(
    conn: &Connection,
    board_id: &str,
    last_event_id: Option<i64>,
) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS( \
             SELECT 1 FROM index_outbox o \
             JOIN task_events e ON e.id=o.source_event_id \
             WHERE o.target IN ('lancedb', 'all') \
               AND o.status IN ('pending', 'running', 'failed') \
               AND e.board_id=?1 \
               AND e.id <= ?2 \
         )",
        params![board_id, last_event_id.unwrap_or(i64::MAX)],
        |row| row.get::<_, bool>(0),
    )
    .map_err(storage)
}

fn vector_chunks_for_board(
    conn: &Connection,
    board_id: &str,
    embedding_model: &str,
) -> Result<Vec<kanban_vector::EmbeddingChunk>> {
    let mut stmt = conn
        .prepare(
            "SELECT 'kb://task/' || t.id,t.board_id,t.id,t.title,t.description,\
                    (SELECT MAX(e.id) FROM task_events e WHERE e.board_id=t.board_id AND e.task_id=t.id),\
                    t.created_at,t.updated_at \
             FROM tasks t WHERE t.board_id=?1 AND t.archived_at IS NULL ORDER BY t.seq ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map([board_id], task_chunk_source_from_row)
        .map_err(storage)?;
    let sources = rows
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    build_vector_chunks(&sources, embedding_model)
}

fn vector_chunks_for_entity_uris(
    conn: &Connection,
    board_id: &str,
    entity_uris: &[String],
    embedding_model: &str,
) -> Result<Vec<kanban_vector::EmbeddingChunk>> {
    let mut task_ids = entity_uris
        .iter()
        .filter_map(|uri| uri.strip_prefix("kb://task/").map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    task_ids.sort();
    task_ids.dedup();
    if task_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat_n("?", task_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT 'kb://task/' || t.id,t.board_id,t.id,t.title,t.description,\
                (SELECT MAX(e.id) FROM task_events e WHERE e.board_id=t.board_id AND e.task_id=t.id),\
                t.created_at,t.updated_at \
         FROM tasks t WHERE t.board_id=? AND t.archived_at IS NULL AND t.id IN ({placeholders}) ORDER BY t.seq ASC"
    );
    let mut params = vec![Value::Text(board_id.to_owned())];
    params.extend(task_ids.into_iter().map(Value::Text));
    let mut stmt = conn.prepare(&sql).map_err(storage)?;
    let rows = stmt
        .query_map(params_from_iter(params.iter()), task_chunk_source_from_row)
        .map_err(storage)?;
    let sources = rows
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    build_vector_chunks(&sources, embedding_model)
}

fn task_chunk_source_from_row(row: &Row<'_>) -> rusqlite::Result<TaskChunkSource> {
    Ok(TaskChunkSource {
        task_uri: row.get(0)?,
        project_id: None,
        board_id: row.get(1)?,
        task_id: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        source_event_id: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn build_vector_chunks(
    sources: &[TaskChunkSource],
    embedding_model: &str,
) -> Result<Vec<kanban_vector::EmbeddingChunk>> {
    let builder = ChunkBuilder::new(embedding_model);
    let mut chunks = Vec::new();
    for source in sources {
        chunks.extend(builder.build_task_chunks(source).map_err(vector_storage)?);
    }
    Ok(chunks)
}

#[cfg(feature = "vector-lancedb")]
fn vector_store_path(db_path: &Path) -> PathBuf {
    kanban_local::vector_store_path(db_path.to_path_buf())
}

fn vector_storage(error: impl std::fmt::Display) -> KanbanError {
    KanbanError::Storage(error.to_string())
}

fn get_board_conn(conn: &Connection, slug_or_id: &str) -> Result<BoardRecord> {
    let sql = if slug_or_id.starts_with("b_") {
        "SELECT id,slug,name,description,created_at,updated_at,archived_at FROM boards WHERE id=?1 AND archived_at IS NULL"
    } else {
        "SELECT id,slug,name,description,created_at,updated_at,archived_at FROM boards WHERE slug=?1 AND archived_at IS NULL"
    };
    conn.query_row(sql, [slug_or_id], board_from_row)
        .optional()
        .map_err(storage)?
        .ok_or_else(|| KanbanError::NotFound(format!("board {slug_or_id}")))
}

fn board_from_row(row: &Row<'_>) -> rusqlite::Result<BoardRecord> {
    Ok(BoardRecord {
        id: row.get(0)?,
        slug: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        archived_at: row.get(6)?,
    })
}

fn board_column_from_row(row: &Row<'_>) -> rusqlite::Result<BoardColumnRecord> {
    let status: String = row.get(2)?;
    let hidden: i64 = row.get(5)?;
    Ok(BoardColumnRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        status: TaskStatus::try_from(status.as_str())
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        title: row.get(3)?,
        position: row.get(4)?,
        hidden: hidden != 0,
        wip_limit: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn comment_from_row(row: &Row<'_>) -> rusqlite::Result<CommentRecord> {
    Ok(CommentRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        task_id: row.get(2)?,
        author: row.get(3)?,
        body: row.get(4)?,
        kind: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn board_id(conn: &Connection, slug_or_id: &str) -> Result<String> {
    let sql = if slug_or_id.starts_with("b_") {
        "SELECT id FROM boards WHERE id=?1"
    } else {
        "SELECT id FROM boards WHERE slug=?1"
    };
    conn.query_row(sql, [slug_or_id], |r| r.get(0))
        .optional()
        .map_err(storage)?
        .ok_or_else(|| KanbanError::NotFound(format!("board {slug_or_id}")))
}

fn board_id_for_task(conn: &Connection, task_id: &str) -> Result<String> {
    conn.query_row("SELECT board_id FROM tasks WHERE id=?1", [task_id], |r| {
        r.get(0)
    })
    .optional()
    .map_err(storage)?
    .ok_or_else(|| KanbanError::NotFound(format!("task {task_id}")))
}

#[allow(clippy::too_many_arguments)]
fn insert_event(
    conn: &Connection,
    board_id: &str,
    task_id: Option<&str>,
    run_id: Option<&str>,
    kind: &str,
    actor: &str,
    payload: &str,
    now: i64,
) -> Result<()> {
    let event_id = new_event_id();
    conn.execute("INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", params![event_id, board_id, task_id, run_id, kind, actor, payload, now]).map_err(storage)?;
    let source_event_id = conn.last_insert_rowid();
    upsert_board_entity(conn, board_id)?;
    upsert_event_entity(conn, &event_id, board_id, task_id, kind, payload, now)?;
    if let Some(task_id) = task_id {
        upsert_task_entity(conn, task_id)?;
    }
    if let Some(run_id) = run_id {
        upsert_run_entity(conn, run_id)?;
    }
    let entity_uri = task_id
        .map(|task_id| format!("kb://task/{task_id}"))
        .or_else(|| run_id.map(|run_id| format!("kb://run/{run_id}")))
        .unwrap_or_else(|| format!("kb://board/{board_id}"));
    enqueue_index_outbox(conn, source_event_id, &entity_uri, "upsert", now)?;
    Ok(())
}

fn upsert_board_entity(conn: &Connection, board_id: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         SELECT 'kb://board/' || id, 'board', 'boards', id, id, NULL, name, description, NULL, created_at, updated_at, archived_at FROM boards WHERE id=?1 \
         ON CONFLICT(uri) DO UPDATE SET kind=excluded.kind, source_table=excluded.source_table, source_id=excluded.source_id, board_id=excluded.board_id, task_id=excluded.task_id, title=excluded.title, summary=excluded.summary, content_hash=excluded.content_hash, updated_at=excluded.updated_at, archived_at=excluded.archived_at",
        [board_id],
    )
    .map_err(storage)?;
    Ok(())
}

fn upsert_event_entity(
    conn: &Connection,
    event_id: &str,
    board_id: &str,
    task_id: Option<&str>,
    kind: &str,
    payload: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         VALUES (?1, 'event', 'task_events', ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?7, NULL)",
        params![
            format!("kb://event/{event_id}"),
            event_id,
            board_id,
            task_id,
            kind,
            payload,
            now
        ],
    )
    .map_err(storage)?;
    Ok(())
}

fn upsert_task_entity(conn: &Connection, task_id: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         SELECT 'kb://task/' || id, 'task', 'tasks', id, board_id, id, title, description, NULL, created_at, updated_at, archived_at FROM tasks WHERE id=?1 \
         ON CONFLICT(uri) DO UPDATE SET kind=excluded.kind, source_table=excluded.source_table, source_id=excluded.source_id, board_id=excluded.board_id, task_id=excluded.task_id, title=excluded.title, summary=excluded.summary, content_hash=excluded.content_hash, updated_at=excluded.updated_at, archived_at=excluded.archived_at",
        [task_id],
    )
    .map_err(storage)?;
    upsert_task_board_relation(conn, task_id)?;
    Ok(())
}

fn upsert_run_entity(conn: &Connection, run_id: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) \
         SELECT 'kb://run/' || id, 'run', 'task_runs', id, board_id, task_id, id, COALESCE(summary, error), NULL, started_at, COALESCE(finished_at, last_heartbeat_at, started_at), NULL FROM task_runs WHERE id=?1",
        [run_id],
    )
    .map_err(storage)?;
    Ok(())
}

fn upsert_task_board_relation(conn: &Connection, task_id: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO entity_relations(subject_uri, predicate, object_uri, graph_uri, authoritative_store, source_table, source_id, source_event_id, metadata_json, created_at, updated_at) \
         SELECT 'kb://task/' || id, 'belongs_to_board', 'kb://board/' || board_id, 'kb://graph/indexed', 'sqlite', 'tasks', id, NULL, '{}', created_at, updated_at \
         FROM tasks WHERE id=?1",
        [task_id],
    )
    .map_err(storage)?;
    Ok(())
}

fn upsert_dependency_relation(
    conn: &Connection,
    parent_task_id: &str,
    child_task_id: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO entity_relations(subject_uri, predicate, object_uri, graph_uri, authoritative_store, source_table, source_id, source_event_id, metadata_json, created_at, updated_at) \
         SELECT 'kb://task/' || child_task_id, 'depends_on', 'kb://task/' || parent_task_id, 'kb://graph/indexed', 'sqlite', 'task_dependencies', parent_task_id || '->' || child_task_id, NULL, '{}', created_at, ?3 \
         FROM task_dependencies WHERE parent_task_id=?1 AND child_task_id=?2",
        params![parent_task_id, child_task_id, now],
    )
    .map_err(storage)?;
    Ok(())
}

fn delete_dependency_relation(
    conn: &Connection,
    parent_task_id: &str,
    child_task_id: &str,
) -> Result<()> {
    conn.execute(
        "DELETE FROM entity_relations \
         WHERE subject_uri=?1 AND predicate='depends_on' AND object_uri=?2 AND source_table='task_dependencies'",
        params![
            format!("kb://task/{child_task_id}"),
            format!("kb://task/{parent_task_id}")
        ],
    )
    .map_err(storage)?;
    Ok(())
}

fn enqueue_index_outbox(
    conn: &Connection,
    source_event_id: i64,
    entity_uri: &str,
    action: &str,
    now: i64,
) -> Result<()> {
    for target in OUTBOX_FANOUT_TARGETS {
        conn.execute(
            "INSERT INTO index_outbox(source_event_id, target, entity_uri, action, payload_json, status, attempts, last_error, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, '{}', 'pending', 0, NULL, ?5, ?5)",
            params![source_event_id, target.as_str(), entity_uri, action, now],
        )
        .map_err(storage)?;
    }
    for seed in DERIVED_STORE_SEEDS {
        mark_derived_store_dirty(conn, seed.store_name, now)?;
    }
    Ok(())
}

fn mark_derived_store_dirty(conn: &Connection, store_name: &str, now: i64) -> Result<()> {
    let seed = derived_store_for_name(store_name)
        .ok_or_else(|| KanbanError::Storage(format!("unknown derived store: {store_name}")))?;
    let update = DerivedStoreUpdate::dirty(seed, now);
    conn.execute(
        "INSERT INTO derived_store_state(store_name, schema_version, last_event_id, dirty, last_rebuild_at, last_sync_at, last_error, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT(store_name) DO UPDATE SET dirty=1, updated_at=excluded.updated_at",
        params![
            store_name,
            update.schema_version,
            update.last_event_id,
            i64::from(update.dirty),
            update.last_rebuild_at,
            update.last_sync_at,
            update.last_error,
            update.updated_at
        ],
    )
    .map_err(storage)?;
    Ok(())
}

fn mark_derived_store_success(
    conn: &Connection,
    store_name: &str,
    board_id: &str,
    last_event_id: Option<i64>,
    rebuilt: bool,
    now: i64,
) -> Result<()> {
    let target = store_target(store_name)?;
    let seed = derived_store_for_name(store_name)
        .ok_or_else(|| KanbanError::Storage(format!("unknown derived store: {store_name}")))?;
    complete_outbox_for_store(conn, target, board_id, last_event_id, now)?;
    let dirty = has_unfinished_outbox_for_store(conn, target)?;
    let update = DerivedStoreUpdate::success(seed, last_event_id, rebuilt, now);
    conn.execute(
        "INSERT INTO derived_store_state(store_name, schema_version, last_event_id, dirty, last_rebuild_at, last_sync_at, last_error, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT(store_name) DO UPDATE SET last_event_id=MAX(derived_store_state.last_event_id, excluded.last_event_id), dirty=excluded.dirty, last_rebuild_at=COALESCE(excluded.last_rebuild_at, derived_store_state.last_rebuild_at), last_sync_at=COALESCE(excluded.last_sync_at, derived_store_state.last_sync_at), last_error=NULL, updated_at=excluded.updated_at",
        params![
            update.store_name,
            update.schema_version,
            update.last_event_id,
            i64::from(dirty),
            update.last_rebuild_at,
            update.last_sync_at,
            update.last_error,
            update.updated_at
        ],
    )
    .map_err(storage)?;
    Ok(())
}

fn mark_derived_store_failure(
    conn: &Connection,
    store_name: &str,
    board_id: &str,
    error: &str,
    now: i64,
) -> Result<()> {
    let target = store_target(store_name)?;
    let seed = derived_store_for_name(store_name)
        .ok_or_else(|| KanbanError::Storage(format!("unknown derived store: {store_name}")))?;
    let update = DerivedStoreUpdate::failure(seed, error, now);
    conn.execute(
        "INSERT INTO derived_store_state(store_name, schema_version, last_event_id, dirty, last_rebuild_at, last_sync_at, last_error, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT(store_name) DO UPDATE SET dirty=1, last_error=excluded.last_error, updated_at=excluded.updated_at",
        params![
            update.store_name,
            update.schema_version,
            update.last_event_id,
            i64::from(update.dirty),
            update.last_rebuild_at,
            update.last_sync_at,
            update.last_error,
            update.updated_at
        ],
    )
    .map_err(storage)?;
    fail_outbox_for_store(conn, target, board_id, error, now)?;
    Ok(())
}

fn store_target(store_name: &str) -> Result<OutboxTarget> {
    DERIVED_STORE_SEEDS
        .iter()
        .find(|seed| seed.store_name == store_name)
        .map(|seed| seed.target)
        .ok_or_else(|| KanbanError::Storage(format!("unknown derived store: {store_name}")))
}

fn complete_outbox_for_store(
    conn: &Connection,
    target: OutboxTarget,
    board_id: &str,
    last_event_id: Option<i64>,
    now: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE index_outbox \
         SET status='done', last_error=NULL, updated_at=?1 \
         WHERE target IN (?2, 'all') \
           AND status IN ('pending', 'running', 'failed') \
           AND source_event_id <= ?3 \
           AND EXISTS ( \
               SELECT 1 FROM task_events e \
               WHERE e.id=index_outbox.source_event_id AND e.board_id=?4 \
           )",
        params![
            now,
            target.as_str(),
            last_event_id.unwrap_or(i64::MAX),
            board_id
        ],
    )
    .map_err(storage)?;
    Ok(())
}

fn has_unfinished_outbox_for_store(conn: &Connection, target: OutboxTarget) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS( \
             SELECT 1 FROM index_outbox \
             WHERE target IN (?1, 'all') \
               AND status IN ('pending', 'running', 'failed') \
         )",
        [target.as_str()],
        |row| row.get::<_, i64>(0),
    )
    .map(|exists| exists != 0)
    .map_err(storage)
}

fn fail_outbox_for_store(
    conn: &Connection,
    target: OutboxTarget,
    board_id: &str,
    error: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE index_outbox \
         SET status='failed', attempts=attempts + 1, last_error=?1, updated_at=?2 \
         WHERE target IN (?3, 'all') \
           AND status IN ('pending', 'running') \
           AND EXISTS ( \
               SELECT 1 FROM task_events e \
               WHERE e.id=index_outbox.source_event_id AND e.board_id=?4 \
           )",
        params![error, now, target.as_str(), board_id],
    )
    .map_err(storage)?;
    Ok(())
}

fn json_valid(conn: &Connection, json: &str) -> Result<bool> {
    conn.query_row("SELECT json_valid(?1)", [json], |r| r.get::<_, i64>(0))
        .map(|v| v == 1)
        .map_err(storage)
}

fn with_immediate_tx<T>(conn: &Connection, f: impl FnOnce() -> Result<T>) -> Result<T> {
    conn.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
    match f() {
        Ok(value) => {
            conn.execute_batch("COMMIT").map_err(storage)?;
            Ok(value)
        }
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

fn with_read_tx<T>(conn: &Connection, f: impl FnOnce() -> Result<T>) -> Result<T> {
    conn.execute_batch("BEGIN").map_err(storage)?;
    match f() {
        Ok(value) => {
            conn.execute_batch("COMMIT").map_err(storage)?;
            Ok(value)
        }
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

fn storage(err: rusqlite::Error) -> KanbanError {
    KanbanError::Storage(err.to_string())
}

#[cfg(feature = "tantivy-backend")]
fn search_storage(err: impl std::error::Error) -> KanbanError {
    KanbanError::Storage(err.to_string())
}
