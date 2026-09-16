//! Ontology 的具名传输适配；字段转换后只调用共享 application facade。

mod convert;
mod enums;

use kanban_protocol as wire;
use kanban_service as service;
use serde_json::{Value, json};

use super::support::{CallContext, request_actor};
use crate::{error::ApiError, state::AppState};

fn body_value(value: wire::JsonBodyFieldWire, default: Value) -> Value {
    match value {
        wire::JsonBodyFieldWire::Missing => default,
        wire::JsonBodyFieldWire::Present(value) => value,
    }
}

fn actor(value: wire::LabelOntologyActorWire) -> service::OntologyActorCommand {
    service::OntologyActorCommand {
        name: value.name,
        actor_type: value.actor_type,
        agent_type: value.agent_type,
    }
}

fn signal(value: wire::LabelOntologySignalRequest) -> service::OntologySignalCommand {
    let (candidate_atom_polarity, candidate_atom_kind, candidate_text) = value
        .candidate_atom
        .map_or((None, None, None), |candidate| {
            (
                Some(candidate.polarity),
                Some(candidate.kind),
                Some(candidate.text),
            )
        });
    service::OntologySignalCommand {
        kind: enums::signal_kind_literal(&value.kind).to_owned(),
        target_label_ref: value.target_label_ref,
        related_labels: body_value(value.related_labels, json!([])),
        proposed_action: enums::proposed_action_literal(&value.proposed_action).to_owned(),
        candidate_atom_polarity,
        candidate_atom_kind,
        candidate_text,
        proposed_label_name: value.proposed_label_name,
        proposal: body_value(value.proposal, json!({})),
        agent_selected: value.agent_selected,
        suggest_state: value
            .suggest_state
            .as_ref()
            .map(|value| enums::suggest_state_literal(value).to_owned()),
        suggest_score: value.suggest_score,
        suggest_rank: value.suggest_rank,
        final_selected: value.final_selected,
        rationale: value.rationale,
        confidence: value.confidence,
        signal_key: value.signal_key,
    }
}

async fn task_board(
    state: &AppState,
    task_id: &str,
    requested_board: Option<&str>,
) -> Result<String, ApiError> {
    let task = state.application().get_task(task_id).await?;
    if let Some(board) = requested_board
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && board != task.board_id
        && board != task.board_slug
    {
        return Err(service::KanbanError::InvalidInput(format!(
            "task {task_id} 不属于 board {board}"
        ))
        .into());
    }
    Ok(task.board_id)
}

pub(crate) async fn list_semantics(
    state: AppState,
    path: wire::BoardLabelPath,
) -> Result<wire::ListLabelSemanticsResponse, ApiError> {
    let records = state
        .application()
        .list_label_semantics(&path.board)
        .await?;
    Ok(wire::DataEnvelope::new(
        records.into_iter().map(convert::label_semantics).collect(),
    ))
}

pub(crate) async fn get_semantics(
    state: AppState,
    path: wire::LabelSemanticsPath,
) -> Result<wire::GetLabelSemanticsResponse, ApiError> {
    let record = state
        .application()
        .get_label_semantics(&path.board, &path.label_id)
        .await?;
    Ok(wire::DataEnvelope::new(convert::label_semantics(record)))
}

pub(crate) async fn upsert_semantics(
    state: AppState,
    path: wire::LabelSemanticsPath,
    context: CallContext,
    body: wire::UpsertLabelSemanticsRequest,
) -> Result<wire::UpsertLabelSemanticsResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &context, "http")?;
    let record = state
        .application()
        .upsert_label_semantics(
            &path.board,
            &path.label_id,
            service::UpsertLabelSemanticsCommand {
                expected_semantics_hash: body.expected_semantics_hash,
                replace: body.replace,
                description: body.description,
                applies_when: body.applies_when,
                excludes_when: body.excludes_when,
                positive_examples: body.positive_examples,
                negative_examples: body.negative_examples,
                remove_applies_when: body.remove_applies_when,
                remove_excludes_when: body.remove_excludes_when,
                remove_positive_examples: body.remove_positive_examples,
                remove_negative_examples: body.remove_negative_examples,
                actor,
                reason: body.reason,
                source_signal_ids: body.source_signal_ids,
            },
        )
        .await?;
    Ok(wire::DataEnvelope::new(convert::label_semantics(record)))
}

