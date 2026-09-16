//! Application record 到 wire DTO 的逐字段转换。

use kanban_protocol as wire;
use kanban_service as service;
use serde_json::Value;

use super::enums;
use crate::{application::tasks::support::api_task, error::ApiError, state::AppState};

fn object(value: Value, field: &str) -> Result<wire::structured_metadata::JsonObject, ApiError> {
    match value {
        Value::Object(value) => Ok(wire::structured_metadata::JsonObject(
            value.into_iter().collect(),
        )),
        _ => Err(service::KanbanError::Storage(format!("ontology {field} 必须是对象")).into()),
    }
}

fn array(value: Value, field: &str) -> Result<wire::structured_metadata::JsonArray, ApiError> {
    match value {
        Value::Array(value) => Ok(wire::structured_metadata::JsonArray(value)),
        _ => Err(service::KanbanError::Storage(format!("ontology {field} 必须是数组")).into()),
    }
}

pub(super) fn label_semantics(value: service::LabelSemanticsRecord) -> wire::LabelSemanticsWire {
    wire::LabelSemanticsWire {
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
        atoms: value.atoms.into_iter().map(label_atom).collect(),
    }
}

pub(super) fn label_atom(value: service::LabelAtomRecord) -> wire::LabelAtomWire {
    wire::LabelAtomWire {
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

pub(super) fn label_atom_explain_action(
    value: service::LabelAtomExplainActionRecord,
) -> Result<wire::LabelAtomExplainActionWire, ApiError> {
    Ok(wire::LabelAtomExplainActionWire {
        action: label_ontology_action(value.action)?,
        matched_by: value.matched_by,
    })
}

pub(super) async fn label_atom_explain_signal(
    state: &AppState,
    value: service::LabelAtomExplainSignalRecord,
) -> Result<wire::LabelAtomExplainSignalWire, ApiError> {
    let source_task = api_task(state.application().get_task(&value.task_id).await?)?;
    Ok(wire::LabelAtomExplainSignalWire {
        signal: label_ontology_signal(value.signal)?,
        observation: label_ontology_observation(value.observation)?,
        source_task,
        task_ref_snapshot: value.task_ref_snapshot,
        suggest_input_stale: value.suggest_input_stale,
        suggest_degraded: value.suggest_degraded,
        warnings: value.warnings,
    })
}

pub(super) fn label_atom_explain_validation(
    value: service::LabelAtomExplainValidationRecord,
) -> Result<wire::LabelAtomExplainValidationWire, ApiError> {
    Ok(wire::LabelAtomExplainValidationWire {
        action: label_ontology_action(value.action)?,
        parent_action_id: value.parent_action_id,
        validation_status: enums::parse_validation_status(&value.validation_status)?,
        manual: value.manual,
        summary: value.summary,
        cases: value.cases,
        warnings: value.warnings,
    })
}

pub(super) async fn label_atom_explain(
    state: &AppState,
    value: service::LabelAtomExplainRecord,
) -> Result<wire::LabelAtomExplainWire, ApiError> {
    let mut supporting_signals = Vec::with_capacity(value.supporting_signals.len());
    for signal in value.supporting_signals {
        supporting_signals.push(label_atom_explain_signal(state, signal).await?);
    }
    Ok(wire::LabelAtomExplainWire {
        query: value.query,
        atom: value.atom.map(label_atom),
        current_semantics: value.current_semantics.map(label_semantics),
        provenance_actions: value
            .provenance_actions
            .into_iter()
            .map(label_atom_explain_action)
            .collect::<Result<_, _>>()?,
        supporting_signals,
        validation_history: value
            .validation_history
            .into_iter()
            .map(label_atom_explain_validation)
            .collect::<Result<_, _>>()?,
        legacy_untracked: value.legacy_untracked,
        legacy_reason: value.legacy_reason,
    })
}

pub(super) fn label_atom_index_status(
    value: service::LabelAtomIndexStatusRecord,
) -> wire::VectorStoreStatusWire {
    wire::VectorStoreStatusWire {
        backend: value.backend,
        enabled: value.enabled,
        message: value.message,
        diagnostics: value.diagnostics,
        dirty: value.dirty,
        board_dirty: value.board_dirty,
        generation: value.generation,
    }
}

pub(super) fn label_suggestion_result(
    value: service::LabelSuggestionResultRecord,
) -> wire::LabelSuggestionResultWire {
    wire::LabelSuggestionResultWire {
        task_id: value.task_id,
        board_id: value.board_id,
        selected_labels: value
            .selected_labels
            .into_iter()
            .map(label_suggestion_candidate)
            .collect(),
        candidates: value
            .candidates
            .into_iter()
            .map(label_suggestion_candidate)
            .collect(),
        coverage: value.coverage,
        coverage_cosine: value.coverage_cosine,
        residual_norm: value.residual_norm,
        needs_new_label: value.needs_new_label,
        reason_codes: value.reason_codes,
        degraded: value.degraded,
        diagnostics: value.diagnostics,
    }
}

pub(super) fn label_suggestion_candidate(
    value: service::LabelSuggestionCandidateRecord,
) -> wire::LabelSuggestionCandidateWire {
    wire::LabelSuggestionCandidateWire {
        label_id: value.label_id,
        label_name: value.label_name,
        score: value.score,
        weight: value.weight,
        already_applied: value.already_applied,
        evidence_atoms: value
            .evidence_atoms
            .into_iter()
            .map(label_suggestion_evidence)
            .collect(),
        negative_evidence_atoms: value
            .negative_evidence_atoms
            .into_iter()
            .map(label_suggestion_evidence)
            .collect(),
    }
}

pub(super) fn label_suggestion_evidence(
    value: service::LabelSuggestionEvidenceRecord,
) -> wire::LabelSuggestionEvidenceAtomWire {
    wire::LabelSuggestionEvidenceAtomWire {
        atom_id: value.atom_id,
        label_id: value.label_id,
        label_name: value.label_name,
        polarity: value.polarity,
        kind: value.kind,
        text: value.text,
        score: value.score,
    }
}

pub(super) fn label_semantic_proposal(
    value: service::LabelSemanticProposalRecord,
) -> Result<wire::LabelSemanticProposalWire, ApiError> {
    Ok(wire::LabelSemanticProposalWire {
        id: value.id,
        board_id: value.board_id,
        task_id: value.task_id,
        status: enums::parse_proposal_status(&value.status)?,
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
    })
}

pub(super) fn label_proposal_attempt(
    value: service::LabelProposalAttemptRecord,
) -> Result<wire::LabelProposalAttemptWire, ApiError> {
    Ok(wire::LabelProposalAttemptWire {
        task_id: value.task_id,
        board_id: value.board_id,
        proposal: value.proposal.map(label_semantic_proposal).transpose()?,
        degraded: value.degraded,
        diagnostics: value.diagnostics,
        heuristic_coverage: value.heuristic_coverage,
        heuristic_coverage_cosine: value.heuristic_coverage_cosine,
        heuristic_residual_norm: value.heuristic_residual_norm,
        top1_existing_label_id: value.top1_existing_label_id,
        top1_existing_label_name: value.top1_existing_label_name,
    })
}

pub(super) fn label_ontology_observation(
    value: service::LabelOntologyObservationRecord,
) -> Result<wire::LabelOntologyObservationWire, ApiError> {
    Ok(wire::LabelOntologyObservationWire {
        id: value.id,
        board_id: value.board_id,
        task_id: value.task_id,
        task_ref_snapshot: value.task_ref_snapshot,
        task_snapshot: object(value.task_snapshot, "task_snapshot")?,
        suggest_input_hash: value.suggest_input_hash,
        agent_candidates: array(value.agent_candidates, "agent_candidates")?,
        suggestion_snapshot: object(value.suggestion_snapshot, "suggestion_snapshot")?,
        final_decision: object(value.final_decision, "final_decision")?,
        suggest_coverage: value.suggest_coverage,
        suggest_coverage_cosine: value.suggest_coverage_cosine,
        suggest_residual_norm: value.suggest_residual_norm,
        suggest_needs_new_label: value.suggest_needs_new_label,
        suggest_degraded: value.suggest_degraded,
        diagnostics: array(value.diagnostics, "diagnostics")?,
        capture_fingerprint: value.capture_fingerprint,
        created_by: value.created_by,
        created_by_type: value.created_by_type,
        agent_type: value.agent_type,
        created_at: value.created_at,
        signals: value
            .signals
            .into_iter()
            .map(label_ontology_signal)
            .collect::<Result<_, _>>()?,
    })
}

pub(super) fn label_ontology_signal(
    value: service::LabelOntologySignalRecord,
) -> Result<wire::LabelOntologySignalWire, ApiError> {
    Ok(wire::LabelOntologySignalWire {
        id: value.id,
        observation_id: value.observation_id,
        board_id: value.board_id,
        kind: value.kind,
        status: value.status,
        target_label_id: value.target_label_id,
        target_label_name_snapshot: value.target_label_name_snapshot,
        related_labels: array(value.related_labels, "related_labels")?,
        proposed_action: value.proposed_action,
        candidate_atom_polarity: value.candidate_atom_polarity,
        candidate_atom_kind: value.candidate_atom_kind,
        candidate_text: value.candidate_text,
        candidate_content_hash: value.candidate_content_hash,
        proposed_label_name: value.proposed_label_name,
        proposed_label_name_normalized: value.proposed_label_name_normalized,
        proposal: object(value.proposal, "proposal")?,
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

pub(super) fn label_ontology_action(
    value: service::LabelOntologyActionRecord,
) -> Result<wire::LabelOntologyActionWire, ApiError> {
    Ok(wire::LabelOntologyActionWire {
        id: value.id,
        board_id: value.board_id,
        parent_action_id: value.parent_action_id,
        action_type: enums::parse_action_type(&value.action_type)?,
        reason: value.reason,
        target_label_id: value.target_label_id,
        result_label_id: value.result_label_id,
        result_atom_id: value.result_atom_id,
        result_atom_content_hash: value.result_atom_content_hash,
        result_proposal_id: value.result_proposal_id,
        canonical_before_hash: value.canonical_before_hash,
        canonical_after_hash: value.canonical_after_hash,
        change: object(value.change, "change")?,
        validation_requirement: enums::parse_validation_requirement(&value.validation_requirement)?,
        validation_status: enums::parse_validation_status(&value.validation_status)?,
        validation_effective_outcome: enums::parse_validation_outcome(
            &value.validation_effective_outcome,
        )?,
        validation_latest_attempt_id: value.validation_latest_attempt_id,
        validation: object(value.validation, "validation")?,
        created_by: value.created_by,
        created_by_type: value.created_by_type,
        agent_type: value.agent_type,
        created_at: value.created_at,
        signal_ids: value.signal_ids,
    })
}

pub(super) fn label_ontology_signal_detail(
    value: service::LabelOntologySignalDetailRecord,
) -> Result<wire::LabelOntologySignalDetailWire, ApiError> {
    Ok(wire::LabelOntologySignalDetailWire {
        signal: label_ontology_signal(value.signal)?,
        observation: label_ontology_observation(value.observation)?,
        actions: value
            .actions
            .into_iter()
            .map(label_ontology_action)
            .collect::<Result<_, _>>()?,
    })
}

pub(super) fn label_ontology_review_group(
    value: service::LabelOntologyReviewGroupRecord,
) -> Result<wire::LabelOntologyReviewGroupWire, ApiError> {
    Ok(wire::LabelOntologyReviewGroupWire {
        group_by: enums::parse_review_group(&value.group_by)?,
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
        labels: value
            .labels
            .into_iter()
            .map(label_ontology_review_label_ref)
            .collect(),
        candidate_atom_variants: value
            .candidate_atom_variants
            .into_iter()
            .map(label_ontology_review_atom_variant)
            .collect(),
    })
}

pub(super) fn label_ontology_quality(
    value: service::LabelOntologyQualityRecord,
) -> wire::cli_labels::CliLabelOntologyQuality {
    wire::cli_labels::CliLabelOntologyQuality {
        board_id: value.board_id,
        denominator: label_ontology_quality_denominator(value.denominator),
        disagreement: label_ontology_quality_disagreement(value.disagreement),
        rates: label_ontology_quality_rates(value.rates),
        precision_recall: label_ontology_precision_recall(value.precision_recall),
        warnings: value.warnings,
    }
}

pub(super) fn label_ontology_review_label_ref(
    value: service::LabelOntologyReviewLabelRefRecord,
) -> wire::LabelOntologyReviewLabelRefWire {
    wire::LabelOntologyReviewLabelRefWire {
        id: value.id,
        name: value.name,
    }
}

pub(super) fn label_ontology_review_atom_variant(
    value: service::LabelOntologyReviewAtomVariantRecord,
) -> wire::LabelOntologyReviewAtomVariantWire {
    wire::LabelOntologyReviewAtomVariantWire {
        content_hash: value.content_hash,
        polarity: value.polarity,
        kind: value.kind,
        text: value.text,
        signal_count: value.signal_count,
    }
}

pub(super) fn label_ontology_quality_denominator(
    value: service::LabelOntologyQualityDenominator,
) -> wire::cli_labels::CliLabelOntologyQualityDenominator {
    wire::cli_labels::CliLabelOntologyQualityDenominator {
        source: value.source,
        description: value.description,
        observation_count: value.observation_count,
        distinct_task_count: value.distinct_task_count,
        agreement_observation_count: value.agreement_observation_count,
        agreement_task_count: value.agreement_task_count,
        degraded_observation_count: value.degraded_observation_count,
        first_observed_at: value.first_observed_at,
        latest_observed_at: value.latest_observed_at,
        sample_task_refs: value.sample_task_refs,
    }
}

pub(super) fn label_ontology_quality_disagreement(
    value: service::LabelOntologyQualityDisagreement,
) -> wire::cli_labels::CliLabelOntologyQualityDisagreement {
    wire::cli_labels::CliLabelOntologyQualityDisagreement {
        signal_count: value.signal_count,
        distinct_task_count: value.distinct_task_count,
        by_kind: value.by_kind,
        by_status: value.by_status,
    }
}

pub(super) fn label_ontology_quality_rates(
    value: service::LabelOntologyQualityRates,
) -> wire::cli_labels::CliLabelOntologyQualityRates {
    wire::cli_labels::CliLabelOntologyQualityRates {
        disagreement_task_rate: value.disagreement_task_rate,
        disagreement_task_rate_basis: value.disagreement_task_rate_basis,
    }
}

pub(super) fn label_ontology_precision_recall(
    value: service::LabelOntologyPrecisionRecall,
) -> wire::cli_labels::CliLabelOntologyPrecisionRecall {
    wire::cli_labels::CliLabelOntologyPrecisionRecall {
        available: value.available,
        reason: value.reason,
    }
}

pub(super) fn label_atom_index_query(
    value: service::LabelAtomIndexQueryRecord,
) -> wire::rpc::dto::LabelAtomIndexQueryData {
    wire::rpc::dto::LabelAtomIndexQueryData {
        data: value.data.into_iter().map(label_atom_index_hit).collect(),
        degraded: value.degraded,
        diagnostics: value.diagnostics,
    }
}

pub(super) fn label_atom_index_hit(
    value: service::LabelAtomIndexHitRecord,
) -> wire::rpc::dto::LabelAtomIndexHit {
    wire::rpc::dto::LabelAtomIndexHit {
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
