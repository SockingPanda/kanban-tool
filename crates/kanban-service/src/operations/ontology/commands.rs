//! Ontology 的 application command。动态 JSON 使用自然值，store 编码留在 service 内。

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct UpsertLabelSemanticsCommand {
    pub expected_semantics_hash: Option<String>,
    pub replace: bool,
    pub description: Option<String>,
    pub applies_when: Option<Vec<String>>,
    pub excludes_when: Option<Vec<String>>,
    pub positive_examples: Option<Vec<String>>,
    pub negative_examples: Option<Vec<String>>,
    pub remove_applies_when: Vec<String>,
    pub remove_excludes_when: Vec<String>,
    pub remove_positive_examples: Vec<String>,
    pub remove_negative_examples: Vec<String>,
    pub actor: String,
    pub reason: Option<String>,
    pub source_signal_ids: Vec<String>,
}

impl From<UpsertLabelSemanticsCommand> for crate::store_operations::UpsertLabelSemanticsInput {
    fn from(value: UpsertLabelSemanticsCommand) -> Self {
        Self {
            expected_semantics_hash: value.expected_semantics_hash,
            replace: value.replace,
            description: value.description,
            applies_when: value.applies_when,
            excludes_when: value.excludes_when,
            positive_examples: value.positive_examples,
            negative_examples: value.negative_examples,
            remove_applies_when: value.remove_applies_when,
            remove_excludes_when: value.remove_excludes_when,
            remove_positive_examples: value.remove_positive_examples,
            remove_negative_examples: value.remove_negative_examples,
            actor: value.actor,
            reason: value.reason,
            source_signal_ids: value.source_signal_ids,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
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
            output_limit: 5,
            candidate_limit: 32,
            atom_limit: 80,
            max_selected_labels: 4,
            min_score: 0.15,
        }
    }
}

