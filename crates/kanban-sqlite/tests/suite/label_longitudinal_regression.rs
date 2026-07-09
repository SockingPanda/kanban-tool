use crate::common::*;

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
};

use kanban_sqlite::api::LabelSuggestionOptions;
use kanban_sqlite::api::provider::rebuild_label_atom_index_with;
use kanban_vector::{
    LabelAtomHit, LabelAtomVector, LabelAtomVectorHit, LabelAtomVectorQuery, LabelAtomVectorStore,
    VectorError, VectorStoreStatus,
};

#[test]
fn label_ontology_longitudinal_regression_corpus_tracks_selection_score_and_evidence()
-> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_longitudinal_regression_corpus")?;
    init_database(&temp.path, "tester")?;
    seed_longitudinal_labels(&temp)?;

    let backend_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Fix API route persistence failure"),
    )?;
    let docs_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Update CLI help manual"),
    )?;
    let visual_control_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Tune desktop visual layout CSS"),
    )?;

    let cases = vec![
        LongitudinalCase {
            id: "backend-positive",
            task_ref: backend_task.id.clone(),
            expected_selected: vec!["backend"],
            forbidden_selected: vec!["docs"],
            required_evidence: vec!["server handlers and SQLite persistence"],
            min_score: 0.80,
        },
        LongitudinalCase {
            id: "docs-positive",
            task_ref: docs_task.id.clone(),
            expected_selected: vec!["docs"],
            forbidden_selected: vec!["backend"],
            required_evidence: vec!["Documentation and operator guidance work"],
            min_score: 0.80,
        },
        LongitudinalCase {
            id: "visual-negative-control",
            task_ref: visual_control_task.id.clone(),
            expected_selected: Vec::new(),
            forbidden_selected: vec!["backend", "docs"],
            required_evidence: Vec::new(),
            min_score: 0.0,
        },
    ];

    let store = LongitudinalVectorStore::default();
    rebuild_label_atom_index_with(&temp.path, "default", &store)?;

    let before_counts = canonical_counts(&temp.path)?;
    let baseline = run_longitudinal_corpus(&temp.path, &store, &cases)?;
    let after_baseline_counts = canonical_counts(&temp.path)?;
    assert_eq!(
        after_baseline_counts, before_counts,
        "running the longitudinal corpus must be read-only for canonical ontology tables"
    );
    assert_corpus_matches_expectations(&cases, &baseline);

    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            applies_when: vec!["desktop visual layout CSS".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;
    rebuild_label_atom_index_with(&temp.path, "default", &store)?;
    let after_mutation = run_longitudinal_corpus(&temp.path, &store, &cases)?;

    let regressions = compare_longitudinal_runs(&cases, &baseline, &after_mutation);
    let visual_backend_regression = regressions.iter().find(|issue| {
        issue.case_id == "visual-negative-control"
            && issue.label == "backend"
            && issue.kind == "forbidden_label_selected"
    });
    assert!(
        visual_backend_regression.is_some(),
        "intentional broad backend atom should be detected as visual-control regression: {regressions:#?}"
    );
    let visual_backend_regression = visual_backend_regression.expect("checked above");
    assert_eq!(visual_backend_regression.before_score, None);
    assert!(
        visual_backend_regression
            .after_score
            .is_some_and(|score| score > 0.05),
        "regression should record the after score: {visual_backend_regression:#?}"
    );
    assert!(
        regressions.iter().any(|issue| issue
            .evidence
            .iter()
            .any(|text| text.contains("desktop visual layout CSS"))),
        "regression report should preserve the new evidence atom text: {regressions:#?}"
    );
    Ok(())
}

