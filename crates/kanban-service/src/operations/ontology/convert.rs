//! Store record 到 application record 的显式转换。

use kanban_core::{KanbanError, Result};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{LabelAtomRecord, LabelSemanticsRecord, domain};

use super::records::*;

fn json_component<T: DeserializeOwned>(raw: &str, field: &str) -> Result<T> {
    serde_json::from_str(raw)
        .map_err(|error| KanbanError::Storage(format!("label ontology {field} 无法解码：{error}")))
}

fn public_change(raw: &str) -> Result<Value> {
    let mut change: Value = json_component(raw, "change_json")?;
    if let Some(object) = change.as_object_mut() {
        object.remove("_idempotency_key");
        object.remove("_idempotency_fingerprint");
    }
    Ok(change)
}

impl From<domain::LabelSemanticsRecord> for LabelSemanticsRecord {
    fn from(value: domain::LabelSemanticsRecord) -> Self {
        Self {
            label_id: value.label_id,
            board_id: value.board_id,
            label_name: value.label_name,
            semantics_hash: value.semantics_hash,
            description: value.description,
            applies_when: value.applies_when,
            excludes_when: value.excludes_when,
            positive_examples: value.positive_examples,
            negative_examples: value.negative_examples,
            created_at: value.created_at,
            updated_at: value.updated_at,
            atoms: value.atoms.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<domain::LabelAtomRecord> for LabelAtomRecord {
    fn from(value: domain::LabelAtomRecord) -> Self {
        Self {
            id: value.id,
            label_id: value.label_id,
            board_id: value.board_id,
            label_name: value.label_name,
            polarity: value.polarity,
            kind: value.kind,
            text: value.text,
            ordinal: value.ordinal,
            content_hash: value.content_hash,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl TryFrom<domain::LabelAtomExplainActionRecord> for LabelAtomExplainActionRecord {
    type Error = KanbanError;

    fn try_from(value: domain::LabelAtomExplainActionRecord) -> Result<Self> {
        Ok(Self {
            action: value.action.try_into()?,
            matched_by: value.matched_by,
        })
    }
}

impl TryFrom<domain::LabelAtomExplainSignalRecord> for LabelAtomExplainSignalRecord {
    type Error = KanbanError;

    fn try_from(value: domain::LabelAtomExplainSignalRecord) -> Result<Self> {
        Ok(Self {
            signal: value.signal.try_into()?,
            observation: value.observation.try_into()?,
            task_id: value.task_id,
            task_ref_snapshot: value.task_ref_snapshot,
            suggest_input_stale: value.suggest_input_stale,
            suggest_degraded: value.suggest_degraded,
            warnings: value.warnings,
        })
    }
}

impl TryFrom<domain::LabelAtomExplainValidationRecord> for LabelAtomExplainValidationRecord {
    type Error = KanbanError;

    fn try_from(value: domain::LabelAtomExplainValidationRecord) -> Result<Self> {
        Ok(Self {
            validation_effective_outcome: value.validation_status.clone(),
            validation_latest_attempt_id: None,
            action: value.action.try_into()?,
            parent_action_id: value.parent_action_id,
            validation_status: value.validation_status,
            manual: json_component(&value.manual_json, "manual_json")?,
            summary: json_component(&value.summary_json, "summary_json")?,
            cases: json_component(&value.cases_json, "cases_json")?,
            warnings: value.warnings,
        })
    }
}

impl TryFrom<domain::LabelAtomExplainRecord> for LabelAtomExplainRecord {
    type Error = KanbanError;

    fn try_from(value: domain::LabelAtomExplainRecord) -> Result<Self> {
        Ok(Self {
            query: value.query,
            atom: value.atom.map(Into::into),
            current_semantics: value.current_semantics.map(Into::into),
            provenance_actions: value
                .provenance_actions
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
            supporting_signals: value
                .supporting_signals
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
            validation_history: value
                .validation_history
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
            legacy_untracked: value.legacy_untracked,
            legacy_reason: value.legacy_reason,
        })
    }
}

impl From<domain::LabelAtomIndexStatusRecord> for LabelAtomIndexStatusRecord {
    fn from(value: domain::LabelAtomIndexStatusRecord) -> Self {
        Self {
            backend: value.backend,
            enabled: value.enabled,
            message: value.message,
            diagnostics: value.diagnostics,
            dirty: value.dirty,
            board_dirty: value.board_dirty,
            generation: value.generation,
        }
    }
}

impl From<domain::LabelSuggestionResultRecord> for LabelSuggestionResultRecord {
    fn from(value: domain::LabelSuggestionResultRecord) -> Self {
        Self {
            task_id: value.task_id,
            board_id: value.board_id,
            selected_labels: value.selected_labels.into_iter().map(Into::into).collect(),
            candidates: value.candidates.into_iter().map(Into::into).collect(),
            coverage: value.coverage,
            coverage_cosine: value.coverage_cosine,
            residual_norm: value.residual_norm,
            needs_new_label: value.needs_new_label,
            reason_codes: value.reason_codes,
            degraded: value.degraded,
            diagnostics: value.diagnostics,
        }
    }
}

impl From<domain::LabelSuggestionCandidateRecord> for LabelSuggestionCandidateRecord {
    fn from(value: domain::LabelSuggestionCandidateRecord) -> Self {
        Self {
            label_id: value.label_id,
            label_name: value.label_name,
            score: value.score,
            weight: value.weight,
            already_applied: value.already_applied,
            evidence_atoms: value.evidence_atoms.into_iter().map(Into::into).collect(),
            negative_evidence_atoms: value
                .negative_evidence_atoms
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<domain::LabelSuggestionEvidenceRecord> for LabelSuggestionEvidenceRecord {
    fn from(value: domain::LabelSuggestionEvidenceRecord) -> Self {
        Self {
            atom_id: value.atom_id,
            label_id: value.label_id,
            label_name: value.label_name,
            polarity: value.polarity,
            kind: value.kind,
            text: value.text,
            score: value.score,
        }
    }
}

impl From<domain::LabelSemanticProposalRecord> for LabelSemanticProposalRecord {
    fn from(value: domain::LabelSemanticProposalRecord) -> Self {
        Self {
            id: value.id,
            board_id: value.board_id,
            task_id: value.task_id,
            status: value.status,
            name: value.name,
            description: value.description,
            applies_when: value.applies_when,
            excludes_when: value.excludes_when,
            positive_examples: value.positive_examples,
            negative_examples: value.negative_examples,
            heuristic_coverage: value.heuristic_coverage,
            heuristic_coverage_cosine: value.heuristic_coverage_cosine,
            heuristic_residual_norm: value.heuristic_residual_norm,
            top1_existing_label_id: value.top1_existing_label_id,
            top1_existing_label_name: value.top1_existing_label_name,
            diagnostics: value.diagnostics,
            created_by: value.created_by,
            decision_reason: value.decision_reason,
            resolved_label_id: value.resolved_label_id,
            created_at: value.created_at,
            updated_at: value.updated_at,
            decided_at: value.decided_at,
        }
    }
}

impl From<domain::LabelProposalAttemptRecord> for LabelProposalAttemptRecord {
    fn from(value: domain::LabelProposalAttemptRecord) -> Self {
        Self {
            task_id: value.task_id,
            board_id: value.board_id,
            proposal: value.proposal.map(Into::into),
            degraded: value.degraded,
            diagnostics: value.diagnostics,
            heuristic_coverage: value.heuristic_coverage,
            heuristic_coverage_cosine: value.heuristic_coverage_cosine,
            heuristic_residual_norm: value.heuristic_residual_norm,
            top1_existing_label_id: value.top1_existing_label_id,
            top1_existing_label_name: value.top1_existing_label_name,
        }
    }
}

impl TryFrom<domain::LabelOntologyObservationRecord> for LabelOntologyObservationRecord {
    type Error = KanbanError;

    fn try_from(value: domain::LabelOntologyObservationRecord) -> Result<Self> {
        Ok(Self {
            id: value.id,
            board_id: value.board_id,
            task_id: value.task_id,
            task_ref_snapshot: value.task_ref_snapshot,
            task_snapshot: json_component(&value.task_snapshot_json, "task_snapshot_json")?,
            suggest_input_hash: value.suggest_input_hash,
            agent_candidates: json_component(
                &value.agent_candidates_json,
                "agent_candidates_json",
            )?,
            suggestion_snapshot: json_component(
                &value.suggestion_snapshot_json,
                "suggestion_snapshot_json",
            )?,
            final_decision: json_component(&value.final_decision_json, "final_decision_json")?,
            suggest_coverage: value.suggest_coverage,
            suggest_coverage_cosine: value.suggest_coverage_cosine,
            suggest_residual_norm: value.suggest_residual_norm,
            suggest_needs_new_label: value.suggest_needs_new_label,
            suggest_degraded: value.suggest_degraded,
            diagnostics: json_component(&value.diagnostics_json, "diagnostics_json")?,
            capture_fingerprint: value.capture_fingerprint,
            created_by: value.created_by,
            created_by_type: value.created_by_type,
            agent_type: value.agent_type,
            created_at: value.created_at,
            signals: value
                .signals
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
        })
    }
}

impl TryFrom<domain::LabelOntologySignalRecord> for LabelOntologySignalRecord {
    type Error = KanbanError;

    fn try_from(value: domain::LabelOntologySignalRecord) -> Result<Self> {
        Ok(Self {
            id: value.id,
            observation_id: value.observation_id,
            board_id: value.board_id,
            kind: value.kind,
            status: value.status,
            target_label_id: value.target_label_id,
            target_label_name_snapshot: value.target_label_name_snapshot,
            related_labels: json_component(&value.related_labels_json, "related_labels_json")?,
            proposed_action: value.proposed_action,
            candidate_atom_polarity: value.candidate_atom_polarity,
            candidate_atom_kind: value.candidate_atom_kind,
            candidate_text: value.candidate_text,
            candidate_content_hash: value.candidate_content_hash,
            proposed_label_name: value.proposed_label_name,
            proposed_label_name_normalized: value.proposed_label_name_normalized,
            proposal: json_component(&value.proposal_json, "proposal_json")?,
            agent_selected: value.agent_selected,
            suggest_state: value.suggest_state,
            suggest_score: value.suggest_score,
            suggest_rank: value.suggest_rank,
            final_selected: value.final_selected,
            rationale: value.rationale,
            confidence: value.confidence,
            signal_key: value.signal_key,
            superseded_by_signal_id: value.superseded_by_signal_id,
            status_reason: value.status_reason,
            created_at: value.created_at,
            updated_at: value.updated_at,
            reviewed_at: value.reviewed_at,
            closed_at: value.closed_at,
        })
    }
}

impl TryFrom<domain::LabelOntologyActionRecord> for LabelOntologyActionRecord {
    type Error = KanbanError;

    fn try_from(value: domain::LabelOntologyActionRecord) -> Result<Self> {
        Ok(Self {
            validation_effective_outcome: value.validation_status.clone(),
            validation_latest_attempt_id: None,
            id: value.id,
            board_id: value.board_id,
            parent_action_id: value.parent_action_id,
            action_type: value.action_type,
            reason: value.reason,
            target_label_id: value.target_label_id,
            result_label_id: value.result_label_id,
            result_atom_id: value.result_atom_id,
            result_atom_content_hash: value.result_atom_content_hash,
            result_proposal_id: value.result_proposal_id,
            canonical_before_hash: value.canonical_before_hash,
            canonical_after_hash: value.canonical_after_hash,
            change: public_change(&value.change_json)?,
            validation_requirement: value.validation_requirement,
            validation_status: value.validation_status,
            validation: json_component(&value.validation_json, "validation_json")?,
            created_by: value.created_by,
            created_by_type: value.created_by_type,
            agent_type: value.agent_type,
            created_at: value.created_at,
            signal_ids: value.signal_ids,
        })
    }
}

impl TryFrom<domain::LabelOntologySignalDetailRecord> for LabelOntologySignalDetailRecord {
    type Error = KanbanError;

    fn try_from(value: domain::LabelOntologySignalDetailRecord) -> Result<Self> {
        Ok(Self {
            signal: value.signal.try_into()?,
            observation: value.observation.try_into()?,
            actions: value
                .actions
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_>>()?,
        })
    }
}

impl TryFrom<domain::LabelOntologyReviewGroupRecord> for LabelOntologyReviewGroupRecord {
    type Error = KanbanError;

    fn try_from(value: domain::LabelOntologyReviewGroupRecord) -> Result<Self> {
        Ok(Self {
            group_by: value.group_by,
            key: value.key,
            label_id: value.label_id,
            label_name: value.label_name,
            candidate_atom_polarity: value.candidate_atom_polarity,
            candidate_atom_kind: value.candidate_atom_kind,
            candidate_text: value.candidate_text,
            candidate_content_hash: value.candidate_content_hash,
            proposed_label_name: value.proposed_label_name,
            proposed_label_name_normalized: value.proposed_label_name_normalized,
            cluster_key: value.cluster_key,
            cluster_reason: value.cluster_reason,
            task_count: value.task_count,
            signal_count: value.signal_count,
            open_count: value.open_count,
            confirmed_count: value.confirmed_count,
            resolved_count: value.resolved_count,
            rejected_count: value.rejected_count,
            superseded_count: value.superseded_count,
            degraded_count: value.degraded_count,
            average_score: value.average_score,
            median_score: value.median_score,
            oldest_signal_at: value.oldest_signal_at,
            latest_signal_at: value.latest_signal_at,
            sample_task_refs: value.sample_task_refs,
            signal_ids: value.signal_ids,
            action_count: value.action_count,
            action_ids: value.action_ids,
            proposal_ids: value.proposal_ids,
            labels: json_component::<Vec<String>>(&value.labels_json, "labels_json")?
                .into_iter()
                .map(|id| LabelOntologyReviewLabelRefRecord { id, name: None })
                .collect(),
            candidate_atom_variants: json_component(
                &value.candidate_atom_variants_json,
                "candidate_atom_variants_json",
            )?,
        })
    }
}

impl TryFrom<domain::LabelOntologyQualityRecord> for LabelOntologyQualityRecord {
    type Error = KanbanError;

    fn try_from(value: domain::LabelOntologyQualityRecord) -> Result<Self> {
        Ok(Self {
            board_id: value.board_id,
            denominator: json_component(&value.denominator_json, "denominator_json")?,
            disagreement: json_component(&value.disagreement_json, "disagreement_json")?,
            rates: json_component(&value.rates_json, "rates_json")?,
            precision_recall: json_component(
                &value.precision_recall_json,
                "precision_recall_json",
            )?,
            warnings: json_component(&value.warnings_json, "warnings_json")?,
        })
    }
}

impl From<domain::LabelAtomIndexQueryRecord> for LabelAtomIndexQueryRecord {
    fn from(value: domain::LabelAtomIndexQueryRecord) -> Self {
        Self {
            data: value.data.into_iter().map(Into::into).collect(),
            degraded: value.degraded,
            diagnostics: value.diagnostics,
        }
    }
}

impl From<domain::LabelAtomIndexHitRecord> for LabelAtomIndexHitRecord {
    fn from(value: domain::LabelAtomIndexHitRecord) -> Self {
        Self {
            atom_id: value.atom_id,
            label_id: value.label_id,
            label_name: value.label_name,
            board_id: value.board_id,
            polarity: value.polarity,
            kind: value.kind,
            text: value.text,
            ordinal: value.ordinal,
            content_hash: value.content_hash,
            embedding_model: value.embedding_model,
            distance: value.distance,
        }
    }
}
