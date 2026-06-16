use crate::connect_file;

use super::{
    LabelSuggestionCandidate, LabelSuggestionEvidenceAtom, LabelSuggestionOptions,
    LabelSuggestionResult, SelectedLabelSuggestion, board_id, derived_status_by_name,
    get_task_by_id, resolve_task, storage,
};

use std::path::Path;

use kanban_core::{KanbanError, Result};
use kanban_indexer::LANCEDB_LABEL_ATOMS_STORE;
use kanban_labels::{
    LabelAtomKind, LabelAtomPolarity, LabelSolverConfig, LabelSolverError, RetrievedLabelAtom,
    resolve_label_groups_by_residual,
};
use kanban_vector::{
    DisabledVectorStore, LabelAtomVectorHit, LabelAtomVectorQuery, VectorError, VectorStore,
    VectorStoreStatus,
};
use rusqlite::{Connection, OptionalExtension, params};

const NEW_LABEL_COVERAGE_THRESHOLD: f32 = 0.55;

pub fn suggest_task_labels(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: &str,
    options: LabelSuggestionOptions,
) -> Result<LabelSuggestionResult> {
    let store = DisabledVectorStore;
    suggest_task_labels_with(path, board, task_ref, &store, options)
}

pub fn suggest_task_labels_with(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: &str,
    store: &(impl VectorStore + ?Sized),
    options: LabelSuggestionOptions,
) -> Result<LabelSuggestionResult> {
    Ok(compute_task_label_suggestions_with(path, board, task_ref, store, options)?.result)
}

pub(crate) struct LabelSuggestionComputation {
    pub result: LabelSuggestionResult,
    pub query_vector: Vec<f32>,
    pub residual_vector: Vec<f32>,
}