fn seed_longitudinal_labels(temp: &TempDb) -> anyhow::Result<()> {
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "docs".to_owned(),
            color: None,
        },
    )?;
    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("Backend service and SQLite persistence work".to_owned()),
            applies_when: vec!["server handlers and SQLite persistence".to_owned()],
            excludes_when: vec!["documentation-only or desktop visual layout work".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;
    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "docs".to_owned(),
            description: Some("Documentation and operator guidance work".to_owned()),
            applies_when: vec!["CLI help, README, or operator documentation".to_owned()],
            excludes_when: vec!["runtime persistence bug fixes".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;
    Ok(())
}

#[derive(Debug)]
struct LongitudinalCase {
    id: &'static str,
    task_ref: String,
    expected_selected: Vec<&'static str>,
    forbidden_selected: Vec<&'static str>,
    required_evidence: Vec<&'static str>,
    min_score: f32,
}

#[derive(Debug, Clone)]
struct LongitudinalSnapshot {
    selected: BTreeMap<String, LabelObservation>,
    candidates: BTreeMap<String, LabelObservation>,
}

#[derive(Debug, Clone)]
struct LabelObservation {
    score: f32,
    evidence: Vec<String>,
}

#[derive(Debug)]
struct LongitudinalRegression {
    case_id: &'static str,
    label: String,
    kind: &'static str,
    before_score: Option<f32>,
    after_score: Option<f32>,
    evidence: Vec<String>,
}

fn run_longitudinal_corpus(
    path: &std::path::Path,
    store: &LongitudinalVectorStore,
    cases: &[LongitudinalCase],
) -> anyhow::Result<BTreeMap<&'static str, LongitudinalSnapshot>> {
    let mut snapshots = BTreeMap::new();
    for case in cases {
        let suggestion = kanban_sqlite::api::provider::suggest_task_labels_with(
            path,
            "default",
            &case.task_ref,
            store,
            LabelSuggestionOptions {
                output_limit: 8,
                candidate_limit: 16,
                atom_limit: 32,
                max_selected_labels: 4,
                min_score: 0.05,
            },
        )?;
        snapshots.insert(
            case.id,
            LongitudinalSnapshot {
                selected: suggestion
                    .selected_labels
                    .iter()
                    .map(|label| {
                        (
                            label.label_name.clone(),
                            LabelObservation {
                                score: label.score,
                                evidence: label
                                    .evidence_atoms
                                    .iter()
                                    .map(|atom| atom.text.clone())
                                    .collect(),
                            },
                        )
                    })
                    .collect(),
                candidates: suggestion
                    .candidates
                    .iter()
                    .map(|label| {
                        (
                            label.label_name.clone(),
                            LabelObservation {
                                score: label.score,
                                evidence: label
                                    .evidence_atoms
                                    .iter()
                                    .map(|atom| atom.text.clone())
                                    .collect(),
                            },
                        )
                    })
                    .collect(),
            },
        );
    }
    Ok(snapshots)
}

fn assert_corpus_matches_expectations(
    cases: &[LongitudinalCase],
    snapshots: &BTreeMap<&'static str, LongitudinalSnapshot>,
) {
    for case in cases {
        let snapshot = snapshots.get(case.id).expect("missing case snapshot");
        for label in &case.expected_selected {
            let selected = snapshot
                .selected
                .get(*label)
                .unwrap_or_else(|| panic!("{} did not select expected label {label}", case.id));
            assert!(
                selected.score >= case.min_score,
                "{} expected label {label} score {} below {}",
                case.id,
                selected.score,
                case.min_score
            );
        }
        for label in &case.forbidden_selected {
            assert!(
                !snapshot.selected.contains_key(*label),
                "{} selected forbidden label {label}",
                case.id
            );
        }
        let evidence_texts = snapshot
            .selected
            .values()
            .flat_map(|observation| observation.evidence.iter())
            .collect::<BTreeSet<_>>();
        for required in &case.required_evidence {
            assert!(
                evidence_texts.iter().any(|text| text.contains(required)),
                "{} missing expected evidence text {required:?}: {evidence_texts:?}",
                case.id
            );
        }
    }
}

fn compare_longitudinal_runs(
    cases: &[LongitudinalCase],
    before: &BTreeMap<&'static str, LongitudinalSnapshot>,
    after: &BTreeMap<&'static str, LongitudinalSnapshot>,
) -> Vec<LongitudinalRegression> {
    let mut regressions = Vec::new();
    for case in cases {
        let before_snapshot = before.get(case.id).expect("missing before snapshot");
        let after_snapshot = after.get(case.id).expect("missing after snapshot");
        for label in &case.expected_selected {
            let before_selected = before_snapshot.selected.get(*label);
            let after_selected = after_snapshot.selected.get(*label);
            if before_selected.is_some() && after_selected.is_none() {
                regressions.push(LongitudinalRegression {
                    case_id: case.id,
                    label: (*label).to_owned(),
                    kind: "expected_label_lost",
                    before_score: before_selected.map(|item| item.score),
                    after_score: after_label_score(after_snapshot, label),
                    evidence: after_label_evidence(after_snapshot, label),
                });
            }
            if let (Some(before), Some(after)) = (before_selected, after_selected)
                && after.score + 0.05 < before.score
            {
                regressions.push(LongitudinalRegression {
                    case_id: case.id,
                    label: (*label).to_owned(),
                    kind: "expected_label_score_drop",
                    before_score: Some(before.score),
                    after_score: Some(after.score),
                    evidence: after.evidence.clone(),
                });
            }
        }
        for label in &case.forbidden_selected {
            if after_snapshot.selected.contains_key(*label) {
                regressions.push(LongitudinalRegression {
                    case_id: case.id,
                    label: (*label).to_owned(),
                    kind: "forbidden_label_selected",
                    before_score: after_label_score(before_snapshot, label),
                    after_score: after_label_score(after_snapshot, label),
                    evidence: after_label_evidence(after_snapshot, label),
                });
            }
        }
    }
    regressions
}

fn after_label_score(snapshot: &LongitudinalSnapshot, label: &str) -> Option<f32> {
    snapshot
        .selected
        .get(label)
        .or_else(|| snapshot.candidates.get(label))
        .map(|item| item.score)
}