pub(crate) async fn delete_semantics(
    state: AppState,
    path: wire::LabelSemanticsPath,
    context: CallContext,
    query: wire::DeleteLabelSemanticsQuery,
) -> Result<wire::DeleteResponse, ApiError> {
    let deleted = state
        .application()
        .delete_label_semantics(
            &path.board,
            &path.label_id,
            service::DeleteLabelSemanticsCommand {
                expected_semantics_hash: query.expected_semantics_hash,
                reason: query.reason,
                actor: request_actor(None, &context, "user")?,
            },
        )
        .await?;
    Ok(wire::DataEnvelope::new(wire::DeleteResult { deleted }))
}

pub(crate) async fn list_atoms(
    state: AppState,
    path: wire::BoardLabelPath,
) -> Result<wire::ListLabelAtomsResponse, ApiError> {
    let records = state.application().list_label_atoms(&path.board).await?;
    Ok(wire::DataEnvelope::new(
        records.into_iter().map(convert::label_atom).collect(),
    ))
}

pub(crate) async fn explain_atom(
    state: AppState,
    path: wire::LabelAtomPath,
) -> Result<wire::ExplainLabelAtomResponse, ApiError> {
    let record = state
        .application()
        .explain_label_atom(&path.board, &path.atom_ref)
        .await?;
    Ok(wire::DataEnvelope::new(
        convert::label_atom_explain(&state, record).await?,
    ))
}

pub(crate) async fn index_status(
    state: AppState,
    path: wire::BoardLabelPath,
) -> Result<wire::LabelAtomIndexStatusResponse, ApiError> {
    let record = state
        .application()
        .label_atom_index_status(&path.board)
        .await?;
    Ok(wire::DataEnvelope::new(convert::label_atom_index_status(
        record,
    )))
}

pub(crate) async fn rebuild_index(
    state: AppState,
    path: wire::BoardLabelPath,
) -> Result<wire::RebuildLabelAtomIndexResponse, ApiError> {
    let record = state
        .application()
        .rebuild_label_atom_index(&path.board)
        .await?;
    Ok(wire::DataEnvelope::new(convert::label_atom_index_status(
        record,
    )))
}

pub(crate) async fn query_index(
    state: AppState,
    path: wire::BoardLabelPath,
    query: wire::LabelAtomIndexQuery,
) -> Result<wire::rpc::dto::LabelAtomIndexQueryResponse, ApiError> {
    let record = state
        .application()
        .query_label_atom_index(
            &path.board,
            service::LabelAtomIndexQuery {
                query: query.q,
                polarity: query.polarity,
                limit: query.limit,
            },
        )
        .await?;
    Ok(wire::DataEnvelope::new(convert::label_atom_index_query(
        record,
    )))
}

pub(crate) async fn suggestions(
    state: AppState,
    path: wire::TaskLabelSurfacePath,
    query: wire::rpc::dto::TaskLabelSuggestionQuery,
) -> Result<wire::SuggestTaskLabelsResponse, ApiError> {
    let board = task_board(&state, &path.task_id, query.board.as_deref()).await?;
    let record = state
        .application()
        .suggest_task_labels(
            &board,
            &path.task_id,
            service::LabelSuggestionOptions {
                output_limit: query.limit,
                candidate_limit: query.candidate_limit,
                atom_limit: query.atom_limit,
                max_selected_labels: query.max_selected_labels,
                min_score: query.min_score,
            },
        )
        .await?;
    Ok(wire::DataEnvelope::new(convert::label_suggestion_result(
        record,
    )))
}

pub(crate) async fn list_proposals_for_task(
    state: AppState,
    path: wire::TaskLabelSurfacePath,
    query: wire::rpc::dto::TaskLabelProposalQuery,
) -> Result<wire::ListTaskLabelProposalsResponse, ApiError> {
    let board = task_board(&state, &path.task_id, query.board.as_deref()).await?;
    let records = state
        .application()
        .list_label_proposals(&board, Some(&path.task_id), query.status.as_deref())
        .await?;
    Ok(wire::DataEnvelope::new(
        records
            .into_iter()
            .map(convert::label_semantic_proposal)
            .collect::<Result<_, _>>()?,
    ))
}

