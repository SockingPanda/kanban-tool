use crate::connect_file;

use super::label_suggestions::{
    bounded_diagnostic_message, compute_task_label_suggestions_with, retrieve_residual_atoms,
};
use super::{
    LabelProposalAttempt, LabelProposalCandidate, LabelProposalListOptions, LabelProposalStatus,
    LabelSemanticProposalRecord, LabelSuggestionOptions, LabelSuggestionResult, SqlFilter, all,
    all_values, board_id, exec, get_task_by_id, insert_event, mark_label_atom_store_dirty,
    required_row, resolve_task, upsert_label_semantics_candidate_in_tx, with_immediate_tx,
};

use std::{collections::HashMap, path::Path, str::FromStr};

use kanban_core::{Clock, KanbanError, Result, SystemClock, new_label_id, new_typed_id};
use kanban_labels::{LabelAtomPolarity, LabelDefinition};
use kanban_vector::{DisabledVectorStore, LabelAtomVectorStore};
use rusqlite::{Connection, Row, named_params};
use serde_json::json;

const PROPOSAL_COVERAGE_THRESHOLD: f32 = 0.55;
const PROPOSAL_RESIDUAL_MARGIN: f32 = 0.05;
const NEGATIVE_SUPPRESSION_THRESHOLD: f32 = 0.65;
const NEGATIVE_SUPPRESSION_FACTOR: f32 = 0.8;

/// Supplies candidate semantics when the SQLite proposal service needs a new label.
///
/// This trait is the dependency boundary for future LLM/local-AI providers:
/// `kanban-sqlite` owns proposal validation and persistence, but concrete
/// providers live in upper layers such as `kanban-server`, `kanban-cli`, a local
/// runtime, or a separate AI crate. Do not add LLM SDKs, HTTP AI clients, or
/// credential handling to this crate.
pub trait LabelProposalProvider {
    fn propose_label(
        &self,
        task: &super::TaskRecord,
        suggestions: &LabelSuggestionResult,
    ) -> Result<Option<LabelProposalCandidate>>;

    fn unavailable_diagnostics(&self) -> Vec<String> {
        Vec::new()
    }
}

pub struct DisabledLabelProposalProvider;

impl LabelProposalProvider for DisabledLabelProposalProvider {
    fn propose_label(
        &self,
        _task: &super::TaskRecord,
        _suggestions: &LabelSuggestionResult,
    ) -> Result<Option<LabelProposalCandidate>> {
        Ok(None)
    }

    fn unavailable_diagnostics(&self) -> Vec<String> {
        vec!["label_proposal_provider_unavailable".to_owned()]
    }
}

#[derive(Debug, Clone)]
pub struct ManualLabelProposalProvider {
    proposal: LabelProposalCandidate,
}

impl ManualLabelProposalProvider {
    pub fn new(proposal: LabelProposalCandidate) -> Self {
        Self { proposal }
    }
}

impl LabelProposalProvider for ManualLabelProposalProvider {
    fn propose_label(
        &self,
        _task: &super::TaskRecord,
        _suggestions: &LabelSuggestionResult,
    ) -> Result<Option<LabelProposalCandidate>> {
        Ok(Some(self.proposal.clone()))
    }
}

pub fn propose_task_label(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    options: LabelSuggestionOptions,
) -> Result<LabelProposalAttempt> {
    propose_task_label_with(
        path,
        board,
        actor,
        task_ref,
        &DisabledLabelProposalProvider,
        options,
    )
}

pub fn propose_task_label_with(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    provider: &(impl LabelProposalProvider + ?Sized),
    options: LabelSuggestionOptions,
) -> Result<LabelProposalAttempt> {
    let store = DisabledVectorStore;
    propose_task_label_with_store(path, board, actor, task_ref, provider, &store, options)
}

