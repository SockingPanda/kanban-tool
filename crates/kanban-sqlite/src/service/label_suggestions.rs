use crate::connect_file;

use super::{
    LabelSuggestionCandidate, LabelSuggestionEvidenceAtom, LabelSuggestionOptions,
    LabelSuggestionResult, SelectedLabelSuggestion, board_id, derived_status_by_name,
    get_task_by_id, resolve_task, storage,
};

use std::{collections::HashMap, path::Path};

use kanban_core::{KanbanError, Result};
use kanban_indexer::LANCEDB_LABEL_ATOMS_STORE;
use kanban_vector::{
    DisabledVectorStore, LabelAtomHit, LabelAtomQuery, VectorStore, VectorStoreStatus,
};
use rusqlite::{Connection, OptionalExtension, params};

const NEW_LABEL_COVERAGE_THRESHOLD: f32 = 0.55;
const NEGATIVE_EVIDENCE_PENALTY: f32 = 0.8;

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
    validate_options(options)?;
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    let task = get_task_by_id(&conn, &task.board_id, &task.id)?;
    let mut diagnostics = Vec::new();
    diagnostics.push("solver_refit_unavailable".to_owned());

    let status = store.status();
    if !status.enabled {
        diagnostics.push("vector_store_disabled".to_owned());
        return Ok(empty_result(
            task.id,
            task.board_id,
            diagnostics,
            true,
            false,
        ));
    }
    push_label_atom_index_diagnostics(&conn, &task.board_id, &status, &mut diagnostics)?;

    let query_text = task_query_text(&task.title, task.description.as_deref());
    let hits = match store.query_label_atoms(&LabelAtomQuery {
        text: query_text,
        limit: options.atom_limit,
        board_id: Some(task.board_id.clone()),
        embedding_model: Some(store.chunk_embedding_model().to_owned()),
        polarity: None,
    }) {
        Ok(hits) => hits,
        Err(error) => {
            diagnostics.push("vector_query_error".to_owned());
            diagnostics.push(bounded_diagnostic_message(&error));
            return Ok(empty_result(
                task.id,
                task.board_id,
                diagnostics,
                true,
                false,
            ));
        }
    };

    if hits.is_empty() {
        diagnostics.push("label_atom_index_empty".to_owned());
    }

    let already_applied = task
        .labels
        .iter()
        .map(|label| label.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut candidates = aggregate_hits(hits, &already_applied);
    candidates.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.label_name.cmp(&right.label_name))
    });
    if candidates.len() > options.limit {
        candidates.truncate(options.limit);
    }

    let selected_labels = candidates
        .iter()
        .filter(|candidate| candidate.score >= options.min_score)
        .map(|candidate| SelectedLabelSuggestion {
            label_id: candidate.label_id.clone(),
            label_name: candidate.label_name.clone(),
            score: candidate.score,
            weight: candidate.weight,
            already_applied: candidate.already_applied,
            evidence_atoms: candidate.evidence_atoms.clone(),
            negative_evidence_atoms: candidate.negative_evidence_atoms.clone(),
        })
        .collect::<Vec<_>>();
    let coverage = selected_labels
        .iter()
        .map(|candidate| candidate.score)
        .fold(0.0_f32, f32::max)
        .clamp(0.0, 1.0);
    let degraded = !diagnostics.is_empty();
    let needs_new_label = selected_labels.is_empty() || coverage < NEW_LABEL_COVERAGE_THRESHOLD;
    Ok(LabelSuggestionResult {
        task_id: task.id,
        board_id: task.board_id,
        selected_labels,
        candidates,
        coverage,
        residual_norm: (1.0 - coverage).clamp(0.0, 1.0),
        needs_new_label,
        degraded,
        diagnostics,
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

fn aggregate_hits(
    hits: Vec<LabelAtomHit>,
    already_applied: &std::collections::HashSet<&str>,
) -> Vec<LabelSuggestionCandidate> {
    let mut groups: HashMap<String, CandidateAccumulator> = HashMap::new();
    for hit in hits {
        let score = distance_to_similarity(hit.score);
        let group = groups.entry(hit.label_id.clone()).or_insert_with(|| {
            CandidateAccumulator::new(&hit, already_applied.contains(hit.label_id.as_str()))
        });
        let evidence = LabelSuggestionEvidenceAtom {
            atom_id: hit.atom_id,
            label_id: hit.label_id,
            label_name: hit.label_name,
            polarity: hit.polarity.clone(),
            kind: hit.kind,
            text: hit.text,
            score,
        };
        if hit.polarity == "negative" {
            group.negative_score = group.negative_score.max(score);
            group.negative_evidence_atoms.push(evidence);
        } else {
            group.positive_score = group.positive_score.max(score);
            group.evidence_atoms.push(evidence);
        }
    }
    groups
        .into_values()
        .map(CandidateAccumulator::into_candidate)
        .collect()
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

fn bounded_diagnostic_message(error: &impl std::fmt::Display) -> String {
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

struct CandidateAccumulator {
    label_id: String,
    label_name: String,
    already_applied: bool,
    positive_score: f32,
    negative_score: f32,
    evidence_atoms: Vec<LabelSuggestionEvidenceAtom>,
    negative_evidence_atoms: Vec<LabelSuggestionEvidenceAtom>,
}

impl CandidateAccumulator {
    fn new(hit: &LabelAtomHit, already_applied: bool) -> Self {
        Self {
            label_id: hit.label_id.clone(),
            label_name: hit.label_name.clone(),
            already_applied,
            positive_score: 0.0,
            negative_score: 0.0,
            evidence_atoms: Vec::new(),
            negative_evidence_atoms: Vec::new(),
        }
    }

    fn into_candidate(mut self) -> LabelSuggestionCandidate {
        self.evidence_atoms
            .sort_by(|left, right| right.score.total_cmp(&left.score));
        self.negative_evidence_atoms
            .sort_by(|left, right| right.score.total_cmp(&left.score));
        self.evidence_atoms.truncate(3);
        self.negative_evidence_atoms.truncate(3);
        let score = (self.positive_score - self.negative_score * NEGATIVE_EVIDENCE_PENALTY)
            .max(0.0)
            .clamp(0.0, 1.0);
        LabelSuggestionCandidate {
            label_id: self.label_id,
            label_name: self.label_name,
            score,
            weight: score,
            already_applied: self.already_applied,
            evidence_atoms: self.evidence_atoms,
            negative_evidence_atoms: self.negative_evidence_atoms,
        }
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