pub(crate) async fn list_proposals_for_board(
    state: AppState,
    path: wire::ListBoardLabelProposalsPath,
    query: wire::ListBoardLabelProposalsQuery,
) -> Result<wire::ListBoardLabelProposalsResponse, ApiError> {
    let status = query
        .status
        .as_ref()
        .map(|value| enums::proposal_status_literal(value).to_owned());
    let records = state
        .application()
        .list_label_proposals(&path.board, None, status.as_deref())
        .await?;
    Ok(wire::DataEnvelope::new(
        records
            .into_iter()
            .map(convert::label_semantic_proposal)
            .collect::<Result<_, _>>()?,
    ))
}

pub(crate) async fn propose_for_task(
    state: AppState,
    path: wire::TaskLabelSurfacePath,
    context: CallContext,
    query: wire::rpc::dto::TaskLabelSuggestionQuery,
    body: Option<wire::ProposeTaskLabelRequest>,
) -> Result<wire::ProposeTaskLabelResponse, ApiError> {
    let board = task_board(&state, &path.task_id, query.board.as_deref()).await?;
    let body_actor = body.as_ref().and_then(|body| {
        body.actor.as_deref().or_else(|| {
            body.ontology_actor
                .as_ref()
                .map(|actor| actor.name.as_str())
        })
    });
    let actor = request_actor(body_actor, &context, "user")?;
    let (candidate, source_signal_ids) = body.map_or((None, Vec::new()), |body| {
        (body.proposal, body.source_signal_ids)
    });
    let command = match candidate {
        Some(candidate) => service::LabelProposalCommand {
            name: Some(candidate.name),
            description: candidate.description,
            applies_when: candidate.applies_when,
            excludes_when: candidate.excludes_when,
            positive_examples: candidate.positive_examples,
            negative_examples: candidate.negative_examples,
            actor,
            source_signal_ids,
        },
        None => service::LabelProposalCommand {
            name: None,
            description: None,
            applies_when: Vec::new(),
            excludes_when: Vec::new(),
            positive_examples: Vec::new(),
            negative_examples: Vec::new(),
            actor,
            source_signal_ids,
        },
    };
    let record = state
        .application()
        .propose_task_label(&board, &path.task_id, command)
        .await?;
    Ok(wire::DataEnvelope::new(convert::label_proposal_attempt(
        record,
    )?))
}

pub(crate) async fn record_observation(
    state: AppState,
    path: wire::TaskLabelSurfacePath,
    query: wire::rpc::dto::TaskLabelBoardQuery,
    body: wire::RecordLabelOntologyObservationRequest,
) -> Result<wire::RecordLabelOntologyObservationResponse, ApiError> {
    let board = task_board(&state, &path.task_id, query.board.as_deref()).await?;
    let record = state
        .application()
        .record_label_ontology_observation(
            &board,
            service::OntologyObservationCommand {
                actor: actor(body.actor),
                task_ref: path.task_id,
                agent_candidates: body_value(body.agent_candidates, json!([])),
                suggestion_snapshot: body_value(body.suggestion_snapshot, json!({})),
                final_decision: body_value(body.final_decision, json!({})),
                suggest_coverage: body.suggest_coverage,
                suggest_coverage_cosine: body.suggest_coverage_cosine,
                suggest_residual_norm: body.suggest_residual_norm,
                suggest_needs_new_label: body.suggest_needs_new_label.unwrap_or(false),
                suggest_degraded: body.suggest_degraded.unwrap_or(false),
                diagnostics: body_value(body.diagnostics, json!([])),
                capture_fingerprint: body.capture_fingerprint,
                signals: body.signals.into_iter().map(signal).collect(),
            },
        )
        .await?;
    Ok(wire::DataEnvelope::new(
        convert::label_ontology_observation(record)?,
    ))
}

