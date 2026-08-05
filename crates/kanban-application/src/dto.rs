use kanban_core::{Board, TaskStatus};
use serde::{Deserialize, Serialize};

pub type BoardRecord = Board;

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
pub struct ApplicationHealth {
    pub ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelRecord {
    pub id: String,
    pub board_id: String,
    pub name: String,
    pub color: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPlanState {
    Unplanned,
    Planned,
    NotRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Succeeded,
    Failed,
    Canceled,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRecord {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub status: RunStatus,
    pub worker_profile: Option<String>,
    pub worker_pid: Option<i64>,
    pub claim_owner: String,
    pub claim_expires_at: i64,
    pub started_at: i64,
    pub last_heartbeat_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub exit_code: Option<i64>,
    pub summary: Option<String>,
    pub error: Option<String>,
    pub log_path: Option<String>,
    pub metadata_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimRecord {
    pub task: TaskRecord,
    pub run: RunRecord,
    pub claim_token: String,
    pub claim_expires_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentKind {
    Note,
    Decision,
    Signal,
}

impl CommentKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Decision => "decision",
            Self::Signal => "signal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentAuthorType {
    User,
    Agent,
}

impl CommentAuthorType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Agent => "agent",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentRecord {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub author: String,
    pub author_type: CommentAuthorType,
    pub agent_type: Option<String>,
    pub body: String,
    pub kind: CommentKind,
    pub metadata_json: String,
    pub created_at: i64,
}

/// 附件内容不进入 DTO；host 仅返回受控读取得到的字节，canonical 数据库存元数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentRecord {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub filename: String,
    pub rel_path: String,
    pub content_type: Option<String>,
    pub size_bytes: i64,
    pub sha256: Option<String>,
    pub created_by: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentContentRecord {
    pub attachment: AttachmentRecord,
    pub content: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyEdgeRecord {
    pub parent: TaskRecord,
    pub child: TaskRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencySnapshotRecord {
    pub task: TaskRecord,
    pub parents: Vec<TaskRecord>,
    pub children: Vec<TaskRecord>,
    pub edges: Vec<DependencyEdgeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepRecord {
    pub id: String,
    pub parent_task_id: String,
    pub title: String,
    pub body: Option<String>,
    pub linked_task: Option<TaskRecord>,
    pub position: i64,
    pub required: bool,
    pub status: String,
    pub resolution_note: Option<String>,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<i64>,
    pub created_by: String,
    pub created_at: i64,
    pub updated_by: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStepsRecord {
    pub task_id: String,
    pub steps: Vec<StepRecord>,
    pub execution_plan: ExecutionPlanRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlanRecord {
    pub board_id: String,
    pub task_id: String,
    pub state: ExecutionPlanState,
    pub reason: Option<String>,
    pub updated_by: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRecord {
    pub id: String,
    pub board_id: String,
    pub board_slug: String,
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
    pub has_claim_token: bool,
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
    pub dependency_blocked: bool,
    pub unfinished_parent_count: i64,
    pub execution_plan_state: ExecutionPlanState,
    pub required_step_count: i64,
    pub completed_required_step_count: i64,
    pub optional_step_count: i64,
    pub labels: Vec<LabelRecord>,
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
pub struct RelationPredicateRecord {
    pub name: String,
    pub domain_kind: Option<String>,
    pub range_kind: Option<String>,
    pub cardinality: String,
    pub authoritative_store: String,
    pub description: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationRecord {
    pub id: i64,
    pub subject_uri: String,
    pub predicate: String,
    pub object_uri: String,
    pub graph_uri: String,
    pub board_id: Option<String>,
    pub authoritative_store: String,
    pub source_table: Option<String>,
    pub source_id: Option<String>,
    pub source_event_id: Option<i64>,
    pub metadata_json: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionStateRecord {
    pub projection: String,
    pub lifecycle_status: String,
    pub active_generation: Option<String>,
    pub active_fingerprint: Option<String>,
    pub last_event_id: i64,
    pub dirty: bool,
    pub last_success_at: Option<i64>,
    pub last_error: Option<String>,
    pub updated_at: i64,
    pub pending_jobs: i64,
    pub running_jobs: i64,
    pub failed_jobs: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphStatusRecord {
    pub backend: String,
    pub enabled: bool,
    pub message: String,
    pub projection: ProjectionStateRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphQueryBindingRecord {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphQueryRowRecord {
    pub bindings: Vec<GraphQueryBindingRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskGraphNodeRole {
    Center,
    DependencyParent,
    DependencyChild,
    StepParent,
    StepChild,
    Active,
    Context,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskGraphEdgeKind {
    Dependency,
    Step,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskGraphNodeRecord {
    pub task: TaskRecord,
    pub role: TaskGraphNodeRole,
    pub context_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGraphEdgeRecord {
    pub id: String,
    pub source_task_id: String,
    pub target_task_id: String,
    pub kind: TaskGraphEdgeKind,
    pub required: bool,
    pub blocking: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGraphMetaRecord {
    pub depth: usize,
    pub context_depth: usize,
    pub generated_at: i64,
    pub node_count: usize,
    pub edge_count: usize,
    pub truncated: bool,
    pub active_statuses: Vec<TaskStatus>,
    pub active_only: bool,
    pub include_done_context: bool,
    pub include_archived_context: bool,
    pub hide_isolated: bool,
    pub limit_nodes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskNeighborhoodRecord {
    pub center_task_id: String,
    pub nodes: Vec<TaskGraphNodeRecord>,
    pub edges: Vec<TaskGraphEdgeRecord>,
    pub meta: TaskGraphMetaRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardTaskMapRecord {
    pub nodes: Vec<TaskGraphNodeRecord>,
    pub edges: Vec<TaskGraphEdgeRecord>,
    pub meta: TaskGraphMetaRecord,
}

// Compatibility facade for callers that historically imported all DTOs from
// `kanban_application::dto`. Definitions now live next to their operation.
pub use crate::operations::{
    AddDependencyCommand, AddDependencyRecord, AddDependencyResult, ArchiveBoardCommand,
    ArchiveBoardRecord, ArchiveTaskCommand, ArchiveTaskRecord, BlockTaskCommand, BlockTaskRecord,
    BlockedReasonCountRecord, ClaimTaskCommand, ClaimTaskRecord, CompleteStepCommand,
    CompleteStepRecord, CompleteTaskCommand, CompleteTaskRecord, CreateBoardCommand,
    CreateBoardRecord, CreateCommentCommand, CreateCommentRecord, CreateStepCommand,
    CreateStepRecord, CreateTaskCommand, CreateTaskRecord, HeartbeatTaskCommand,
    HeartbeatTaskRecord, MarkExecutionPlanNotRequiredCommand, MarkExecutionPlanNotRequiredRecord,
    PromoteTaskCommand, PromoteTaskRecord, QueueStatsRecord, ReclaimExpiredTaskRecord,
    ReclaimTaskCommand, ReclaimTaskRecord, ReleaseTaskCommand, ReleaseTaskRecord,
    RemoveDependencyCommand, RemoveDependencyResult, RemoveStepCommand, RemoveStepRecord,
    ReopenStepCommand, ReopenStepRecord, ReopenTaskCommand, ReopenTaskRecord, SignalCreateRecord,
    SignalLedger, SignalLifecycle, SignalListOptions, SignalObservationRecord, SignalRecord,
    SignalRecordCommand, SignalRecordResult, SignalReviewCommand, SignalReviewRecord, SignalStatus,
    SkipStepCommand, SkipStepRecord, SpecifyTaskCommand, SpecifyTaskRecord, StaleClaimRecord,
    StatsQuery, StatusCountRecord, SubmitReviewTaskCommand, SubmitReviewTaskRecord,
    TaskListOptions, TaskListPage, TaskListSort, TaskPlanFilter, UnblockTaskCommand,
    UnblockTaskRecord, UpdateStepCommand, UpdateStepRecord, UpdateTaskCommand, UpdateTaskRecord,
};