pub fn propose_task_label_with_store(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    provider: &(impl LabelProposalProvider + ?Sized),
    store: &(impl LabelAtomVectorStore + ?Sized),
    options: LabelSuggestionOptions,
) -> Result<LabelProposalAttempt> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task_ref = resolve_task(&conn, &board_id, task_ref)?;
    let task = get_task_by_id(&conn, &task_ref.board_id, &task_ref.id)?;
    let computation = compute_task_label_suggestions_with(
        path.as_ref(),
        &task.board_slug,
        &task.id,
        store,
        options,
    )?;
    let suggestions = computation.result;
    let top1 = suggestions.candidates.first();
    let mut diagnostics = suggestions.diagnostics.clone();
    diagnostics.push("label_proposal_heuristic_context".to_owned());
    if suggestions.coverage >= PROPOSAL_COVERAGE_THRESHOLD && !suggestions.needs_new_label {
        diagnostics.push("heuristic_coverage_sufficient".to_owned());
        diagnostics.sort();
        diagnostics.dedup();
        return Ok(LabelProposalAttempt {
            task_id: task.id,
            board_id: task.board_id,
            proposal: None,
            degraded: suggestions.degraded,
            diagnostics,
            heuristic_coverage: suggestions.coverage,
            heuristic_coverage_cosine: suggestions.coverage_cosine,
            heuristic_residual_norm: suggestions.residual_norm,
            top1_existing_label_id: top1.map(|candidate| candidate.label_id.clone()),
            top1_existing_label_name: top1.map(|candidate| candidate.label_name.clone()),
        });
    }
    let Some(candidate) = provider.propose_label(&task, &suggestions)? else {
        diagnostics.extend(provider.unavailable_diagnostics());
        diagnostics.sort();
        diagnostics.dedup();
        return Ok(LabelProposalAttempt {
            task_id: task.id,
            board_id: task.board_id,
            proposal: None,
            degraded: true,
            diagnostics,
            heuristic_coverage: suggestions.coverage,
            heuristic_coverage_cosine: suggestions.coverage_cosine,
            heuristic_residual_norm: suggestions.residual_norm,
            top1_existing_label_id: top1.map(|candidate| candidate.label_id.clone()),
            top1_existing_label_name: top1.map(|candidate| candidate.label_name.clone()),
        });
    };
    let candidate = normalize_candidate(candidate)?;

    let conflict = normalized_label_conflict(&conn, &task.board_id, &candidate.name)?;
    if conflict.is_some() {
        diagnostics.push("near_duplicate_label_conflict".to_owned());
    }
    let validation = if conflict.is_some() {
        ResidualValidation::not_run()
    } else {
        validate_candidate_residual(
            store,
            &task.board_id,
            store.embedding_model(),
            &computation.query_vector,
            &computation.residual_vector,
            &candidate,
        )
    };
    diagnostics.extend(validation.diagnostics);
    diagnostics.sort();
    diagnostics.dedup();
    let top1_existing_label_id = validation
        .top1_existing_label_id
        .as_deref()
        .or_else(|| top1.map(|candidate| candidate.label_id.as_str()));
    let top1_existing_label_name = validation
        .top1_existing_label_name
        .as_deref()
        .or_else(|| top1.map(|candidate| candidate.label_name.as_str()));
    if conflict.is_none()
        && validation.degraded
        && validation.status == LabelProposalStatus::Proposed
    {
        return Ok(LabelProposalAttempt {
            task_id: task.id,
            board_id: task.board_id,
            proposal: None,
            degraded: true,
            diagnostics,
            heuristic_coverage: suggestions.coverage,
            heuristic_coverage_cosine: suggestions.coverage_cosine,
            heuristic_residual_norm: suggestions.residual_norm,
            top1_existing_label_id: top1_existing_label_id.map(ToOwned::to_owned),
            top1_existing_label_name: top1_existing_label_name.map(ToOwned::to_owned),
        });
    }
    let now = SystemClock.now_ms();
    let status = conflict
        .as_ref()
        .map(|_| LabelProposalStatus::Rejected)
        .unwrap_or_else(|| validation.status.clone());
    let decision_reason = conflict
        .as_ref()
        .map(|name| format!("near-duplicate normalized-name conflict with existing label {name}"))
        .or(validation.decision_reason);
    let proposal = with_immediate_tx(&conn, || {
        let proposal = insert_proposal_in_tx(
            &conn,
            &task.board_id,
            &task.id,
            actor,
            status.clone(),
            &candidate,
            &suggestions,
            top1_existing_label_id,
            top1_existing_label_name,
            &diagnostics,
            decision_reason,
            now,
        )?;
        insert_event(
            &conn,
            &task.board_id,
            Some(&task.id),
            None,
            if proposal.status == LabelProposalStatus::Rejected {
                "task.label_proposal.rejected"
            } else {
                "task.label_proposal.proposed"
            },
            actor,
            &json!({ "proposal_id": proposal.id, "name": proposal.name, "status": proposal.status }).to_string(),
            now,
        )?;
        Ok(proposal)
    })?;
    Ok(LabelProposalAttempt {
        task_id: task.id,
        board_id: task.board_id,
        proposal: Some(proposal),
        degraded: conflict.is_some() || validation.degraded || suggestions.degraded,
        diagnostics,
        heuristic_coverage: suggestions.coverage,
        heuristic_coverage_cosine: suggestions.coverage_cosine,
        heuristic_residual_norm: suggestions.residual_norm,
        top1_existing_label_id: top1_existing_label_id.map(ToOwned::to_owned),
        top1_existing_label_name: top1_existing_label_name.map(ToOwned::to_owned),
    })
}

