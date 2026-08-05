use std::collections::BTreeMap;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPlanState {
    Unplanned,
    Planned,
    NotRequired,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTaskCommand {
    pub task_id: String,
    pub board: String,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub requested_status: Option<TaskStatus>,
    pub assignee: Option<String>,
    pub priority: i64,
    pub scheduled_at: Option<i64>,
    pub due_at: Option<i64>,
    pub max_retries: Option<i64>,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub actor: String,
}

/// Canonicalized input passed from the application service to persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTaskRecord {
    pub id: String,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub assignee: Option<String>,
    pub priority: i64,
    pub scheduled_at: Option<i64>,
    pub due_at: Option<i64>,
    pub max_retries: Option<i64>,
    pub metadata_json: String,
    pub created_by: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkExecutionPlanNotRequiredCommand {
    pub task_id: String,
    pub reason: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkExecutionPlanNotRequiredRecord {
    pub reason: String,
    pub actor: String,
    pub event_id: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromoteTaskCommand {
    pub task_id: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromoteTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub event_id: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaimTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub ttl_ms: i64,
    pub worker_profile: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub claim_token: String,
    pub run_id: String,
    pub event_id: String,
    pub worker_profile: String,
    pub metadata_json: String,
    pub log_path: Option<String>,
    pub now: i64,
    pub claim_expires_at: i64,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub claim_token: String,
    pub ttl_ms: i64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub claim_token: String,
    pub event_id: String,
    pub note: Option<String>,
    pub now: i64,
    pub claim_expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub claim_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub claim_token: String,
    pub event_id: String,
    pub now: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReclaimExpiredTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub event_id: String,
    pub target_status: TaskStatus,
    pub retry_count: i64,
    pub reason: String,
    pub now: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitReviewTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub claim_token: Option<String>,
    pub force: bool,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitReviewTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub claim_token: Option<String>,
    pub force: bool,
    pub summary: Option<String>,
    pub event_id: String,
    pub now: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompleteTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub claim_token: Option<String>,
    pub force: bool,
    pub summary: Option<String>,
    pub result: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub claim_token: Option<String>,
    pub force: bool,
    pub summary: Option<String>,
    pub result_json: Option<String>,
    pub event_id: String,
    pub now: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub reason: String,
    pub claim_token: Option<String>,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub reason: String,
    pub claim_token: Option<String>,
    pub force: bool,
    pub event_id: String,
    pub now: i64,
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

#[derive(Debug, Clone, PartialEq)]
pub struct CreateCommentCommand {
    pub task_id: String,
    pub idempotency_key: Option<String>,
    pub author: String,
    pub author_type: CommentAuthorType,
    pub agent_type: Option<String>,
    pub body: String,
    pub kind: CommentKind,
    pub metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateCommentRecord {
    pub id: String,
    pub idempotency_key: Option<String>,
    pub author: String,
    pub author_type: CommentAuthorType,
    pub agent_type: Option<String>,
    pub body: String,
    pub kind: CommentKind,
    pub metadata_json: String,
    pub event_id: String,
    pub created_at: i64,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddDependencyCommand {
    pub child_task_id: String,
    pub parent_task_id: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddDependencyRecord {
    pub expected_child_lock_version: i64,
    pub target_child_status: TaskStatus,
    pub actor: String,
    pub event_id: String,
    pub recompute_event_id: String,
    pub now: i64,
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
pub struct AddDependencyResult {
    pub added: bool,
    pub dependencies: DependencySnapshotRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveDependencyCommand {
    pub child_task_id: String,
    pub parent_task_id: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveDependencyResult {
    pub removed: bool,
    pub dependencies: DependencySnapshotRecord,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateStepCommand {
    pub task_id: String,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub body: Option<String>,
    pub linked_task_id: Option<String>,
    pub position: Option<i64>,
    pub required: bool,
    pub actor: String,
}

/// Canonicalized step mutation passed from the application service to the
/// Turso store. The expected task facts keep the transaction CAS-guarded even
/// if another caller changes the parent between the application read and the
/// store mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateStepRecord {
    pub id: String,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub body: Option<String>,
    pub linked_task_id: Option<String>,
    pub position: Option<i64>,
    pub required: bool,
    pub created_by: String,
    pub event_id: String,
    pub plan_event_id: String,
    pub recompute_event_id: String,
    pub created_at: i64,
    pub expected_lock_version: i64,
    pub expected_plan_state: ExecutionPlanState,
    pub target_status: TaskStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateStepCommand {
    pub task_id: String,
    pub step_id: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub linked_task_id: Option<String>,
    pub unlink_task: bool,
    pub position: Option<i64>,
    pub required: Option<bool>,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateStepRecord {
    pub title: Option<String>,
    pub body: Option<String>,
    pub linked_task_id: Option<String>,
    pub unlink_task: bool,
    pub position: Option<i64>,
    pub required: Option<bool>,
    pub updated_by: String,
    pub event_id: String,
    pub updated_at: i64,
    pub expected_lock_version: i64,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPlanFilter {
    PlanNeeded,
    HasSteps,
    IncompleteRequiredSteps,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TaskListSort {
    Seq,
    SeqDesc,
    Title,
    TitleDesc,
    Status,
    StatusDesc,
    #[default]
    Position,
    PositionDesc,
    Priority,
    PriorityDesc,
    Assignee,
    AssigneeDesc,
    ScheduledAt,
    ScheduledAtDesc,
    DueAt,
    DueAtDesc,
    CreatedAt,
    CreatedAtDesc,
    UpdatedAt,
    UpdatedAtDesc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListOptions {
    pub statuses: Vec<TaskStatus>,
    pub priorities: Vec<i64>,
    pub plan_filters: Vec<TaskPlanFilter>,
    pub assignee: Option<String>,
    pub query: Option<String>,
    pub include_archived: bool,
    pub limit: usize,
    pub offset: usize,
    pub sort: TaskListSort,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListPage {
    pub tasks: Vec<TaskRecord>,
    pub total: usize,
}