pub(crate) async fn list_signals(
    state: AppState,
    path: wire::BoardLabelPath,
    query: wire::LabelOntologySignalQuery,
) -> Result<wire::LabelOntologySignalsResponse, ApiError> {
    let meta = wire::SignalFilterMeta {
        include_all: query.include_all,
        limit: query.limit,
    };
    let records = state
        .application()
        .list_label_ontology_signals(
            &path.board,
            service::LabelOntologySignalQuery {
                statuses: query
                    .status
                    .into_iter()
                    .flat_map(|value| value.split(',').map(str::to_owned).collect::<Vec<_>>())
                    .collect(),
                kinds: query
                    .kind
                    .into_iter()
                    .flat_map(|value| value.split(',').map(str::to_owned).collect::<Vec<_>>())
                    .collect(),
                task_ref: query.task_ref,
                target_label_ref: query.target_label_ref,
                proposed_label_name: query.proposed_label_name,
                include_all: query.include_all,
                limit: query.limit,
            },
        )
        .await?;
    Ok(wire::LabelOntologySignalsResponse {
        data: records
            .into_iter()
            .map(convert::label_ontology_signal)
            .collect::<Result<_, _>>()?,
        meta,
    })
}

pub(crate) async fn review_signals(
    state: AppState,
    path: wire::BoardLabelPath,
    query: wire::LabelOntologyReviewQuery,
) -> Result<wire::ReviewLabelOntologyResponse, ApiError> {
    let group_by = enums::review_group_literal(&query.group_by).to_owned();
    let meta = wire::LabelOntologyReviewMeta {
        group_by: group_by.clone(),
        include_all: query.include_all,
        limit: query.limit,
    };
    let records = state
        .application()
        .review_label_ontology(
            &path.board,
            service::LabelOntologyReviewQuery {
                group_by,
                include_all: query.include_all,
                limit: query.limit,
            },
        )
        .await?;
    Ok(wire::MetadataEnvelope {
        data: records
            .into_iter()
            .map(convert::label_ontology_review_group)
            .collect::<Result<_, _>>()?,
        meta,
    })
}

pub(crate) async fn get_label_ontology_quality(
    state: AppState,
    path: wire::BoardLabelPath,
    sample_limit: usize,
) -> Result<wire::cli_labels::CliLabelOntologyQualityOutput, ApiError> {
    let record = state
        .application()
        .label_ontology_quality(&path.board, sample_limit)
        .await?;
    Ok(wire::DataEnvelope::new(convert::label_ontology_quality(
        record,
    )))
}

pub(crate) async fn get_signal(
    state: AppState,
    path: wire::SignalPath,
) -> Result<wire::GetLabelOntologySignalResponse, ApiError> {
    let record = state
        .application()
        .get_label_ontology_signal(&path.signal_id)
        .await?;
    Ok(wire::DataEnvelope::new(
        convert::label_ontology_signal_detail(record)?,
    ))
}

pub(crate) async fn get_proposal(
    state: AppState,
    path: wire::ProposalPath,
) -> Result<wire::GetLabelProposalResponse, ApiError> {
    let record = state
        .application()
        .get_label_proposal(&path.proposal_id)
        .await?;
    Ok(wire::DataEnvelope::new(convert::label_semantic_proposal(
        record,
    )?))
}

async fn decide_proposal(
    state: AppState,
    path: wire::ProposalPath,
    context: CallContext,
    body: wire::LabelProposalDecisionRequest,
    accept: bool,
) -> Result<wire::LabelProposalDecisionResponse, ApiError> {
    let body_actor = body.actor.as_deref().or_else(|| {
        body.ontology_actor
            .as_ref()
            .map(|actor| actor.name.as_str())
    });
    let actor = request_actor(body_actor, &context, "user")?;
    let record = state
        .application()
        .decide_label_proposal(service::LabelProposalDecisionCommand {
            proposal_id: path.proposal_id,
            accept,
            reason: body.reason,
            actor,
            source_signal_ids: body.source_signal_ids,
        })
        .await?;
    Ok(wire::DataEnvelope::new(convert::label_semantic_proposal(
        record,
    )?))
}