struct ResidualValidation {
    status: LabelProposalStatus,
    degraded: bool,
    diagnostics: Vec<String>,
    decision_reason: Option<String>,
    top1_existing_label_id: Option<String>,
    top1_existing_label_name: Option<String>,
}

impl ResidualValidation {
    fn not_run() -> Self {
        Self {
            status: LabelProposalStatus::Proposed,
            degraded: false,
            diagnostics: Vec::new(),
            decision_reason: None,
            top1_existing_label_id: None,
            top1_existing_label_name: None,
        }
    }

    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            status: LabelProposalStatus::Proposed,
            degraded: true,
            diagnostics: vec![
                "label_proposal_residual_validation_unavailable".to_owned(),
                message.into(),
            ],
            decision_reason: None,
            top1_existing_label_id: None,
            top1_existing_label_name: None,
        }
    }
}

fn validate_candidate_residual(
    store: &(impl LabelAtomVectorStore + ?Sized),
    board_id: &str,
    embedding_model: &str,
    query_vector: &[f32],
    residual: &[f32],
    candidate: &LabelProposalCandidate,
) -> ResidualValidation {
    // Keep this validation aligned with the suggestion solver inputs: positive
    // atoms are scored against the residual, while negative atoms suppress from
    // the original query. That makes persisted rejected proposals comparable to
    // the top existing label returned by residual suggestions.
    if residual.is_empty() || l2_norm(residual) == 0.0 {
        return ResidualValidation::unavailable("label_proposal_residual_vector_unavailable");
    }

    let candidate_score = match candidate_residual_score(store, query_vector, residual, candidate) {
        Ok(score) => score,
        Err(error) => {
            return ResidualValidation::unavailable(bounded_diagnostic_message(&error));
        }
    };
    let existing =
        match best_existing_label_score(store, board_id, embedding_model, query_vector, residual) {
            Ok(existing) => existing,
            Err(error) => {
                return ResidualValidation::unavailable(bounded_diagnostic_message(&error));
            }
        };
    let existing_score = existing
        .as_ref()
        .map(|score| score.score)
        .unwrap_or(0.0_f32);
    let mut diagnostics = Vec::new();
    if existing_score >= candidate_score {
        diagnostics.push("label_proposal_residual_top1_failed".to_owned());
        return ResidualValidation {
            status: LabelProposalStatus::Rejected,
            degraded: false,
            diagnostics,
            decision_reason: Some(format!(
                "candidate residual score {candidate_score:.3} did not beat existing label score {existing_score:.3}"
            )),
            top1_existing_label_id: existing.as_ref().map(|score| score.label_id.clone()),
            top1_existing_label_name: existing.as_ref().map(|score| score.label_name.clone()),
        };
    }
    if candidate_score - existing_score < PROPOSAL_RESIDUAL_MARGIN {
        diagnostics.push("label_proposal_residual_margin_insufficient".to_owned());
        return ResidualValidation {
            status: LabelProposalStatus::Rejected,
            degraded: false,
            diagnostics,
            decision_reason: Some(format!(
                "candidate residual score margin {:.3} is below required {PROPOSAL_RESIDUAL_MARGIN:.3}",
                candidate_score - existing_score
            )),
            top1_existing_label_id: existing.as_ref().map(|score| score.label_id.clone()),
            top1_existing_label_name: existing.as_ref().map(|score| score.label_name.clone()),
        };
    }
    diagnostics.push("label_proposal_residual_top1_verified".to_owned());
    ResidualValidation {
        status: LabelProposalStatus::Proposed,
        degraded: false,
        diagnostics,
        decision_reason: None,
        top1_existing_label_id: existing.as_ref().map(|score| score.label_id.clone()),
        top1_existing_label_name: existing.map(|score| score.label_name),
    }
}