impl From<LabelSuggestionOptions> for crate::store_operations::LabelSuggestionOptions {
    fn from(value: LabelSuggestionOptions) -> Self {
        Self {
            output_limit: value.output_limit,
            candidate_limit: value.candidate_limit,
            atom_limit: value.atom_limit,
            max_selected_labels: value.max_selected_labels,
            min_score: value.min_score,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LabelProposalCommand {
    pub name: Option<String>,
    pub description: Option<String>,
    pub applies_when: Vec<String>,
    pub excludes_when: Vec<String>,
    pub positive_examples: Vec<String>,
    pub negative_examples: Vec<String>,
    pub actor: String,
    pub source_signal_ids: Vec<String>,
}

impl From<LabelProposalCommand> for crate::store_operations::LabelProposalInput {
    fn from(value: LabelProposalCommand) -> Self {
        Self {
            name: value.name,
            description: value.description,
            applies_when: value.applies_when,
            excludes_when: value.excludes_when,
            positive_examples: value.positive_examples,
            negative_examples: value.negative_examples,
            actor: value.actor,
            source_signal_ids: value.source_signal_ids,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LabelProposalDecisionCommand {
    pub proposal_id: String,
    pub accept: bool,
    pub reason: Option<String>,
    pub actor: String,
    pub source_signal_ids: Vec<String>,
}

impl From<LabelProposalDecisionCommand> for crate::store_operations::LabelProposalDecisionInput {
    fn from(value: LabelProposalDecisionCommand) -> Self {
        Self {
            proposal_id: value.proposal_id,
            accept: value.accept,
            reason: value.reason,
            actor: value.actor,
            source_signal_ids: value.source_signal_ids,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OntologyActorCommand {
    pub name: String,
    pub actor_type: String,
    pub agent_type: Option<String>,
}

impl From<OntologyActorCommand> for crate::store_operations::OntologyActorInput {
    fn from(value: OntologyActorCommand) -> Self {
        Self {
            name: value.name,
            actor_type: value.actor_type,
            agent_type: value.agent_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OntologySignalCommand {
    pub kind: String,
    pub target_label_ref: Option<String>,
    pub related_labels: Value,
    pub proposed_action: String,
    pub candidate_atom_polarity: Option<String>,
    pub candidate_atom_kind: Option<String>,
    pub candidate_text: Option<String>,
    pub proposed_label_name: Option<String>,
    pub proposal: Value,
    pub agent_selected: bool,
    pub suggest_state: Option<String>,
    pub suggest_score: Option<f64>,
    pub suggest_rank: Option<i64>,
    pub final_selected: bool,
    pub rationale: String,
    pub confidence: Option<f64>,
    pub signal_key: Option<String>,
}

impl From<OntologySignalCommand> for crate::store_operations::OntologySignalInput {
    fn from(value: OntologySignalCommand) -> Self {
        Self {
            kind: value.kind,
            target_label_ref: value.target_label_ref,
            related_labels_json: value.related_labels.to_string(),
            proposed_action: value.proposed_action,
            candidate_atom_polarity: value.candidate_atom_polarity,
            candidate_atom_kind: value.candidate_atom_kind,
            candidate_text: value.candidate_text,
            proposed_label_name: value.proposed_label_name,
            proposal_json: value.proposal.to_string(),
            agent_selected: value.agent_selected,
            suggest_state: value.suggest_state,
            suggest_score: value.suggest_score,
            suggest_rank: value.suggest_rank,
            final_selected: value.final_selected,
            rationale: value.rationale,
            confidence: value.confidence,
            signal_key: value.signal_key,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OntologyObservationCommand {
    pub actor: OntologyActorCommand,
    pub task_ref: String,
    pub agent_candidates: Value,
    pub suggestion_snapshot: Value,
    pub final_decision: Value,
    pub suggest_coverage: Option<f64>,
    pub suggest_coverage_cosine: Option<f64>,
    pub suggest_residual_norm: Option<f64>,
    pub suggest_needs_new_label: bool,
    pub suggest_degraded: bool,
    pub diagnostics: Value,
    pub capture_fingerprint: Option<String>,
    pub signals: Vec<OntologySignalCommand>,
}

impl From<OntologyObservationCommand> for crate::store_operations::OntologyObservationInput {
    fn from(value: OntologyObservationCommand) -> Self {
        Self {
            actor: value.actor.into(),
            task_ref: value.task_ref,
            agent_candidates_json: value.agent_candidates.to_string(),
            suggestion_snapshot_json: value.suggestion_snapshot.to_string(),
            final_decision_json: value.final_decision.to_string(),
            suggest_coverage: value.suggest_coverage,
            suggest_coverage_cosine: value.suggest_coverage_cosine,
            suggest_residual_norm: value.suggest_residual_norm,
            suggest_needs_new_label: value.suggest_needs_new_label,
            suggest_degraded: value.suggest_degraded,
            diagnostics_json: value.diagnostics.to_string(),
            capture_fingerprint: value.capture_fingerprint,
            signals: value.signals.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OntologyActionCommand {
    pub actor: OntologyActorCommand,
    pub idempotency_key: Option<String>,
    pub action_type: String,
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
    pub change: Value,
    pub validation_status: Option<String>,
    pub validation: Value,
}

impl From<OntologyActionCommand> for crate::store_operations::OntologyActionInput {
    fn from(value: OntologyActionCommand) -> Self {
        Self {
            actor: value.actor.into(),
            idempotency_key: value.idempotency_key,
            action_type: value.action_type,
            signal_ids: value.signal_ids,
            reason: value.reason,
            superseded_by_signal_id: value.superseded_by_signal_id,
            parent_action_id: value.parent_action_id,
            target_label_ref: value.target_label_ref,
            result_label_ref: value.result_label_ref,
            result_atom_id: value.result_atom_id,
            result_atom_content_hash: value.result_atom_content_hash,
            result_proposal_id: value.result_proposal_id,
            canonical_before_hash: value.canonical_before_hash,
            canonical_after_hash: value.canonical_after_hash,
            change_json: value.change.to_string(),
            validation_status: value.validation_status,
            validation_json: value.validation.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OntologyApplyAtomCommand {
    pub actor: OntologyActorCommand,
    pub signal_ids: Vec<String>,
    pub label_ref: String,
    pub polarity: String,
    pub kind: String,
    pub text: String,
    pub reason: String,
}

impl From<OntologyApplyAtomCommand> for crate::store_operations::OntologyApplyAtomInput {
    fn from(value: OntologyApplyAtomCommand) -> Self {
        Self {
            actor: value.actor.into(),
            signal_ids: value.signal_ids,
            label_ref: value.label_ref,
            polarity: value.polarity,
            kind: value.kind,
            text: value.text,
            reason: value.reason,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OntologyRevertCommand {
    pub actor: OntologyActorCommand,
    pub target_action_id: String,
    pub expected_current_hash: Option<String>,
    pub reason: String,
}

impl From<OntologyRevertCommand> for crate::store_operations::OntologyRevertInput {
    fn from(value: OntologyRevertCommand) -> Self {
        Self {
            actor: value.actor.into(),
            target_action_id: value.target_action_id,
            expected_current_hash: value.expected_current_hash,
            reason: value.reason,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OntologyValidateCommand {
    pub actor: OntologyActorCommand,
    pub parent_action_id: String,
    pub signal_ids: Vec<String>,
    pub reason: String,
    pub validation_status: String,
    pub validation: Value,
}

impl From<OntologyValidateCommand> for crate::store_operations::OntologyValidateInput {
    fn from(value: OntologyValidateCommand) -> Self {
        Self {
            actor: value.actor.into(),
            parent_action_id: value.parent_action_id,
            signal_ids: value.signal_ids,
            reason: value.reason,
            validation_status: value.validation_status,
            validation_json: value.validation.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteLabelSemanticsCommand {
    pub expected_semantics_hash: String,
    pub reason: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelAtomIndexQuery {
    pub query: Option<String>,
    pub polarity: Option<String>,
    pub limit: usize,
}

impl Default for LabelAtomIndexQuery {
    fn default() -> Self {
        Self {
            query: None,
            polarity: None,
            limit: 24,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelOntologySignalQuery {
    pub statuses: Vec<String>,
    pub kinds: Vec<String>,
    pub task_ref: Option<String>,
    pub target_label_ref: Option<String>,
    pub proposed_label_name: Option<String>,
    pub include_all: bool,
    pub limit: usize,
}

impl Default for LabelOntologySignalQuery {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelOntologyReviewQuery {
    pub group_by: String,
    pub include_all: bool,
    pub limit: usize,
}

impl Default for LabelOntologyReviewQuery {
    fn default() -> Self {
        Self {
            group_by: "target_label".to_owned(),
            include_all: false,
            limit: 100,
        }
    }
}