pub(crate) async fn accept_proposal(
    state: AppState,
    path: wire::ProposalPath,
    context: CallContext,
    body: wire::LabelProposalDecisionRequest,
) -> Result<wire::LabelProposalDecisionResponse, ApiError> {
    decide_proposal(state, path, context, body, true).await
}

pub(crate) async fn reject_proposal(
    state: AppState,
    path: wire::ProposalPath,
    context: CallContext,
    body: wire::LabelProposalDecisionRequest,
) -> Result<wire::LabelProposalDecisionResponse, ApiError> {
    decide_proposal(state, path, context, body, false).await
}

pub(crate) async fn create_action(
    state: AppState,
    path: wire::BoardLabelPath,
    body: wire::LabelOntologyActionRequest,
) -> Result<wire::LabelOntologyActionResponse, ApiError> {
    let record = state
        .application()
        .create_label_ontology_action(
            &path.board,
            service::OntologyActionCommand {
                actor: actor(body.actor),
                idempotency_key: body.idempotency_key,
                action_type: enums::action_type_literal(&body.action_type).to_owned(),
                signal_ids: body.signal_ids,
                reason: body.reason,
                superseded_by_signal_id: body.superseded_by_signal_id,
                parent_action_id: body.parent_action_id,
                target_label_ref: body.target_label_ref,
                result_label_ref: body.result_label_ref,
                result_atom_id: body.result_atom_id,
                result_atom_content_hash: body.result_atom_content_hash,
                result_proposal_id: body.result_proposal_id,
                canonical_before_hash: body.canonical_before_hash,
                canonical_after_hash: body.canonical_after_hash,
                change: body_value(body.change, json!({})),
                validation_status: body
                    .validation_status
                    .as_ref()
                    .map(|value| enums::validation_status_literal(value).to_owned()),
                validation: body_value(body.validation, json!({})),
            },
        )
        .await?;
    Ok(wire::DataEnvelope::new(convert::label_ontology_action(
        record,
    )?))
}

pub(crate) async fn apply_atom(
    state: AppState,
    path: wire::BoardLabelPath,
    body: wire::ApplyLabelOntologyAtomRequest,
) -> Result<wire::LabelOntologyActionResponse, ApiError> {
    let polarity = match body.kind.as_str() {
        "excludes_when" | "negative_example" => "negative",
        _ => "positive",
    };
    let record = state
        .application()
        .apply_label_ontology_atom(
            &path.board,
            service::OntologyApplyAtomCommand {
                actor: actor(body.actor),
                signal_ids: body.signal_ids,
                label_ref: body.label_ref,
                polarity: polarity.to_owned(),
                kind: body.kind,
                text: body.text,
                reason: body.reason,
            },
        )
        .await?;
    Ok(wire::DataEnvelope::new(convert::label_ontology_action(
        record,
    )?))
}

pub(crate) async fn revert(
    state: AppState,
    path: wire::BoardLabelPath,
    body: wire::RevertLabelOntologyMutationRequest,
) -> Result<wire::LabelOntologyActionResponse, ApiError> {
    let record = state
        .application()
        .revert_label_ontology_mutation(
            &path.board,
            service::OntologyRevertCommand {
                actor: actor(body.actor),
                target_action_id: body.target_action_id,
                expected_current_hash: body.expected_current_hash,
                reason: body.reason,
            },
        )
        .await?;
    Ok(wire::DataEnvelope::new(convert::label_ontology_action(
        record,
    )?))
}

pub(crate) async fn validate(
    state: AppState,
    path: wire::BoardLabelPath,
    body: wire::ValidateLabelOntologyActionRequest,
) -> Result<wire::LabelOntologyActionResponse, ApiError> {
    let record = state
        .application()
        .validate_label_ontology_action(
            &path.board,
            service::OntologyValidateCommand {
                actor: actor(body.actor),
                parent_action_id: body.parent_action_id,
                signal_ids: body.signal_ids,
                reason: body.reason,
                validation_status: enums::validation_status_literal(&body.validation_status)
                    .to_owned(),
                validation: body_value(body.validation, json!({})),
            },
        )
        .await?;
    Ok(wire::DataEnvelope::new(convert::label_ontology_action(
        record,
    )?))
}

#[cfg(test)]
mod tests;