fn candidate_residual_score(
    store: &(impl LabelAtomVectorStore + ?Sized),
    query_vector: &[f32],
    residual: &[f32],
    candidate: &LabelProposalCandidate,
) -> std::result::Result<f32, kanban_vector::VectorError> {
    let definition = LabelDefinition {
        id: "proposal_candidate".to_owned(),
        name: candidate.name.clone(),
        description: candidate.description.clone(),
        applies_when: candidate.applies_when.clone(),
        positive_examples: candidate.positive_examples.clone(),
        excludes_when: candidate.excludes_when.clone(),
        negative_examples: candidate.negative_examples.clone(),
    };
    let mut positive_score = 0.0_f32;
    let mut negative_score = 0.0_f32;
    for source in definition.atom_sources() {
        let vector = store.embed_query_text(&source.text)?;
        match source.polarity {
            LabelAtomPolarity::Positive => {
                positive_score = positive_score.max(cosine_similarity(residual, &vector).max(0.0))
            }
            LabelAtomPolarity::Negative => {
                negative_score =
                    negative_score.max(cosine_similarity(query_vector, &vector).max(0.0))
            }
        }
    }
    Ok(apply_negative_suppression(positive_score, negative_score))
}

#[derive(Debug, Clone)]
struct ExistingLabelScore {
    label_id: String,
    label_name: String,
    score: f32,
}

fn best_existing_label_score(
    store: &(impl LabelAtomVectorStore + ?Sized),
    board_id: &str,
    embedding_model: &str,
    query_vector: &[f32],
    residual: &[f32],
) -> Result<Option<ExistingLabelScore>> {
    let positive_hits = retrieve_residual_atoms(
        store,
        residual,
        LabelAtomPolarity::Positive,
        16,
        board_id,
        embedding_model,
    )
    .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;
    let negative_hits = retrieve_residual_atoms(
        store,
        query_vector,
        LabelAtomPolarity::Negative,
        16,
        board_id,
        embedding_model,
    )
    .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;

    let mut scores: HashMap<String, ExistingLabelScore> = HashMap::new();
    for hit in positive_hits {
        let hit_score = cosine_similarity(residual, &hit.vector).max(0.0);
        scores
            .entry(hit.label_id.clone())
            .and_modify(|score| score.score = score.score.max(hit_score))
            .or_insert(ExistingLabelScore {
                label_id: hit.label_id,
                label_name: hit.label_name,
                score: hit_score,
            });
    }
    let mut negative_scores: HashMap<String, f32> = HashMap::new();
    for hit in negative_hits {
        let hit_score = cosine_similarity(query_vector, &hit.vector).max(0.0);
        negative_scores
            .entry(hit.label_id)
            .and_modify(|score| *score = score.max(hit_score))
            .or_insert(hit_score);
    }
    for (label_id, negative_score) in negative_scores {
        if let Some(score) = scores.get_mut(&label_id) {
            score.score = apply_negative_suppression(score.score, negative_score);
        }
    }
    Ok(scores.into_values().max_by(|left, right| {
        left.score
            .total_cmp(&right.score)
            .then_with(|| right.label_name.cmp(&left.label_name))
    }))
}

fn apply_negative_suppression(positive_score: f32, negative_score: f32) -> f32 {
    if negative_score >= NEGATIVE_SUPPRESSION_THRESHOLD {
        (positive_score - negative_score * NEGATIVE_SUPPRESSION_FACTOR).max(0.0)
    } else {
        positive_score
    }
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let dot = left
        .iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum::<f32>();
    let left_norm = l2_norm(left);
    let right_norm = l2_norm(right);
    if left_norm == 0.0 || right_norm == 0.0 || left.len() != right.len() {
        0.0
    } else {
        dot / (left_norm * right_norm)
    }
}

