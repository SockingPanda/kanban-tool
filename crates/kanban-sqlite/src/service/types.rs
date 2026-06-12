use std::{fs, path::PathBuf};

use kanban_core::TaskStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub board_id: String,
    pub board_slug: String,
    #[serde(rename = "ref")]
    pub task_ref: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateBoard {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BoardListOptions {
    pub include_archived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentRecord {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub author: String,
    pub author_type: String,
    pub agent_type: Option<String>,
    pub body: String,
    pub kind: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateComment {
    pub author: String,
    pub body: String,
    pub kind: Option<String>,
    pub author_type: Option<String>,
    pub agent_type: Option<String>,
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
    pub max_retries: Option<i64>,
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
            max_retries: None,
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
    pub max_retries: Option<Option<i64>>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DagSnapshot {
    pub board: DagBoardSnapshot,
    pub snapshot: DagSnapshotMeta,
    pub raw: DagRawGraph,
    pub derived: DagDerivedGraph,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DagAncestors {
    pub target: DagNode,
    pub nodes: Vec<DagNode>,
    pub edges: Vec<DagEdge>,
    pub ordered_refs: Vec<String>,
    pub generated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DagBoardSnapshot {
    pub id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DagSnapshotMeta {
    pub generated_at: i64,
    pub node_count: usize,
    pub edge_count: usize,
    pub sort: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DagRawGraph {
    pub nodes: Vec<DagNode>,
    pub edges: Vec<DagEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DagDerivedGraph {
    pub blocked_by: Vec<DagAdjacency>,
    pub unblocks: Vec<DagAdjacency>,
    pub actionable: Vec<DagTaskReason>,
    pub frontier: Vec<DagTaskReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DagNode {
    pub id: String,
    #[serde(rename = "ref")]
    pub task_ref: String,
    pub seq: i64,
    pub title: String,
    pub status: TaskStatus,
    pub priority: i64,
    pub due_at: Option<i64>,
    pub scheduled_at: Option<i64>,
    pub created_at: i64,
    pub archived_at: Option<i64>,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DagEdge {
    pub parent: String,
    pub child: String,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DagAdjacency {
    pub task_id: String,
    pub tasks: Vec<String>,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DagTaskReason {
    pub task_id: String,
    #[serde(rename = "ref")]
    pub task_ref: String,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventListOptions {
    pub task_ref: Option<String>,
    pub after: i64,
    pub limit: usize,
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
    pub(super) lock_path: PathBuf,
}

impl Drop for DatabaseReplaceGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

#[derive(Debug)]
pub struct DatabaseRuntimeGuard {
    pub(super) lock_path: PathBuf,
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
