#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardRecord {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardColumnRecord {
    pub id: String,
    pub board_id: String,
    pub status: String,
    pub title: String,
    pub position: i64,
    pub hidden: bool,
    pub wip_limit: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelRecord {
    pub id: String,
    pub board_id: String,
    pub name: String,
    pub color: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
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
pub struct CommentRecord {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub idempotency_key: Option<String>,
    pub author: String,
    pub author_type: String,
    pub agent_type: Option<String>,
    pub body: String,
    pub kind: String,
    pub metadata_json: String,
    pub created_at: i64,
}

/// 附件只在 canonical 数据库中保存元数据；内容由 host 管理的附件根目录保存。
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
pub struct SignalObservationRecord {
    pub id: String,
    pub board_id: String,
    pub task_id: Option<String>,
    pub task_ref_snapshot: Option<String>,
    pub run_id: Option<String>,
    pub comment_id: Option<String>,
    pub actor: String,
    pub agent_type: Option<String>,
    pub source: Option<String>,
    pub evidence_json: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalRecord {
    pub id: String,
    pub board_id: String,
    pub observation_id: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub severity: String,
    pub status: String,
    pub dedupe_key: Option<String>,
    pub superseded_by_signal_id: Option<String>,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<i64>,
    pub review_reason: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub observation: SignalObservationRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalRecordResult {
    pub signal: SignalRecord,
    pub backlink_comment: Option<CommentRecord>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStepRecord {
    pub id: String,
    pub board_id: String,
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
    pub steps: Vec<TaskStepRecord>,
    pub execution_plan: TaskExecutionPlanRecord,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskExecutionPlanRecord {
    pub board_id: String,
    pub task_id: String,
    pub state: String,
    pub reason: Option<String>,
    pub updated_by: String,
    pub updated_at: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRunRecord {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub status: String,
    pub worker_profile: Option<String>,
    pub worker_pid: Option<i64>,
    pub claim_token: String,
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
pub struct TaskEventRecord {
    pub id: i64,
    pub event_id: String,
    pub board_id: String,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub kind: String,
    pub actor: Option<String>,
    pub payload_json: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskEventListPage {
    pub events: Vec<TaskEventRecord>,
    pub next_after: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusCountRecord {
    pub status: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedReasonCountRecord {
    pub reason: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueStatsRecord {
    pub board_id: String,
    pub generated_at: i64,
    pub status_counts: Vec<StatusCountRecord>,
    pub stale_claims: Vec<StaleClaimRecord>,
    pub blocked_reasons: Vec<BlockedReasonCountRecord>,
    pub unplanned_active_tasks: i64,
    pub active_parents_with_incomplete_required_steps: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRecord {
    pub id: String,
    pub board_id: String,
    pub board_slug: String,
    pub task_ref: String,
    pub seq: i64,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
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
    pub execution_plan_state: String,
    pub required_step_count: i64,
    pub completed_required_step_count: i64,
    pub optional_step_count: i64,
    pub labels: Vec<LabelRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListPage {
    pub tasks: Vec<TaskRecord>,
    pub total: usize,
}

// Ontology records are intentionally kept as store-owned values.  JSON fields
// remain opaque here; the application/contract adapters validate their shape
// at the transport boundary while Turso enforces `json_valid` and board FKs.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LabelSemanticsRecord {
    pub label_id: String,
    pub board_id: String,
    pub label_name: String,
    pub semantics_hash: String,
    pub description: Option<String>,
    pub applies_when: Vec<String>,
    pub excludes_when: Vec<String>,
    pub positive_examples: Vec<String>,
    pub negative_examples: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub atoms: Vec<LabelAtomRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
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

/// Canonical knowledge-substrate entity.  This record is a fact row; graph
/// projections must be rebuildable from it and must never become the owner of
/// the task state.
#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationPredicateRecord {
    pub name: String,
    pub domain_kind: Option<String>,
    pub range_kind: Option<String>,
    pub cardinality: String,
    pub authoritative_store: String,
    pub description: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LabelAtomExplainActionRecord {
    pub action: LabelOntologyActionRecord,
    pub matched_by: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LabelAtomExplainSignalRecord {
    pub signal: LabelOntologySignalRecord,
    pub observation: LabelOntologyObservationRecord,
    pub task_id: String,
    pub task_ref_snapshot: String,
    pub suggest_input_stale: bool,
    pub suggest_degraded: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LabelAtomExplainValidationRecord {
    pub action: LabelOntologyActionRecord,
    pub parent_action_id: String,
    pub validation_status: String,
    pub manual_json: String,
    pub summary_json: String,
    pub cases_json: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LabelAtomExplainRecord {
    pub query: String,
    pub atom: Option<LabelAtomRecord>,
    pub current_semantics: Option<LabelSemanticsRecord>,
    pub provenance_actions: Vec<LabelAtomExplainActionRecord>,
    pub supporting_signals: Vec<LabelAtomExplainSignalRecord>,
    pub validation_history: Vec<LabelAtomExplainValidationRecord>,
    pub legacy_untracked: bool,
    pub legacy_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LabelAtomIndexStatusRecord {
    pub backend: String,
    pub enabled: bool,
    pub message: String,
    pub diagnostics: Vec<String>,
    pub dirty: Option<bool>,
    pub board_dirty: Option<bool>,
    pub generation: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LabelSuggestionResultRecord {
    pub task_id: String,
    pub board_id: String,
    pub selected_labels: Vec<LabelSuggestionCandidateRecord>,
    pub candidates: Vec<LabelSuggestionCandidateRecord>,
    pub coverage: f32,
    pub coverage_cosine: f32,
    pub residual_norm: f32,
    pub needs_new_label: bool,
    pub reason_codes: Vec<String>,
    pub degraded: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LabelSuggestionCandidateRecord {
    pub label_id: String,
    pub label_name: String,
    pub score: f32,
    pub weight: f32,
    pub already_applied: bool,
    pub evidence_atoms: Vec<LabelSuggestionEvidenceRecord>,
    pub negative_evidence_atoms: Vec<LabelSuggestionEvidenceRecord>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LabelSuggestionEvidenceRecord {
    pub atom_id: String,
    pub label_id: String,
    pub label_name: String,
    pub polarity: String,
    pub kind: String,
    pub text: String,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LabelSemanticProposalRecord {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub status: String,
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

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LabelProposalAttemptRecord {
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

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LabelOntologyObservationRecord {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub task_ref_snapshot: String,
    pub task_snapshot_json: String,
    pub suggest_input_hash: Option<String>,
    pub agent_candidates_json: String,
    pub suggestion_snapshot_json: String,
    pub final_decision_json: String,
    pub suggest_coverage: Option<f64>,
    pub suggest_coverage_cosine: Option<f64>,
    pub suggest_residual_norm: Option<f64>,
    pub suggest_needs_new_label: bool,
    pub suggest_degraded: bool,
    pub diagnostics_json: String,
    pub capture_fingerprint: String,
    pub created_by: String,
    pub created_by_type: String,
    pub agent_type: Option<String>,
    pub created_at: i64,
    pub signals: Vec<LabelOntologySignalRecord>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LabelOntologySignalRecord {
    pub id: String,
    pub observation_id: String,
    pub board_id: String,
    pub kind: String,
    pub status: String,
    pub target_label_id: Option<String>,
    pub target_label_name_snapshot: Option<String>,
    pub related_labels_json: String,
    pub proposed_action: String,
    pub candidate_atom_polarity: Option<String>,
    pub candidate_atom_kind: Option<String>,
    pub candidate_text: Option<String>,
    pub candidate_content_hash: Option<String>,
    pub proposed_label_name: Option<String>,
    pub proposed_label_name_normalized: Option<String>,
    pub proposal_json: String,
    pub agent_selected: bool,
    pub suggest_state: Option<String>,
    pub suggest_score: Option<f64>,
    pub suggest_rank: Option<i64>,
    pub final_selected: bool,
    pub rationale: String,
    pub confidence: Option<f64>,
    pub signal_key: String,
    pub superseded_by_signal_id: Option<String>,
    pub status_reason: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub reviewed_at: Option<i64>,
    pub closed_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LabelOntologyActionRecord {
    pub id: String,
    pub board_id: String,
    pub parent_action_id: Option<String>,
    pub action_type: String,
    pub reason: String,
    pub target_label_id: Option<String>,
    pub result_label_id: Option<String>,
    pub result_atom_id: Option<String>,
    pub result_atom_content_hash: Option<String>,
    pub result_proposal_id: Option<String>,
    pub canonical_before_hash: Option<String>,
    pub canonical_after_hash: Option<String>,
    pub change_json: String,
    pub validation_requirement: String,
    pub validation_status: String,
    pub validation_json: String,
    pub created_by: String,
    pub created_by_type: String,
    pub agent_type: Option<String>,
    pub created_at: i64,
    pub signal_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LabelOntologySignalDetailRecord {
    pub signal: LabelOntologySignalRecord,
    pub observation: LabelOntologyObservationRecord,
    pub actions: Vec<LabelOntologyActionRecord>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LabelOntologyReviewGroupRecord {
    pub group_by: String,
    pub key: String,
    pub label_id: Option<String>,
    pub label_name: Option<String>,
    pub candidate_atom_polarity: Option<String>,
    pub candidate_atom_kind: Option<String>,
    pub candidate_text: Option<String>,
    pub candidate_content_hash: Option<String>,
    pub proposed_label_name: Option<String>,
    pub proposed_label_name_normalized: Option<String>,
    pub cluster_key: Option<String>,
    pub cluster_reason: Option<String>,
    pub task_count: i64,
    pub signal_count: i64,
    pub open_count: i64,
    pub confirmed_count: i64,
    pub resolved_count: i64,
    pub rejected_count: i64,
    pub superseded_count: i64,
    pub degraded_count: i64,
    pub average_score: Option<f64>,
    pub median_score: Option<f64>,
    pub oldest_signal_at: i64,
    pub latest_signal_at: i64,
    pub sample_task_refs: Vec<String>,
    pub signal_ids: Vec<String>,
    pub action_count: i64,
    pub action_ids: Vec<String>,
    pub proposal_ids: Vec<String>,
    pub labels_json: String,
    pub candidate_atom_variants_json: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LabelOntologyQualityRecord {
    pub board_id: String,
    pub denominator_json: String,
    pub disagreement_json: String,
    pub rates_json: String,
    pub precision_recall_json: String,
    pub warnings_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphStatusRecord {
    pub backend: String,
    pub enabled: bool,
    pub message: String,
    pub projection: ProjectionStateRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphQueryBindingRecord {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphQueryRowRecord {
    pub bindings: Vec<GraphQueryBindingRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskGraphNodeRecord {
    pub task: TaskRecord,
    pub role: String,
    pub context_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskGraphEdgeRecord {
    pub id: String,
    pub source_task_id: String,
    pub target_task_id: String,
    pub kind: String,
    pub required: bool,
    pub blocking: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskGraphMetaRecord {
    pub depth: usize,
    pub context_depth: usize,
    pub generated_at: i64,
    pub node_count: usize,
    pub edge_count: usize,
    pub truncated: bool,
    pub active_statuses: Vec<String>,
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
