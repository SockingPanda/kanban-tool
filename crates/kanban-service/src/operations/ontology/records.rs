//! Ontology 的 application record。已知响应结构在 service 边界展开，真实动态字段保留 JSON。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{LabelAtomRecord, LabelSemanticsRecord};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomExplainActionRecord {
    pub action: LabelOntologyActionRecord,
    pub matched_by: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomExplainSignalRecord {
    pub signal: LabelOntologySignalRecord,
    pub observation: LabelOntologyObservationRecord,
    pub task_id: String,
    pub task_ref_snapshot: String,
    pub suggest_input_stale: bool,
    pub suggest_degraded: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomExplainValidationRecord {
    pub action: LabelOntologyActionRecord,
    pub parent_action_id: String,
    pub validation_status: String,
    pub manual: Value,
    pub summary: Value,
    pub cases: Value,
    pub warnings: Vec<String>,
    pub validation_effective_outcome: String,
    pub validation_latest_attempt_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomIndexStatusRecord {
    pub backend: String,
    pub enabled: bool,
    pub message: String,
    pub diagnostics: Vec<String>,
    pub dirty: Option<bool>,
    pub board_dirty: Option<bool>,
    pub generation: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelSuggestionCandidateRecord {
    pub label_id: String,
    pub label_name: String,
    pub score: f32,
    pub weight: f32,
    pub already_applied: bool,
    pub evidence_atoms: Vec<LabelSuggestionEvidenceRecord>,
    pub negative_evidence_atoms: Vec<LabelSuggestionEvidenceRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelSuggestionEvidenceRecord {
    pub atom_id: String,
    pub label_id: String,
    pub label_name: String,
    pub polarity: String,
    pub kind: String,
    pub text: String,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologyObservationRecord {
    pub id: String,
    pub board_id: String,
    pub task_id: String,
    pub task_ref_snapshot: String,
    pub task_snapshot: Value,
    pub suggest_input_hash: Option<String>,
    pub agent_candidates: Value,
    pub suggestion_snapshot: Value,
    pub final_decision: Value,
    pub suggest_coverage: Option<f64>,
    pub suggest_coverage_cosine: Option<f64>,
    pub suggest_residual_norm: Option<f64>,
    pub suggest_needs_new_label: bool,
    pub suggest_degraded: bool,
    pub diagnostics: Value,
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
    pub kind: String,
    pub status: String,
    pub target_label_id: Option<String>,
    pub target_label_name_snapshot: Option<String>,
    pub related_labels: Value,
    pub proposed_action: String,
    pub candidate_atom_polarity: Option<String>,
    pub candidate_atom_kind: Option<String>,
    pub candidate_text: Option<String>,
    pub candidate_content_hash: Option<String>,
    pub proposed_label_name: Option<String>,
    pub proposed_label_name_normalized: Option<String>,
    pub proposal: Value,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub change: Value,
    pub validation_requirement: String,
    pub validation_status: String,
    pub validation: Value,
    pub created_by: String,
    pub created_by_type: String,
    pub agent_type: Option<String>,
    pub created_at: i64,
    pub signal_ids: Vec<String>,
    pub validation_effective_outcome: String,
    pub validation_latest_attempt_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologySignalDetailRecord {
    pub signal: LabelOntologySignalRecord,
    pub observation: LabelOntologyObservationRecord,
    pub actions: Vec<LabelOntologyActionRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub labels: Vec<LabelOntologyReviewLabelRefRecord>,
    pub candidate_atom_variants: Vec<LabelOntologyReviewAtomVariantRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelOntologyQualityRecord {
    pub board_id: String,
    pub denominator: LabelOntologyQualityDenominator,
    pub disagreement: LabelOntologyQualityDisagreement,
    pub rates: LabelOntologyQualityRates,
    pub precision_recall: LabelOntologyPrecisionRecall,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelOntologyReviewLabelRefRecord {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelOntologyReviewAtomVariantRecord {
    pub content_hash: String,
    pub polarity: Option<String>,
    pub kind: Option<String>,
    pub text: Option<String>,
    pub signal_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
pub struct LabelOntologyPrecisionRecall {
    pub available: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomIndexQueryRecord {
    pub data: Vec<LabelAtomIndexHitRecord>,
    pub degraded: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomIndexHitRecord {
    pub atom_id: String,
    pub label_id: String,
    pub label_name: String,
    pub board_id: String,
    pub polarity: String,
    pub kind: String,
    pub text: String,
    pub ordinal: i64,
    pub content_hash: String,
    pub embedding_model: String,
    pub distance: f64,
}