fn l2_norm(values: &[f32]) -> f32 {
    values.iter().map(|value| value * value).sum::<f32>().sqrt()
}

pub fn list_label_proposals(
    path: impl AsRef<Path>,
    board: &str,
    options: LabelProposalListOptions,
) -> Result<Vec<LabelSemanticProposalRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task_id = match options.task_ref {
        Some(task_ref) => Some(resolve_task(&conn, &board_id, &task_ref)?.id),
        None => None,
    };
    let mut filter = SqlFilter::new();
    filter.and("board_id=?", board_id)?;
    if let Some(task_id) = task_id {
        filter.and("task_id=?", task_id)?;
    }
    if let Some(status) = options.status {
        filter.and("status=?", status.to_string())?;
    }
    let sql = format!(
        "SELECT id,board_id,task_id,status,name,description,applies_when,excludes_when,positive_examples,negative_examples,heuristic_coverage,heuristic_coverage_cosine,heuristic_residual_norm,top1_existing_label_id,top1_existing_label_name,diagnostics_json,created_by,decision_reason,resolved_label_id,created_at,updated_at,decided_at \
         FROM label_semantic_proposals {} ORDER BY created_at DESC, id ASC",
        filter.where_sql()
    );
    all_values(&conn, &sql, filter.params(), proposal_from_row)
}

pub fn get_label_proposal(
    path: impl AsRef<Path>,
    proposal_id: &str,
) -> Result<LabelSemanticProposalRecord> {
    let conn = connect_file(path.as_ref())?;
    get_label_proposal_conn(&conn, proposal_id)
}

pub fn accept_label_proposal(
    path: impl AsRef<Path>,
    actor: &str,
    proposal_id: &str,
    reason: Option<String>,
) -> Result<LabelSemanticProposalRecord> {
    decide_label_proposal(
        path,
        actor,
        proposal_id,
        LabelProposalStatus::Accepted,
        reason,
    )
}

pub fn reject_label_proposal(
    path: impl AsRef<Path>,
    actor: &str,
    proposal_id: &str,
    reason: Option<String>,
) -> Result<LabelSemanticProposalRecord> {
    decide_label_proposal(
        path,
        actor,
        proposal_id,
        LabelProposalStatus::Rejected,
        reason,
    )
}

fn decide_label_proposal(
    path: impl AsRef<Path>,
    actor: &str,
    proposal_id: &str,
    decision: LabelProposalStatus,
    reason: Option<String>,
) -> Result<LabelSemanticProposalRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let proposal = get_label_proposal_conn(&conn, proposal_id)?;
        if proposal.status != LabelProposalStatus::Proposed {
            return Err(KanbanError::InvalidInput(format!(
                "label proposal {proposal_id} is already {}",
                proposal.status
            )));
        }
        let mut resolved_label_id = proposal.resolved_label_id.clone();
        if decision == LabelProposalStatus::Accepted {
            if let Some(existing) =
                normalized_label_conflict(&conn, &proposal.board_id, &proposal.name)?
            {
                return Err(KanbanError::InvalidInput(format!(
                    "label proposal conflicts with existing label {existing}"
                )));
            }
            let label_id = new_label_id();
            exec(
                &conn,
                "INSERT INTO labels(id, board_id, name, color, created_at, updated_at) VALUES (?1, ?2, ?3, NULL, ?4, ?4)",
                (&label_id, &proposal.board_id, &proposal.name, now),
            )?;
            upsert_label_semantics_candidate_in_tx(
                &conn,
                &proposal.board_id,
                &label_id,
                &proposal.name,
                &proposal.candidate(),
                now,
            )?;
            mark_label_atom_store_dirty(&conn, &proposal.board_id, now)?;
            resolved_label_id = Some(label_id);
        }
        let decision_status = decision.to_string();
        let decision_reason = normalize_optional(reason);
        exec(
            &conn,
            "UPDATE label_semantic_proposals
             SET status=:status,
                 decision_reason=:decision_reason,
                 resolved_label_id=:resolved_label_id,
                 updated_at=:now,
                 decided_at=:now
             WHERE id=:proposal_id",
            named_params! {
                ":status": decision_status,
                ":decision_reason": decision_reason,
                ":resolved_label_id": resolved_label_id,
                ":now": now,
                ":proposal_id": proposal_id,
            },
        )?;
        insert_event(
            &conn,
            &proposal.board_id,
            Some(&proposal.task_id),
            None,
            if decision == LabelProposalStatus::Accepted {
                "task.label_proposal.accepted"
            } else {
                "task.label_proposal.rejected"
            },
            actor,
            &json!({ "proposal_id": proposal_id, "name": proposal.name, "status": decision })
                .to_string(),
            now,
        )?;
        get_label_proposal_conn(&conn, proposal_id)
    })
}

