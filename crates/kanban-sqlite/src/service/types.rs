use std::{collections::BTreeMap, fs, path::PathBuf};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskOntologySummary {
    pub task_id: String,
    pub observation_count: i64,
    pub signal_count: i64,
    pub open_count: i64,
    pub confirmed_count: i64,
    pub resolved_count: i64,
    pub rejected_count: i64,
    pub superseded_count: i64,
    pub degraded_count: i64,
    pub stale_count: i64,
    pub suggest_input_drift_count: i64,
    pub legacy_incomparable_count: i64,
    pub incomparable_count: i64,
    pub action_count: i64,
    pub oldest_open_confirmed_signal_at: Option<i64>,
    pub oldest_open_confirmed_signal_age_ms: Option<i64>,
    pub latest_signal_at: Option<i64>,
    pub latest_action_at: Option<i64>,
    pub current_suggest_input_hash: String,
    pub sample_signals: Vec<TaskOntologySignalSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskOntologySignalSummary {
    pub id: String,
    pub kind: LabelOntologySignalKind,
    pub status: LabelOntologySignalStatus,
    pub proposed_action: LabelOntologyProposedAction,
    pub target_label_id: Option<String>,
    pub target_label_name: Option<String>,
    pub candidate_atom_polarity: Option<String>,
    pub candidate_atom_kind: Option<String>,
    pub candidate_text: Option<String>,
    pub candidate_content_hash: Option<String>,
    pub proposed_label_name: Option<String>,
    pub proposed_label_name_normalized: Option<String>,
    pub suggest_score: Option<f64>,
    pub suggest_rank: Option<i64>,
    pub degraded: bool,
    pub stale: bool,
    pub legacy_incomparable: bool,
    pub suggest_input_drift: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub latest_action_at: Option<i64>,
    pub action_count: i64,
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
pub struct AddTaskLabelsResult {
    pub task: TaskRecord,
    pub created_labels: Vec<LabelRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteLabelResult {
    pub label: LabelRecord,
    pub forced: bool,
    pub removed_task_bindings: i64,
    pub removed_semantics: bool,
    pub removed_atoms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BootstrapTaskLabelVerification {
    pub label_name: String,
    pub score: f32,
    pub source: String,
    pub min_score: f32,
    pub degraded: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BootstrapTaskLabelVerifiedResult {
    pub task: TaskRecord,
    pub semantics: LabelSemanticsRecord,
    pub verification: BootstrapTaskLabelVerification,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomExplainRecord {
    pub query: String,
    pub atom: Option<LabelAtomRecord>,
    pub current_semantics: Option<LabelSemanticsRecord>,
    pub provenance_actions: Vec<LabelAtomExplainAction>,
    pub supporting_signals: Vec<LabelAtomExplainSignal>,
    pub validation_history: Vec<LabelAtomExplainValidation>,
    pub legacy_untracked: bool,
    pub legacy_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomExplainAction {
    pub action: LabelOntologyActionRecord,
    pub matched_by: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomExplainSignal {
    pub signal: LabelOntologySignalRecord,
    pub observation: LabelOntologyObservationRecord,
    pub source_task: TaskRecord,
    pub task_ref_snapshot: String,
    pub suggest_input_stale: bool,
    pub suggest_degraded: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomExplainValidation {
    pub action: LabelOntologyActionRecord,
    pub parent_action_id: String,
    pub validation_status: LabelOntologyValidationStatus,
    pub manual: serde_json::Value,
    pub summary: serde_json::Value,
    pub cases: serde_json::Value,
    pub warnings: Vec<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LabelProposalCreateOptions {
    pub source_signal_ids: Vec<String>,
    pub ontology_actor: Option<LabelOntologyActor>,
    pub allow_retarget: bool,
    pub retarget_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LabelProposalProposeOptions {
    pub suggestion: LabelSuggestionOptions,
    pub create: LabelProposalCreateOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LabelProposalDecisionOptions {
    pub source_signal_ids: Vec<String>,
    pub ontology_actor: Option<LabelOntologyActor>,
    pub allow_retarget: bool,
    pub retarget_reason: Option<String>,
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
    pub expected_semantics_hash: Option<String>,
    pub replace: bool,
    pub description: Option<String>,
    pub applies_when: Vec<String>,
    pub excludes_when: Vec<String>,
    pub positive_examples: Vec<String>,
    pub negative_examples: Vec<String>,
    pub remove_applies_when: Vec<String>,
    pub remove_excludes_when: Vec<String>,
    pub remove_positive_examples: Vec<String>,
    pub remove_negative_examples: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelSemanticsMutationOptions {
    pub actor: LabelOntologyActor,
    pub reason: Option<String>,
    pub source_signal_ids: Vec<String>,
    pub context_json: Option<String>,
}

impl LabelSemanticsMutationOptions {
    pub fn manual_actor(actor: impl Into<String>) -> Self {
        Self {
            actor: LabelOntologyActor {
                name: actor.into(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            reason: None,
            source_signal_ids: Vec::new(),
            context_json: None,
        }
    }
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
    pub reason_codes: Vec<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelOntologySignalKind {
    FalseNegative,
    FalsePositive,
    VocabularyGap,
    NameIssue,
    BoundaryIssue,
    StructureIssue,
}

impl std::fmt::Display for LabelOntologySignalKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::FalseNegative => "false_negative",
            Self::FalsePositive => "false_positive",
            Self::VocabularyGap => "vocabulary_gap",
            Self::NameIssue => "name_issue",
            Self::BoundaryIssue => "boundary_issue",
            Self::StructureIssue => "structure_issue",
        })
    }
}

impl std::str::FromStr for LabelOntologySignalKind {
    type Err = kanban_core::KanbanError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "false_negative" => Ok(Self::FalseNegative),
            "false_positive" => Ok(Self::FalsePositive),
            "vocabulary_gap" => Ok(Self::VocabularyGap),
            "name_issue" => Ok(Self::NameIssue),
            "boundary_issue" => Ok(Self::BoundaryIssue),
            "structure_issue" => Ok(Self::StructureIssue),
            _ => Err(kanban_core::KanbanError::InvalidInput(format!(
                "invalid label ontology signal kind: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelOntologySignalStatus {
    Open,
    Confirmed,
    Resolved,
    Rejected,
    Superseded,
}

impl std::fmt::Display for LabelOntologySignalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Open => "open",
            Self::Confirmed => "confirmed",
            Self::Resolved => "resolved",
            Self::Rejected => "rejected",
            Self::Superseded => "superseded",
        })
    }
}

impl std::str::FromStr for LabelOntologySignalStatus {
    type Err = kanban_core::KanbanError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "open" => Ok(Self::Open),
            "confirmed" => Ok(Self::Confirmed),
            "resolved" => Ok(Self::Resolved),
            "rejected" => Ok(Self::Rejected),
            "superseded" => Ok(Self::Superseded),
            _ => Err(kanban_core::KanbanError::InvalidInput(format!(
                "invalid label ontology signal status: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelOntologyProposedAction {
    Observe,
    AddPositiveAtom,
    AddNegativeAtom,
    UpdateSemantics,
    BootstrapLabel,
    RenameLabel,
    SplitLabel,
    MergeLabels,
}

impl std::fmt::Display for LabelOntologyProposedAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Observe => "observe",
            Self::AddPositiveAtom => "add_positive_atom",
            Self::AddNegativeAtom => "add_negative_atom",
            Self::UpdateSemantics => "update_semantics",
            Self::BootstrapLabel => "bootstrap_label",
            Self::RenameLabel => "rename_label",
            Self::SplitLabel => "split_label",
            Self::MergeLabels => "merge_labels",
        })
    }
}

impl std::str::FromStr for LabelOntologyProposedAction {
    type Err = kanban_core::KanbanError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "observe" => Ok(Self::Observe),
            "add_positive_atom" => Ok(Self::AddPositiveAtom),
            "add_negative_atom" => Ok(Self::AddNegativeAtom),
            "update_semantics" => Ok(Self::UpdateSemantics),
            "bootstrap_label" => Ok(Self::BootstrapLabel),
            "rename_label" => Ok(Self::RenameLabel),
            "split_label" => Ok(Self::SplitLabel),
            "merge_labels" => Ok(Self::MergeLabels),
            _ => Err(kanban_core::KanbanError::InvalidInput(format!(
                "invalid label ontology proposed action: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelOntologySuggestState {
    Selected,
    Candidate,
    Absent,
    Unavailable,
}

impl std::fmt::Display for LabelOntologySuggestState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Selected => "selected",
            Self::Candidate => "candidate",
            Self::Absent => "absent",
            Self::Unavailable => "unavailable",
        })
    }
}

impl std::str::FromStr for LabelOntologySuggestState {
    type Err = kanban_core::KanbanError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "selected" => Ok(Self::Selected),
            "candidate" => Ok(Self::Candidate),
            "absent" => Ok(Self::Absent),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err(kanban_core::KanbanError::InvalidInput(format!(
                "invalid label ontology suggest state: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelOntologyActionType {
    Confirm,
    Reject,
    Supersede,
    ResolveNoChange,
    AddPositiveAtom,
    AddNegativeAtom,
    AdoptExistingAtom,
    UpdateSemantics,
    CreateLabelProposal,
    BootstrapLabel,
    RenameLabel,
    SplitLabel,
    MergeLabels,
    RevertOntologyMutation,
    Validate,
}

impl std::fmt::Display for LabelOntologyActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Confirm => "confirm",
            Self::Reject => "reject",
            Self::Supersede => "supersede",
            Self::ResolveNoChange => "resolve_no_change",
            Self::AddPositiveAtom => "add_positive_atom",
            Self::AddNegativeAtom => "add_negative_atom",
            Self::AdoptExistingAtom => "adopt_existing_atom",
            Self::UpdateSemantics => "update_semantics",
            Self::CreateLabelProposal => "create_label_proposal",
            Self::BootstrapLabel => "bootstrap_label",
            Self::RenameLabel => "rename_label",
            Self::SplitLabel => "split_label",
            Self::MergeLabels => "merge_labels",
            Self::RevertOntologyMutation => "revert_ontology_mutation",
            Self::Validate => "validate",
        })
    }
}

impl std::str::FromStr for LabelOntologyActionType {
    type Err = kanban_core::KanbanError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "confirm" => Ok(Self::Confirm),
            "reject" => Ok(Self::Reject),
            "supersede" => Ok(Self::Supersede),
            "resolve_no_change" => Ok(Self::ResolveNoChange),
            "add_positive_atom" => Ok(Self::AddPositiveAtom),
            "add_negative_atom" => Ok(Self::AddNegativeAtom),
            "adopt_existing_atom" => Ok(Self::AdoptExistingAtom),
            "update_semantics" => Ok(Self::UpdateSemantics),
            "create_label_proposal" => Ok(Self::CreateLabelProposal),
            "bootstrap_label" => Ok(Self::BootstrapLabel),
            "rename_label" => Ok(Self::RenameLabel),
            "split_label" => Ok(Self::SplitLabel),
            "merge_labels" => Ok(Self::MergeLabels),
            "revert_ontology_mutation" => Ok(Self::RevertOntologyMutation),
            "validate" => Ok(Self::Validate),
            _ => Err(kanban_core::KanbanError::InvalidInput(format!(
                "invalid label ontology action type: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelOntologyValidationStatus {
    NotRequired,
    Pending,
    Passed,
    Failed,
    Partial,
}

impl std::fmt::Display for LabelOntologyValidationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NotRequired => "not_required",
            Self::Pending => "pending",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Partial => "partial",
        })
    }
}

impl std::str::FromStr for LabelOntologyValidationStatus {
    type Err = kanban_core::KanbanError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "not_required" => Ok(Self::NotRequired),
            "pending" => Ok(Self::Pending),
            "passed" => Ok(Self::Passed),
            "failed" => Ok(Self::Failed),
            "partial" => Ok(Self::Partial),
            _ => Err(kanban_core::KanbanError::InvalidInput(format!(
                "invalid label ontology validation status: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelOntologyValidationRequirement {
    None,
    Required,
    Unsupported,
}

impl std::fmt::Display for LabelOntologyValidationRequirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::None => "none",
            Self::Required => "required",
            Self::Unsupported => "unsupported",
        })
    }
}

impl std::str::FromStr for LabelOntologyValidationRequirement {
    type Err = kanban_core::KanbanError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "none" => Ok(Self::None),
            "required" => Ok(Self::Required),
            "unsupported" => Ok(Self::Unsupported),
            _ => Err(kanban_core::KanbanError::InvalidInput(format!(
                "invalid label ontology validation requirement: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelOntologyValidationEffectiveOutcome {
    NotRequired,
    Unsupported,
    Pending,
    Passed,
    Failed,
    Partial,
}

impl std::fmt::Display for LabelOntologyValidationEffectiveOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NotRequired => "not_required",
            Self::Unsupported => "unsupported",
            Self::Pending => "pending",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Partial => "partial",
        })
    }
}

impl std::str::FromStr for LabelOntologyValidationEffectiveOutcome {
    type Err = kanban_core::KanbanError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "not_required" => Ok(Self::NotRequired),
            "unsupported" => Ok(Self::Unsupported),
            "pending" => Ok(Self::Pending),
            "passed" => Ok(Self::Passed),
            "failed" => Ok(Self::Failed),
            "partial" => Ok(Self::Partial),
            _ => Err(kanban_core::KanbanError::InvalidInput(format!(
                "invalid label ontology validation effective outcome: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelOntologyActor {
    pub name: String,
    #[serde(rename = "type")]
    pub actor_type: String,
    pub agent_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelOntologyCandidateAtomInput {
    pub polarity: String,
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologySignalInput {
    pub kind: LabelOntologySignalKind,
    pub target_label_ref: Option<String>,
    pub related_labels_json: String,
    pub proposed_action: LabelOntologyProposedAction,
    pub candidate_atom: Option<LabelOntologyCandidateAtomInput>,
    pub proposed_label_name: Option<String>,
    pub proposal_json: String,
    pub agent_selected: bool,
    pub suggest_state: Option<LabelOntologySuggestState>,
    pub suggest_score: Option<f64>,
    pub suggest_rank: Option<i64>,
    pub final_selected: bool,
    pub rationale: String,
    pub confidence: Option<f64>,
    pub signal_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologyRecordInput {
    pub actor: LabelOntologyActor,
    pub agent_candidates_json: String,
    pub suggestion_snapshot_json: String,
    pub final_decision_json: String,
    pub suggest_coverage: Option<f64>,
    pub suggest_coverage_cosine: Option<f64>,
    pub suggest_residual_norm: Option<f64>,
    pub suggest_needs_new_label: bool,
    pub suggest_degraded: bool,
    pub diagnostics_json: String,
    pub capture_fingerprint: Option<String>,
    pub signals: Vec<LabelOntologySignalInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologySignalRecord {
    pub id: String,
    pub observation_id: String,
    pub board_id: String,
    pub kind: LabelOntologySignalKind,
    pub status: LabelOntologySignalStatus,
    pub target_label_id: Option<String>,
    pub target_label_name_snapshot: Option<String>,
    pub related_labels_json: String,
    pub proposed_action: LabelOntologyProposedAction,
    pub candidate_atom_polarity: Option<String>,
    pub candidate_atom_kind: Option<String>,
    pub candidate_text: Option<String>,
    pub candidate_content_hash: Option<String>,
    pub proposed_label_name: Option<String>,
    pub proposed_label_name_normalized: Option<String>,
    pub proposal_json: String,
    pub agent_selected: bool,
    pub suggest_state: Option<LabelOntologySuggestState>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologyActionRecord {
    pub id: String,
    pub board_id: String,
    pub parent_action_id: Option<String>,
    pub action_type: LabelOntologyActionType,
    pub reason: String,
    pub target_label_id: Option<String>,
    pub result_label_id: Option<String>,
    pub result_atom_id: Option<String>,
    pub result_atom_content_hash: Option<String>,
    pub result_proposal_id: Option<String>,
    pub canonical_before_hash: Option<String>,
    pub canonical_after_hash: Option<String>,
    pub change_json: String,
    pub validation_requirement: LabelOntologyValidationRequirement,
    pub validation_status: LabelOntologyValidationStatus,
    pub validation_effective_outcome: LabelOntologyValidationEffectiveOutcome,
    pub validation_latest_attempt_id: Option<String>,
    pub validation_json: String,
    pub created_by: String,
    pub created_by_type: String,
    pub agent_type: Option<String>,
    pub created_at: i64,
    pub signal_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologyActionAtomEffectRecord {
    pub board_id: String,
    pub action_id: String,
    pub label_id_snapshot: String,
    pub atom_id_snapshot: String,
    pub atom_content_hash: String,
    pub polarity: String,
    pub kind: String,
    pub text: String,
    pub effect: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologyActionInput {
    pub actor: LabelOntologyActor,
    pub action_type: LabelOntologyActionType,
    pub signal_ids: Vec<String>,
    pub reason: String,
    pub superseded_by_signal_id: Option<String>,
    pub parent_action_id: Option<String>,
    pub target_label_ref: Option<String>,
    pub result_label_ref: Option<String>,
    pub result_atom_id: Option<String>,
    pub result_atom_content_hash: Option<String>,
    pub result_proposal_id: Option<String>,
    pub canonical_before_hash: Option<String>,
    pub canonical_after_hash: Option<String>,
    pub change_json: Option<String>,
    pub validation_status: Option<LabelOntologyValidationStatus>,
    pub validation_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologyAtomApplyInput {
    pub actor: LabelOntologyActor,
    pub signal_ids: Vec<String>,
    pub label_ref: String,
    pub kind: String,
    pub text: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologyRevertInput {
    pub actor: LabelOntologyActor,
    pub target_action_id: String,
    pub expected_current_hash: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LabelOntologyRetargetOptions {
    pub allow_retarget: bool,
    pub retarget_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologyValidationInput {
    pub actor: LabelOntologyActor,
    pub parent_action_id: String,
    pub signal_ids: Vec<String>,
    pub reason: String,
    pub validation_status: LabelOntologyValidationStatus,
    pub validation_json: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologyTrustedValidationInput {
    pub actor: LabelOntologyActor,
    pub parent_action_id: String,
    pub signal_ids: Vec<String>,
    pub reason: String,
    pub validation_status: LabelOntologyValidationStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologySignalDetail {
    pub signal: LabelOntologySignalRecord,
    pub observation: LabelOntologyObservationRecord,
    pub actions: Vec<LabelOntologyActionRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelOntologyReviewGroupBy {
    Label,
    CandidateAtom,
    ProposedLabel,
    Cluster,
}

impl std::fmt::Display for LabelOntologyReviewGroupBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Label => "label",
            Self::CandidateAtom => "candidate_atom",
            Self::ProposedLabel => "proposed_label",
            Self::Cluster => "cluster",
        })
    }
}

impl std::str::FromStr for LabelOntologyReviewGroupBy {
    type Err = kanban_core::KanbanError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "label" => Ok(Self::Label),
            "candidate_atom" | "candidate-atom" => Ok(Self::CandidateAtom),
            "proposed_label" | "proposed-label" => Ok(Self::ProposedLabel),
            "cluster" => Ok(Self::Cluster),
            _ => Err(kanban_core::KanbanError::InvalidInput(format!(
                "invalid label ontology review group: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelOntologyReviewOptions {
    pub group_by: LabelOntologyReviewGroupBy,
    pub include_all: bool,
    pub limit: usize,
}

impl Default for LabelOntologyReviewOptions {
    fn default() -> Self {
        Self {
            group_by: LabelOntologyReviewGroupBy::Label,
            include_all: false,
            limit: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelOntologyReviewLabelRef {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelOntologyReviewAtomVariant {
    pub content_hash: String,
    pub polarity: Option<String>,
    pub kind: Option<String>,
    pub text: Option<String>,
    pub signal_count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologyReviewGroup {
    pub group_by: LabelOntologyReviewGroupBy,
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
    pub labels: Vec<LabelOntologyReviewLabelRef>,
    pub candidate_atom_variants: Vec<LabelOntologyReviewAtomVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelOntologyQualityOptions {
    pub sample_limit: usize,
}

impl Default for LabelOntologyQualityOptions {
    fn default() -> Self {
        Self { sample_limit: 20 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologyQualityReport {
    pub board_id: String,
    pub denominator: LabelOntologyQualityDenominator,
    pub disagreement: LabelOntologyQualityDisagreement,
    pub rates: LabelOntologyQualityRates,
    pub precision_recall: LabelOntologyPrecisionRecallAvailability,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologyQualityDenominator {
    pub source: String,
    pub description: String,
    pub observation_count: i64,
    pub distinct_task_count: i64,
    pub agreement_observation_count: i64,
    pub agreement_task_count: i64,
    pub degraded_observation_count: i64,
    pub first_observed_at: Option<i64>,
    pub latest_observed_at: Option<i64>,
    pub sample_task_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologyQualityDisagreement {
    pub signal_count: i64,
    pub distinct_task_count: i64,
    pub by_kind: BTreeMap<String, i64>,
    pub by_status: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologyQualityRates {
    pub disagreement_task_rate: Option<f64>,
    pub disagreement_task_rate_basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelOntologyPrecisionRecallAvailability {
    pub available: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelOntologySignalListOptions {
    pub statuses: Vec<LabelOntologySignalStatus>,
    pub kinds: Vec<LabelOntologySignalKind>,
    pub task_ref: Option<String>,
    pub target_label_ref: Option<String>,
    pub proposed_label_name: Option<String>,
    pub include_all: bool,
    pub limit: usize,
}

impl Default for LabelOntologySignalListOptions {
    fn default() -> Self {
        Self {
            statuses: Vec::new(),
            kinds: Vec::new(),
            task_ref: None,
            target_label_ref: None,
            proposed_label_name: None,
            include_all: false,
            limit: 100,
        }
    }
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
