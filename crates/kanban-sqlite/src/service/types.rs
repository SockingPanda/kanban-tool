use std::{fs, path::PathBuf};

use kanban_core::TaskStatus;
use serde::{Deserialize, Serialize};

pub const DEFAULT_PRIORITY: i64 = 3;
pub const PRIORITY_ERROR: &str = "priority must be one of P0, P1, P2, P3";

pub fn default_priority() -> i64 {
    DEFAULT_PRIORITY
}

pub fn validate_priority(priority: i64) -> kanban_core::Result<()> {
    if (0..=3).contains(&priority) {
        return Ok(());
    }
    Err(kanban_core::KanbanError::InvalidInput(
        PRIORITY_ERROR.to_owned(),
    ))
}

pub fn normalize_legacy_priority(priority: i64) -> i64 {
    priority.clamp(0, 3)
}

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
    pub dependency_blocked: bool,
    pub unfinished_parent_count: i64,
    pub labels: Vec<LabelRecord>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelSemanticsRecord {
    pub label_id: String,
    pub board_id: String,
    pub label_name: String,
    pub description: Option<String>,
    pub applies_when: Vec<String>,
    pub excludes_when: Vec<String>,
    pub positive_examples: Vec<String>,
    pub negative_examples: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub atoms: Vec<LabelAtomRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BootstrapTaskLabel {
    pub name: String,
    pub description: Option<String>,
    pub applies_when: Vec<String>,
    pub excludes_when: Vec<String>,
    pub positive_examples: Vec<String>,
    pub negative_examples: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapTaskLabelResult {
    pub task: TaskRecord,
    pub semantics: LabelSemanticsRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelAtomRecord {
    pub id: String,
    pub label_id: String,
    pub board_id: String,
    pub label_name: String,
    pub polarity: String,
    pub kind: String,
    pub text: String,
    pub ordinal: i64,
    pub content_hash: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelProposalStatus {
    Proposed,
    Accepted,
    Rejected,
}

impl std::fmt::Display for LabelProposalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Proposed => "proposed",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        };
        f.write_str(value)
    }
}

impl std::str::FromStr for LabelProposalStatus {
    type Err = kanban_core::KanbanError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "proposed" => Ok(Self::Proposed),
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            _ => Err(kanban_core::KanbanError::InvalidInput(format!(
                "invalid label proposal status: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LabelProposalCandidate {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub applies_when: Vec<String>,
    #[serde(default)]
    pub excludes_when: Vec<String>,
    #[serde(default)]
    pub positive_examples: Vec<String>,
    #[serde(default)]
    pub negative_examples: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelSemanticProposalRecord {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub status: LabelProposalStatus,
    pub name: String,
    pub description: Option<String>,
    pub applies_when: Vec<String>,
    pub excludes_when: Vec<String>,
    pub positive_examples: Vec<String>,
    pub negative_examples: Vec<String>,
    pub heuristic_coverage: f32,
    pub heuristic_coverage_cosine: f32,
    pub heuristic_residual_norm: f32,
    pub top1_existing_label_id: Option<String>,
    pub top1_existing_label_name: Option<String>,
    pub diagnostics: Vec<String>,
    pub created_by: String,
    pub decision_reason: Option<String>,
    pub resolved_label_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub decided_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelProposalAttempt {
    pub task_id: String,
    pub board_id: String,
    pub proposal: Option<LabelSemanticProposalRecord>,
    pub degraded: bool,
    pub diagnostics: Vec<String>,
    pub heuristic_coverage: f32,
    pub heuristic_coverage_cosine: f32,
    pub heuristic_residual_norm: f32,
    pub top1_existing_label_id: Option<String>,
    pub top1_existing_label_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LabelProposalListOptions {
    pub task_ref: Option<String>,
    pub status: Option<LabelProposalStatus>,
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
    pub metadata_json: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateComment {
    pub author: String,
    pub body: String,
    pub kind: Option<String>,
    pub author_type: Option<String>,
    pub agent_type: Option<String>,
    pub metadata_json: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateLabel {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpsertLabelSemantics {
    pub label_ref: String,
    pub description: Option<String>,
    pub applies_when: Vec<String>,
    pub excludes_when: Vec<String>,
    pub positive_examples: Vec<String>,
    pub negative_examples: Vec<String>,
}

pub const DEFAULT_LABEL_SUGGESTION_OUTPUT_LIMIT: usize = 5;
pub const DEFAULT_LABEL_SUGGESTION_CANDIDATE_LIMIT: usize = 32;
pub const DEFAULT_LABEL_SUGGESTION_ATOM_LIMIT: usize = 80;
pub const DEFAULT_LABEL_SUGGESTION_MAX_SELECTED_LABELS: usize = 4;
pub const DEFAULT_LABEL_SUGGESTION_MIN_SCORE: f32 = 0.15;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelSuggestionOptions {
    pub output_limit: usize,
    pub candidate_limit: usize,
    pub atom_limit: usize,
    pub max_selected_labels: usize,
    pub min_score: f32,
}

impl Default for LabelSuggestionOptions {
    fn default() -> Self {
        Self {
            output_limit: DEFAULT_LABEL_SUGGESTION_OUTPUT_LIMIT,
            candidate_limit: DEFAULT_LABEL_SUGGESTION_CANDIDATE_LIMIT,
            atom_limit: DEFAULT_LABEL_SUGGESTION_ATOM_LIMIT,
            max_selected_labels: DEFAULT_LABEL_SUGGESTION_MAX_SELECTED_LABELS,
            min_score: DEFAULT_LABEL_SUGGESTION_MIN_SCORE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelSuggestionResult {
    pub task_id: String,
    pub board_id: String,
    pub selected_labels: Vec<SelectedLabelSuggestion>,
    pub candidates: Vec<LabelSuggestionCandidate>,
    pub coverage: f32,
    pub coverage_cosine: f32,
    pub residual_norm: f32,
    pub needs_new_label: bool,
    pub degraded: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectedLabelSuggestion {
    pub label_id: String,
    pub label_name: String,
    pub score: f32,
    pub weight: f32,
    pub already_applied: bool,
    pub evidence_atoms: Vec<LabelSuggestionEvidenceAtom>,
    pub negative_evidence_atoms: Vec<LabelSuggestionEvidenceAtom>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelSuggestionCandidate {
    pub label_id: String,
    pub label_name: String,
    pub score: f32,
    pub weight: f32,
    pub already_applied: bool,
    pub evidence_atoms: Vec<LabelSuggestionEvidenceAtom>,
    pub negative_evidence_atoms: Vec<LabelSuggestionEvidenceAtom>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelSuggestionEvidenceAtom {
    pub atom_id: String,
    pub label_id: String,
    pub label_name: String,
    pub polarity: String,
    pub kind: String,
    pub text: String,
    pub score: f32,
}

impl CreateTask {
    pub fn ready(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: Some("ready spec".to_owned()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: DEFAULT_PRIORITY,
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
    Seq,
    SeqDesc,
    Title,
    TitleDesc,
    Status,
    StatusDesc,
    Position,
    PositionDesc,
    Priority,
    PriorityDesc,
    Assignee,
    AssigneeDesc,
    ScheduledAt,
    ScheduledAtDesc,
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
    pub priorities: Vec<i64>,
    pub labels: Vec<String>,
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