impl LabelSemanticProposalRecord {
    fn candidate(&self) -> LabelProposalCandidate {
        LabelProposalCandidate {
            name: self.name.clone(),
            description: self.description.clone(),
            applies_when: self.applies_when.clone(),
            excludes_when: self.excludes_when.clone(),
            positive_examples: self.positive_examples.clone(),
            negative_examples: self.negative_examples.clone(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn insert_proposal_in_tx(
    conn: &Connection,
    board_id: &str,
    task_id: &str,
    actor: &str,
    status: LabelProposalStatus,
    candidate: &LabelProposalCandidate,
    suggestions: &LabelSuggestionResult,
    top1_existing_label_id: Option<&str>,
    top1_existing_label_name: Option<&str>,
    diagnostics: &[String],
    decision_reason: Option<String>,
    now: i64,
) -> Result<LabelSemanticProposalRecord> {
    let id = new_typed_id("lp");
    let status_text = status.to_string();
    let applies_when = json_array(&candidate.applies_when)?;
    let excludes_when = json_array(&candidate.excludes_when)?;
    let positive_examples = json_array(&candidate.positive_examples)?;
    let negative_examples = json_array(&candidate.negative_examples)?;
    let diagnostics_json = json_array(diagnostics)?;
    let decided_at = if status == LabelProposalStatus::Rejected {
        Some(now)
    } else {
        None
    };
    exec(
        conn,
        "INSERT INTO label_semantic_proposals(
             id, board_id, task_id, status, name, description, applies_when, excludes_when,
             positive_examples, negative_examples, heuristic_coverage, heuristic_coverage_cosine,
             heuristic_residual_norm, top1_existing_label_id, top1_existing_label_name,
             diagnostics_json, created_by, decision_reason, resolved_label_id, created_at,
             updated_at, decided_at
         )
         VALUES (
             :id, :board_id, :task_id, :status, :name, :description, :applies_when,
             :excludes_when, :positive_examples, :negative_examples, :heuristic_coverage,
             :heuristic_coverage_cosine, :heuristic_residual_norm, :top1_existing_label_id,
             :top1_existing_label_name, :diagnostics_json, :created_by, :decision_reason,
             NULL, :now, :now, :decided_at
         )",
        named_params! {
            ":id": id,
            ":board_id": board_id,
            ":task_id": task_id,
            ":status": status_text,
            ":name": candidate.name,
            ":description": candidate.description,
            ":applies_when": applies_when,
            ":excludes_when": excludes_when,
            ":positive_examples": positive_examples,
            ":negative_examples": negative_examples,
            ":heuristic_coverage": suggestions.coverage,
            ":heuristic_coverage_cosine": suggestions.coverage_cosine,
            ":heuristic_residual_norm": suggestions.residual_norm,
            ":top1_existing_label_id": top1_existing_label_id,
            ":top1_existing_label_name": top1_existing_label_name,
            ":diagnostics_json": diagnostics_json,
            ":created_by": actor,
            ":decision_reason": decision_reason,
            ":now": now,
            ":decided_at": decided_at,
        },
    )?;
    get_label_proposal_conn(conn, &id)
}

fn get_label_proposal_conn(
    conn: &Connection,
    proposal_id: &str,
) -> Result<LabelSemanticProposalRecord> {
    required_row(
        conn,
        "SELECT id,board_id,task_id,status,name,description,applies_when,excludes_when,positive_examples,negative_examples,heuristic_coverage,heuristic_coverage_cosine,heuristic_residual_norm,top1_existing_label_id,top1_existing_label_name,diagnostics_json,created_by,decision_reason,resolved_label_id,created_at,updated_at,decided_at FROM label_semantic_proposals WHERE id=?1",
        [proposal_id],
        proposal_from_row,
        || KanbanError::NotFound(format!("label proposal {proposal_id}")),
    )
}

fn proposal_from_row(row: &Row<'_>) -> rusqlite::Result<LabelSemanticProposalRecord> {
    let status: String = row.get(3)?;
    Ok(LabelSemanticProposalRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        task_id: row.get(2)?,
        status: LabelProposalStatus::from_str(&status)
            .map_err(|err| rusqlite::Error::InvalidParameterName(err.to_string()))?,
        name: row.get(4)?,
        description: row.get(5)?,
        applies_when: json_vec(row.get(6)?)
            .map_err(|err| rusqlite::Error::InvalidParameterName(err.to_string()))?,
        excludes_when: json_vec(row.get(7)?)
            .map_err(|err| rusqlite::Error::InvalidParameterName(err.to_string()))?,
        positive_examples: json_vec(row.get(8)?)
            .map_err(|err| rusqlite::Error::InvalidParameterName(err.to_string()))?,
        negative_examples: json_vec(row.get(9)?)
            .map_err(|err| rusqlite::Error::InvalidParameterName(err.to_string()))?,
        heuristic_coverage: row.get(10)?,
        heuristic_coverage_cosine: row.get(11)?,
        heuristic_residual_norm: row.get(12)?,
        top1_existing_label_id: row.get(13)?,
        top1_existing_label_name: row.get(14)?,
        diagnostics: json_vec(row.get(15)?)
            .map_err(|err| rusqlite::Error::InvalidParameterName(err.to_string()))?,
        created_by: row.get(16)?,
        decision_reason: row.get(17)?,
        resolved_label_id: row.get(18)?,
        created_at: row.get(19)?,
        updated_at: row.get(20)?,
        decided_at: row.get(21)?,
    })
}

fn normalize_candidate(candidate: LabelProposalCandidate) -> Result<LabelProposalCandidate> {
    let name = candidate.name.trim().to_owned();
    if name.is_empty() {
        return Err(KanbanError::InvalidInput(
            "label proposal name is required".into(),
        ));
    }
    let description = normalize_optional(candidate.description);
    let applies_when = normalize_text_list(candidate.applies_when);
    let excludes_when = normalize_text_list(candidate.excludes_when);
    let positive_examples = normalize_text_list(candidate.positive_examples);
    let negative_examples = normalize_text_list(candidate.negative_examples);
    if description.is_none()
        && applies_when.is_empty()
        && excludes_when.is_empty()
        && positive_examples.is_empty()
        && negative_examples.is_empty()
    {
        return Err(KanbanError::InvalidInput(
            "label proposal semantics are required".into(),
        ));
    }
    Ok(LabelProposalCandidate {
        name,
        description,
        applies_when,
        excludes_when,
        positive_examples,
        negative_examples,
    })
}

fn normalized_label_conflict(
    conn: &Connection,
    board_id: &str,
    candidate_name: &str,
) -> Result<Option<String>> {
    let normalized_candidate = normalize_label_identity(candidate_name);
    let labels = all(
        conn,
        "SELECT name FROM labels WHERE board_id=?1 ORDER BY name ASC",
        [board_id],
        |row| row.get::<_, String>(0),
    )?;
    Ok(labels
        .into_iter()
        .find(|name| normalize_label_identity(name) == normalized_candidate))
}

fn normalize_label_identity(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn normalize_optional(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

fn normalize_text_list(items: Vec<String>) -> Vec<String> {
    items
        .into_iter()
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
        .collect()
}

fn json_array(items: &[String]) -> Result<String> {
    serde_json::to_string(items).map_err(|err| KanbanError::InvalidInput(err.to_string()))
}

fn json_vec(json: String) -> Result<Vec<String>> {
    serde_json::from_str(&json).map_err(|err| KanbanError::Storage(err.to_string()))
}