fn after_label_evidence(snapshot: &LongitudinalSnapshot, label: &str) -> Vec<String> {
    snapshot
        .selected
        .get(label)
        .or_else(|| snapshot.candidates.get(label))
        .map(|item| item.evidence.clone())
        .unwrap_or_default()
}

fn canonical_counts(path: &std::path::Path) -> anyhow::Result<BTreeMap<&'static str, i64>> {
    let conn = connect_file(path)?;
    let mut counts = BTreeMap::new();
    for table in [
        "labels",
        "task_labels",
        "label_semantics",
        "label_atoms",
        "label_ontology_actions",
    ] {
        counts.insert(
            table,
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })?,
        );
    }
    Ok(counts)
}

#[derive(Default)]
struct LongitudinalVectorStore {
    live_atoms: Mutex<Vec<LabelAtomVector>>,
}

impl kanban_vector::VectorStoreBackend for LongitudinalVectorStore {
    fn embedding_model(&self) -> &str {
        "longitudinal-test-model"
    }

    fn status(&self) -> VectorStoreStatus {
        VectorStoreStatus::new(
            "longitudinal-test-vector",
            true,
            "longitudinal regression vector store; dirty=false last_error=none; board_dirty=false",
        )
    }
}

impl kanban_vector::QueryEmbeddingProvider for LongitudinalVectorStore {
    fn embed_query_text(&self, text: &str) -> Result<Vec<f32>, VectorError> {
        Ok(vector_for_text(text))
    }
}

impl LabelAtomVectorStore for LongitudinalVectorStore {
    fn delete_label_atoms_for_board(&self, board_id: &str) -> Result<(), VectorError> {
        self.live_atoms
            .lock()
            .map_err(|err| VectorError::Store(format!("live_atoms mutex poisoned: {err}")))?
            .retain(|atom| atom.board_id != board_id);
        Ok(())
    }

    fn upsert_label_atoms(&self, atoms: &[LabelAtomVector]) -> Result<(), VectorError> {
        let mut live = self
            .live_atoms
            .lock()
            .map_err(|err| VectorError::Store(format!("live_atoms mutex poisoned: {err}")))?;
        live.extend(atoms.iter().cloned());
        Ok(())
    }

    fn query_label_atoms_by_vector(
        &self,
        query: &LabelAtomVectorQuery,
    ) -> Result<Vec<LabelAtomVectorHit>, VectorError> {
        let mut hits = self
            .live_atoms
            .lock()
            .map_err(|err| VectorError::Store(format!("live_atoms mutex poisoned: {err}")))?
            .iter()
            .filter(|atom| {
                query
                    .board_id
                    .as_ref()
                    .is_none_or(|board_id| &atom.board_id == board_id)
                    && query
                        .embedding_model
                        .as_ref()
                        .is_none_or(|model| &atom.embedding_model == model)
                    && query
                        .polarity
                        .as_ref()
                        .is_none_or(|polarity| &atom.polarity == polarity)
            })
            .filter_map(|atom| {
                let vector = vector_for_text(&atom.text);
                let similarity = cosine_similarity(&query.vector, &vector);
                (similarity > 0.0).then_some((similarity, atom.clone(), vector))
            })
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| {
            right
                .0
                .total_cmp(&left.0)
                .then_with(|| left.1.label_name.cmp(&right.1.label_name))
                .then_with(|| left.1.atom_id.cmp(&right.1.atom_id))
        });
        Ok(hits
            .into_iter()
            .take(query.limit)
            .map(|(similarity, atom, vector)| LabelAtomVectorHit {
                hit: LabelAtomHit {
                    atom_id: atom.atom_id,
                    label_id: atom.label_id,
                    label_name: atom.label_name,
                    board_id: atom.board_id,
                    polarity: atom.polarity,
                    kind: atom.kind,
                    text: atom.text,
                    ordinal: atom.ordinal,
                    content_hash: atom.content_hash,
                    embedding_model: atom.embedding_model,
                    distance: (1.0 / similarity.max(0.0001)) - 1.0,
                },
                vector: query.include_vector.then_some(vector),
            })
            .collect())
    }
}

fn vector_for_text(text: &str) -> Vec<f32> {
    let lower = text.to_lowercase();
    if lower.contains("desktop")
        || lower.contains("visual")
        || lower.contains("layout")
        || lower.contains("css")
    {
        vec![0.0, 0.0, 1.0]
    } else if lower.contains("doc")
        || lower.contains("manual")
        || lower.contains("readme")
        || lower.contains("help")
    {
        vec![0.0, 1.0, 0.0]
    } else if lower.contains("api")
        || lower.contains("server")
        || lower.contains("sqlite")
        || lower.contains("persistence")
        || lower.contains("backend")
    {
        vec![1.0, 0.0, 0.0]
    } else {
        vec![0.0, 0.0, 0.0]
    }
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let dot = left
        .iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm * right_norm)
    }
}