pub(crate) fn compute_task_label_suggestions_with(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: &str,
    store: &(impl VectorStore + ?Sized),
    options: LabelSuggestionOptions,
) -> Result<LabelSuggestionComputation> {
    validate_options(options)?;
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    let task = get_task_by_id(&conn, &task.board_id, &task.id)?;
    let mut diagnostics = Vec::new();

    let status = store.status();
    if !status.enabled {
        diagnostics.push("vector_store_disabled".to_owned());
        return Ok(empty_computation(
            task.id,
            task.board_id,
            diagnostics,
            true,
            false,
        ));
    }
    push_label_atom_index_diagnostics(&conn, &task.board_id, &status, &mut diagnostics)?;

    let query_text = task_query_text(&task.title, task.description.as_deref());
    let query_vector = match store.embed_query_text(&query_text) {
        Ok(vector) => vector,
        Err(error) => {
            diagnostics.push("vector_query_error".to_owned());
            diagnostics.push(bounded_diagnostic_message(&error));
            return Ok(empty_computation(
                task.id,
                task.board_id,
                diagnostics,
                true,
                false,
            ));
        }
    };

    let already_applied = task
        .labels
        .iter()
        .map(|label| label.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let solver_config = LabelSolverConfig {
        max_candidates: options.limit.max(1),
        max_selected_labels: options.limit.max(1),
        min_candidate_score: options.min_score,
        ..LabelSolverConfig::default()
    };
    let board_id_for_query = task.board_id.clone();
    let embedding_model = store.chunk_embedding_model().to_owned();
    let solver_result = match resolve_label_groups_by_residual(
        &query_vector,
        &solver_config,
        |residual, polarity, limit| {
            retrieve_residual_atoms(
                store,
                residual,
                polarity,
                limit.min(options.atom_limit).max(1),
                &board_id_for_query,
                &embedding_model,
            )
        },
    ) {
        Ok(result) => result,
        Err(error) => {
            diagnostics.push("vector_query_error".to_owned());
            diagnostics.push(bounded_diagnostic_message(&error));
            return Ok(empty_computation(
                task.id,
                task.board_id,
                diagnostics,
                true,
                false,
            ));
        }
    };
    if solver_result.candidates.is_empty() {
        diagnostics.push("label_atom_index_empty".to_owned());
    }

    let mut candidates = solver_result
        .candidates
        .iter()
        .map(|candidate| LabelSuggestionCandidate {
            label_id: candidate.label_id.clone(),
            label_name: candidate.label_name.clone(),
            score: candidate.score,
            weight: candidate.score,
            already_applied: already_applied.contains(candidate.label_id.as_str()),
            evidence_atoms: candidate
                .evidence_atoms
                .iter()
                .map(map_solver_evidence)
                .collect(),
            negative_evidence_atoms: candidate
                .negative_evidence_atoms
                .iter()
                .map(map_solver_evidence)
                .collect(),
        })
        .collect::<Vec<_>>();
    candidates.truncate(options.limit);

    let selected_labels = solver_result
        .selected_labels
        .iter()
        .map(|selected| SelectedLabelSuggestion {
            label_id: selected.label_id.clone(),
            label_name: selected.label_name.clone(),
            score: selected.score,
            weight: selected.weight,
            already_applied: already_applied.contains(selected.label_id.as_str()),
            evidence_atoms: selected
                .evidence_atoms
                .iter()
                .map(map_solver_evidence)
                .collect(),
            negative_evidence_atoms: selected
                .negative_evidence_atoms
                .iter()
                .map(map_solver_evidence)
                .collect(),
        })
        .collect::<Vec<_>>();
    let coverage = solver_result.coverage;
    let degraded = !diagnostics.is_empty();
    let needs_new_label = selected_labels.is_empty()
        || coverage < NEW_LABEL_COVERAGE_THRESHOLD
        || solver_result.needs_new_label;
    Ok(LabelSuggestionComputation {
        result: LabelSuggestionResult {
            task_id: task.id,
            board_id: task.board_id,
            selected_labels,
            candidates,
            coverage,
            residual_norm: solver_result.residual_norm,
            needs_new_label,
            degraded,
            diagnostics,
        },
        query_vector,
        residual_vector: solver_result.residual,
    })
}

fn validate_options(options: LabelSuggestionOptions) -> Result<()> {
    if options.limit == 0 {
        return Err(KanbanError::InvalidInput("limit must be >= 1".to_owned()));
    }
    if options.atom_limit == 0 {
        return Err(KanbanError::InvalidInput(
            "atom_limit must be >= 1".to_owned(),
        ));
    }
    if !(0.0..=1.0).contains(&options.min_score) {
        return Err(KanbanError::InvalidInput(
            "min_score must be between 0 and 1".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn retrieve_residual_atoms(
    store: &(impl VectorStore + ?Sized),
    residual: &[f32],
    polarity: LabelAtomPolarity,
    limit: usize,
    board_id: &str,
    embedding_model: &str,
) -> std::result::Result<Vec<RetrievedLabelAtom>, LabelSolverError> {
    let polarity_text = match polarity {
        LabelAtomPolarity::Positive => "positive",
        LabelAtomPolarity::Negative => "negative",
    };
    let hits = store
        .query_label_atoms_by_vector(&LabelAtomVectorQuery {
            vector: residual.to_vec(),
            limit,
            board_id: Some(board_id.to_owned()),
            embedding_model: Some(embedding_model.to_owned()),
            polarity: Some(polarity_text.to_owned()),
            include_vector: true,
        })
        .map_err(solver_vector_error)?;
    hits.into_iter()
        .map(|hit| retrieved_atom_from_hit(hit, polarity))
        .collect()
}

fn retrieved_atom_from_hit(
    hit: LabelAtomVectorHit,
    expected_polarity: LabelAtomPolarity,
) -> std::result::Result<RetrievedLabelAtom, LabelSolverError> {
    let actual_polarity = polarity_from_str(&hit.hit.polarity)?;
    if actual_polarity != expected_polarity {
        return Err(LabelSolverError::RetrievedPolarityMismatch {
            expected: expected_polarity,
            actual: actual_polarity,
        });
    }
    let vector = hit
        .vector
        .ok_or_else(|| LabelSolverError::InvalidConfig("label atom vector missing".to_owned()))?;
    Ok(RetrievedLabelAtom {
        atom_id: hit.hit.atom_id,
        label_id: hit.hit.label_id,
        label_name: hit.hit.label_name,
        polarity: actual_polarity,
        kind: kind_from_str(&hit.hit.kind)?,
        text: hit.hit.text,
        vector,
        score: distance_to_similarity(hit.hit.score),
    })
}

fn solver_vector_error(error: VectorError) -> LabelSolverError {
    LabelSolverError::InvalidConfig(error.to_string())
}

fn polarity_from_str(value: &str) -> std::result::Result<LabelAtomPolarity, LabelSolverError> {
    match value {
        "positive" => Ok(LabelAtomPolarity::Positive),
        "negative" => Ok(LabelAtomPolarity::Negative),
        other => Err(LabelSolverError::InvalidConfig(format!(
            "unknown label atom polarity {other}"
        ))),
    }
}

fn kind_from_str(value: &str) -> std::result::Result<LabelAtomKind, LabelSolverError> {
    match value {
        "name" => Ok(LabelAtomKind::Name),
        "description" => Ok(LabelAtomKind::Description),
        "applies_when" => Ok(LabelAtomKind::AppliesWhen),
        "positive_example" => Ok(LabelAtomKind::PositiveExample),
        "excludes_when" => Ok(LabelAtomKind::ExcludesWhen),
        "negative_example" => Ok(LabelAtomKind::NegativeExample),
        other => Err(LabelSolverError::InvalidConfig(format!(
            "unknown label atom kind {other}"
        ))),
    }
}

fn map_solver_evidence(evidence: &kanban_labels::LabelAtomEvidence) -> LabelSuggestionEvidenceAtom {
    LabelSuggestionEvidenceAtom {
        atom_id: evidence.atom_id.clone().unwrap_or_default(),
        label_id: evidence.source.label_id.clone(),
        label_name: evidence.source.label_name.clone(),
        polarity: match evidence.source.polarity {
            LabelAtomPolarity::Positive => "positive".to_owned(),
            LabelAtomPolarity::Negative => "negative".to_owned(),
        },
        kind: match evidence.source.kind {
            LabelAtomKind::Name => "name",
            LabelAtomKind::Description => "description",
            LabelAtomKind::AppliesWhen => "applies_when",
            LabelAtomKind::PositiveExample => "positive_example",
            LabelAtomKind::ExcludesWhen => "excludes_when",
            LabelAtomKind::NegativeExample => "negative_example",
        }
        .to_owned(),
        text: evidence.source.text.clone(),
        score: evidence.similarity,
    }
}

fn push_label_atom_index_diagnostics(
    conn: &Connection,
    board_id: &str,
    status: &VectorStoreStatus,
    diagnostics: &mut Vec<String>,
) -> Result<()> {
    if status.message.contains("dirty=true") || status.message.contains("board_dirty=true") {
        diagnostics.push("label_atom_index_dirty".to_owned());
    }
    if let Some(error) = derived_status_by_name(conn, LANCEDB_LABEL_ATOMS_STORE)?.last_error {
        diagnostics.push("label_atom_index_error".to_owned());
        diagnostics.push(bounded_diagnostic_message(&error));
    }
    let board_error = conn
        .query_row(
            "SELECT last_error FROM label_atom_index_boards WHERE store_name=?1 AND board_id=?2",
            params![LANCEDB_LABEL_ATOMS_STORE, board_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(storage)?
        .flatten();
    if let Some(error) = board_error {
        diagnostics.push("label_atom_index_error".to_owned());
        diagnostics.push(bounded_diagnostic_message(&error));
    }
    diagnostics.sort();
    diagnostics.dedup();
    Ok(())
}

fn distance_to_similarity(distance: f32) -> f32 {
    1.0 / (1.0 + distance.max(0.0))
}

fn task_query_text(title: &str, description: Option<&str>) -> String {
    match description.map(str::trim).filter(|value| !value.is_empty()) {
        Some(description) => format!("{}\n\n{}", title.trim(), description),
        None => title.trim().to_owned(),
    }
}

fn empty_result(
    task_id: String,
    board_id: String,
    mut diagnostics: Vec<String>,
    degraded: bool,
    needs_new_label: bool,
) -> LabelSuggestionResult {
    diagnostics.sort();
    diagnostics.dedup();
    LabelSuggestionResult {
        task_id,
        board_id,
        selected_labels: Vec::new(),
        candidates: Vec::new(),
        coverage: 0.0,
        residual_norm: 1.0,
        needs_new_label,
        degraded,
        diagnostics,
    }
}

fn empty_computation(
    task_id: String,
    board_id: String,
    diagnostics: Vec<String>,
    degraded: bool,
    needs_new_label: bool,
) -> LabelSuggestionComputation {
    LabelSuggestionComputation {
        result: empty_result(task_id, board_id, diagnostics, degraded, needs_new_label),
        query_vector: Vec::new(),
        residual_vector: Vec::new(),
    }
}

pub(crate) fn bounded_diagnostic_message(error: &impl std::fmt::Display) -> String {
    let message = error.to_string();
    if message.len() > 240 {
        let boundary = message
            .char_indices()
            .map(|(index, _)| index)
            .take_while(|index| *index <= 240)
            .last()
            .unwrap_or(0);
        format!("{}...", &message[..boundary])
    } else {
        message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_diagnostic_message_truncates_non_ascii_on_char_boundary() {
        let message = format!("{}🙂tail", "界".repeat(79));

        let diagnostic = std::panic::catch_unwind(|| bounded_diagnostic_message(&message))
            .expect("diagnostic truncation should not panic on non-ASCII input");

        assert!(diagnostic.ends_with("..."));
        assert!(diagnostic.is_char_boundary(diagnostic.len()));
        assert!(diagnostic.len() <= 243);
    }
}
