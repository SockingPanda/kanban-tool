//! 纯内存 semantic label solver。
//!
//! 本 crate 只负责把上层传入的 label 定义、atom embedding 和 query embedding
//! 解析成可解释的候选与多 label 选择结果。它不连接 SQLite，不替代
//! `labels` / `task_labels` 的 canonical 存储。

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelDefinition {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub applies_when: Vec<String>,
    pub positive_examples: Vec<String>,
    pub excludes_when: Vec<String>,
    pub negative_examples: Vec<String>,
}

impl LabelDefinition {
    pub fn atom_sources(&self) -> Vec<LabelAtomSource> {
        let mut sources = Vec::new();
        push_atom_source(
            &mut sources,
            self,
            LabelAtomPolarity::Positive,
            LabelAtomKind::Name,
            &self.name,
        );
        if let Some(description) = &self.description {
            push_atom_source(
                &mut sources,
                self,
                LabelAtomPolarity::Positive,
                LabelAtomKind::Description,
                description,
            );
        }
        for text in &self.applies_when {
            push_atom_source(
                &mut sources,
                self,
                LabelAtomPolarity::Positive,
                LabelAtomKind::AppliesWhen,
                text,
            );
        }
        for text in &self.positive_examples {
            push_atom_source(
                &mut sources,
                self,
                LabelAtomPolarity::Positive,
                LabelAtomKind::PositiveExample,
                text,
            );
        }
        for text in &self.excludes_when {
            push_atom_source(
                &mut sources,
                self,
                LabelAtomPolarity::Negative,
                LabelAtomKind::ExcludesWhen,
                text,
            );
        }
        for text in &self.negative_examples {
            push_atom_source(
                &mut sources,
                self,
                LabelAtomPolarity::Negative,
                LabelAtomKind::NegativeExample,
                text,
            );
        }
        sources
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelAtomPolarity {
    Positive,
    Negative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelAtomKind {
    Name,
    Description,
    AppliesWhen,
    PositiveExample,
    ExcludesWhen,
    NegativeExample,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelAtomSource {
    pub label_id: String,
    pub label_name: String,
    pub polarity: LabelAtomPolarity,
    pub kind: LabelAtomKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddedLabelAtom {
    pub source: LabelAtomSource,
    pub embedding: Vec<f32>,
}

impl EmbeddedLabelAtom {
    pub fn new(
        source: LabelAtomSource,
        embedding: Vec<f32>,
        expected_dimension: usize,
    ) -> Result<Self, LabelSolverError> {
        ensure_dimension(embedding.len(), expected_dimension)?;
        Ok(Self { source, embedding })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticLabel {
    pub definition: LabelDefinition,
    pub atoms: Vec<EmbeddedLabelAtom>,
    pub embedding_dimension: usize,
}

impl SemanticLabel {
    pub fn new(
        definition: LabelDefinition,
        atoms: Vec<EmbeddedLabelAtom>,
    ) -> Result<Self, LabelSolverError> {
        let first = atoms.first().ok_or(LabelSolverError::EmptyAtoms)?;
        let embedding_dimension = first.embedding.len();
        for atom in &atoms {
            if atom.source.label_id != definition.id {
                return Err(LabelSolverError::LabelMismatch {
                    expected: definition.id.clone(),
                    actual: atom.source.label_id.clone(),
                });
            }
            if atom.source.label_name != definition.name {
                return Err(LabelSolverError::LabelMismatch {
                    expected: definition.name.clone(),
                    actual: atom.source.label_name.clone(),
                });
            }
            ensure_dimension(atom.embedding.len(), embedding_dimension)?;
        }
        Ok(Self {
            definition,
            atoms,
            embedding_dimension,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomEvidence {
    pub source: LabelAtomSource,
    pub similarity: f32,
    pub contribution: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelGroupCandidate {
    pub label_id: String,
    pub label_name: String,
    pub score: f32,
    pub positive_score: f32,
    pub negative_score: f32,
    pub suppressed: bool,
    pub evidence_atoms: Vec<LabelAtomEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectedLabel {
    pub label_id: String,
    pub label_name: String,
    pub weight: f32,
    pub score: f32,
    pub evidence_atoms: Vec<LabelAtomEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelSolverConfig {
    pub max_candidates: usize,
    pub max_selected_labels: usize,
    pub min_candidate_score: f32,
    pub min_evidence_score: f32,
    pub negative_suppression_threshold: f32,
    pub negative_suppression_factor: f32,
    pub min_coverage: f32,
    pub max_residual_norm: f32,
    pub refit_iterations: usize,
}

impl Default for LabelSolverConfig {
    fn default() -> Self {
        Self {
            max_candidates: 8,
            max_selected_labels: 3,
            min_candidate_score: 0.05,
            min_evidence_score: 0.05,
            negative_suppression_threshold: 0.65,
            negative_suppression_factor: 0.8,
            min_coverage: 0.55,
            max_residual_norm: 0.75,
            refit_iterations: 40,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelSolverResult {
    pub candidates: Vec<LabelGroupCandidate>,
    pub selected_labels: Vec<SelectedLabel>,
    pub coverage: f32,
    pub residual_norm: f32,
    pub needs_new_label: bool,
}

pub fn retrieve_label_groups(
    query_embedding: &[f32],
    labels: &[SemanticLabel],
    config: &LabelSolverConfig,
) -> Result<Vec<LabelGroupCandidate>, LabelSolverError> {
    validate_query_dimension(query_embedding, labels)?;

    let mut candidates = labels
        .iter()
        .filter_map(|label| {
            let mut positive_score = 0.0_f32;
            let mut negative_score = 0.0_f32;
            let mut evidence_atoms = Vec::new();

            for atom in &label.atoms {
                let similarity = cosine_similarity(query_embedding, &atom.embedding);
                match atom.source.polarity {
                    LabelAtomPolarity::Positive => {
                        positive_score = positive_score.max(similarity);
                        if similarity >= config.min_evidence_score {
                            evidence_atoms.push(LabelAtomEvidence {
                                source: atom.source.clone(),
                                similarity,
                                contribution: similarity.max(0.0),
                            });
                        }
                    }
                    LabelAtomPolarity::Negative => {
                        negative_score = negative_score.max(similarity);
                    }
                }
            }

            evidence_atoms.sort_by(|left, right| {
                right
                    .contribution
                    .total_cmp(&left.contribution)
                    .then_with(|| left.source.text.cmp(&right.source.text))
            });

            let suppressed = negative_score >= config.negative_suppression_threshold;
            let suppression = if suppressed {
                negative_score * config.negative_suppression_factor
            } else {
                0.0
            };
            let score = (positive_score - suppression).max(0.0);
            (score >= config.min_candidate_score).then(|| LabelGroupCandidate {
                label_id: label.definition.id.clone(),
                label_name: label.definition.name.clone(),
                score,
                positive_score,
                negative_score,
                suppressed,
                evidence_atoms,
            })
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.label_name.cmp(&right.label_name))
    });
    candidates.truncate(config.max_candidates);
    Ok(candidates)
}

pub fn resolve_label_groups(
    query_embedding: &[f32],
    labels: &[SemanticLabel],
    config: &LabelSolverConfig,
) -> Result<LabelSolverResult, LabelSolverError> {
    validate_query_dimension(query_embedding, labels)?;
    let candidates = retrieve_label_groups(query_embedding, labels, config)?;
    let query_norm = l2_norm(query_embedding);

    if candidates.is_empty() || config.max_selected_labels == 0 {
        let residual_norm =
            normalized_residual_norm(query_embedding, &vec![0.0; query_embedding.len()]);
        return Ok(LabelSolverResult {
            candidates,
            selected_labels: Vec::new(),
            coverage: coverage_from_residual(residual_norm),
            residual_norm,
            needs_new_label: true,
        });
    }

    let mut selected_indices = Vec::new();
    let mut selected_vectors: Vec<Vec<f32>> = Vec::new();
    let mut selected_candidate_indices = Vec::new();
    let mut residual = normalize_for_fit(query_embedding);

    while selected_indices.len() < config.max_selected_labels {
        let next = candidates
            .iter()
            .enumerate()
            .filter(|(candidate_index, _)| !selected_candidate_indices.contains(candidate_index))
            .filter_map(|(candidate_index, candidate)| {
                let label_index = labels
                    .iter()
                    .position(|label| label.definition.id == candidate.label_id)?;
                let vector = label_group_vector(&labels[label_index], query_embedding, config)?;
                let gain = dot(&residual, &vector).max(0.0) * candidate.score.max(0.0);
                (gain > 0.0).then_some((candidate_index, label_index, vector, gain))
            })
            .max_by(|left, right| left.3.total_cmp(&right.3));

        let Some((candidate_index, label_index, vector, _gain)) = next else {
            break;
        };
        selected_candidate_indices.push(candidate_index);
        selected_indices.push(label_index);
        selected_vectors.push(vector);
        let weights = fit_non_negative(&selected_vectors, query_embedding, config.refit_iterations);
        residual = fit_residual(query_embedding, &selected_vectors, &weights, query_norm);
    }

    let weights = fit_non_negative(&selected_vectors, query_embedding, config.refit_iterations);
    let fitted = fitted_vector(&selected_vectors, &weights);
    let residual_norm = normalized_residual_norm(query_embedding, &fitted);
    let coverage = coverage_from_residual(residual_norm);

    let selected_labels = selected_indices
        .into_iter()
        .zip(selected_candidate_indices)
        .zip(weights)
        .filter(|((_label_index, _candidate_index), weight)| *weight > 0.0)
        .map(|((label_index, candidate_index), weight)| {
            let candidate = &candidates[candidate_index];
            let label = &labels[label_index];
            SelectedLabel {
                label_id: label.definition.id.clone(),
                label_name: label.definition.name.clone(),
                weight,
                score: candidate.score,
                evidence_atoms: candidate.evidence_atoms.clone(),
            }
        })
        .collect::<Vec<_>>();

    Ok(LabelSolverResult {
        candidates,
        selected_labels,
        coverage,
        residual_norm,
        needs_new_label: coverage < config.min_coverage || residual_norm > config.max_residual_norm,
    })
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LabelSolverError {
    #[error("embedding dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
    #[error("label atom belongs to {actual}, expected {expected}")]
    LabelMismatch { expected: String, actual: String },
    #[error("semantic label requires at least one embedded atom")]
    EmptyAtoms,
}

fn ensure_dimension(actual: usize, expected: usize) -> Result<(), LabelSolverError> {
    if actual == expected {
        Ok(())
    } else {
        Err(LabelSolverError::DimensionMismatch { expected, actual })
    }
}

fn push_atom_source(
    sources: &mut Vec<LabelAtomSource>,
    definition: &LabelDefinition,
    polarity: LabelAtomPolarity,
    kind: LabelAtomKind,
    text: &str,
) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    sources.push(LabelAtomSource {
        label_id: definition.id.clone(),
        label_name: definition.name.clone(),
        polarity,
        kind,
        text: text.to_owned(),
    });
}

fn validate_query_dimension(
    query_embedding: &[f32],
    labels: &[SemanticLabel],
) -> Result<(), LabelSolverError> {
    if let Some(label) = labels.first() {
        ensure_dimension(query_embedding.len(), label.embedding_dimension)?;
        for other in labels.iter().skip(1) {
            ensure_dimension(other.embedding_dimension, label.embedding_dimension)?;
        }
    }
    Ok(())
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let left_norm = l2_norm(left);
    let right_norm = l2_norm(right);
    if left_norm == 0.0 || right_norm == 0.0 {
        return 0.0;
    }
    dot(left, right) / (left_norm * right_norm)
}

fn label_group_vector(
    label: &SemanticLabel,
    query_embedding: &[f32],
    config: &LabelSolverConfig,
) -> Option<Vec<f32>> {
    let mut weighted = vec![0.0; label.embedding_dimension];
    let mut total_weight = 0.0_f32;

    for atom in label
        .atoms
        .iter()
        .filter(|atom| atom.source.polarity == LabelAtomPolarity::Positive)
    {
        let similarity = cosine_similarity(query_embedding, &atom.embedding);
        if similarity >= config.min_evidence_score {
            let weight = similarity.max(0.0);
            add_scaled(&mut weighted, &atom.embedding, weight);
            total_weight += weight;
        }
    }

    if total_weight == 0.0 {
        return None;
    }

    for value in &mut weighted {
        *value /= total_weight;
    }
    normalize_in_place(&mut weighted);
    Some(weighted)
}

fn fit_non_negative(vectors: &[Vec<f32>], query_embedding: &[f32], iterations: usize) -> Vec<f32> {
    if vectors.is_empty() {
        return Vec::new();
    }

    let query = normalize_for_fit(query_embedding);
    let mut weights = vec![0.0; vectors.len()];
    for (weight, vector) in weights.iter_mut().zip(vectors) {
        *weight = dot(&query, vector).max(0.0);
    }

    for _ in 0..iterations {
        for index in 0..vectors.len() {
            let mut residual = query.clone();
            for (other_index, (vector, weight)) in vectors.iter().zip(&weights).enumerate() {
                if other_index != index {
                    add_scaled(&mut residual, vector, -*weight);
                }
            }
            let denom = dot(&vectors[index], &vectors[index]).max(f32::EPSILON);
            weights[index] = (dot(&residual, &vectors[index]) / denom).max(0.0);
        }
    }
    weights
}

fn fitted_vector(vectors: &[Vec<f32>], weights: &[f32]) -> Vec<f32> {
    let dimension = vectors.first().map_or(0, Vec::len);
    let mut fitted = vec![0.0; dimension];
    for (vector, weight) in vectors.iter().zip(weights) {
        add_scaled(&mut fitted, vector, *weight);
    }
    fitted
}

fn fit_residual(
    query_embedding: &[f32],
    vectors: &[Vec<f32>],
    weights: &[f32],
    query_norm: f32,
) -> Vec<f32> {
    let mut residual = if query_norm == 0.0 {
        query_embedding.to_vec()
    } else {
        query_embedding
            .iter()
            .map(|value| *value / query_norm)
            .collect::<Vec<_>>()
    };
    for (vector, weight) in vectors.iter().zip(weights) {
        add_scaled(&mut residual, vector, -*weight);
    }
    residual
}

fn normalized_residual_norm(query_embedding: &[f32], fitted: &[f32]) -> f32 {
    let query = normalize_for_fit(query_embedding);
    let mut residual = query;
    for (value, fitted_value) in residual.iter_mut().zip(fitted) {
        *value -= *fitted_value;
    }
    l2_norm(&residual)
}

fn coverage_from_residual(residual_norm: f32) -> f32 {
    (1.0 - residual_norm).clamp(0.0, 1.0)
}

fn normalize_for_fit(vector: &[f32]) -> Vec<f32> {
    let mut normalized = vector.to_vec();
    normalize_in_place(&mut normalized);
    normalized
}

fn normalize_in_place(vector: &mut [f32]) {
    let norm = l2_norm(vector);
    if norm == 0.0 {
        return;
    }
    for value in vector {
        *value /= norm;
    }
}

fn add_scaled(target: &mut [f32], vector: &[f32], scale: f32) {
    for (target_value, vector_value) in target.iter_mut().zip(vector) {
        *target_value += vector_value * scale;
    }
}

fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right)
        .map(|(left_value, right_value)| left_value * right_value)
        .sum()
}

fn l2_norm(vector: &[f32]) -> f32 {
    dot(vector, vector).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition(id: &str, name: &str) -> LabelDefinition {
        LabelDefinition {
            id: id.to_owned(),
            name: name.to_owned(),
            description: Some("  Core Rust work  ".to_owned()),
            applies_when: vec![" rust implementation ".to_owned(), "".to_owned()],
            positive_examples: vec!["cargo test failure".to_owned()],
            excludes_when: vec!["frontend only".to_owned()],
            negative_examples: vec!["css layout".to_owned(), "  ".to_owned()],
        }
    }

    fn embedded_label(
        id: &str,
        name: &str,
        positive: Vec<Vec<f32>>,
        negative: Vec<Vec<f32>>,
    ) -> SemanticLabel {
        let def = LabelDefinition {
            id: id.to_owned(),
            name: name.to_owned(),
            description: None,
            applies_when: vec!["positive".to_owned()],
            positive_examples: Vec::new(),
            excludes_when: vec!["negative".to_owned()],
            negative_examples: Vec::new(),
        };
        let sources = def.atom_sources();
        let positive_source = sources
            .iter()
            .find(|source| source.polarity == LabelAtomPolarity::Positive)
            .unwrap()
            .clone();
        let negative_source = sources
            .iter()
            .find(|source| source.polarity == LabelAtomPolarity::Negative)
            .unwrap()
            .clone();
        let atoms = positive
            .into_iter()
            .map(|embedding| EmbeddedLabelAtom::new(positive_source.clone(), embedding, 3).unwrap())
            .chain(negative.into_iter().map(|embedding| {
                EmbeddedLabelAtom::new(negative_source.clone(), embedding, 3).unwrap()
            }))
            .collect();
        SemanticLabel::new(def, atoms).unwrap()
    }

    #[test]
    fn atom_source_generation_trims_and_skips_empty_text() {
        let sources = definition("l_backend", "Backend").atom_sources();

        assert_eq!(sources.len(), 6);
        assert!(sources.iter().all(|source| !source.text.is_empty()));
        assert!(
            sources
                .iter()
                .any(|source| source.kind == LabelAtomKind::Description
                    && source.text == "Core Rust work")
        );
        assert_eq!(
            sources
                .iter()
                .filter(|source| source.polarity == LabelAtomPolarity::Negative)
                .count(),
            2
        );
    }

    #[test]
    fn embedded_atom_and_semantic_label_validate_dimensions() {
        let def = definition("l_backend", "Backend");
        let source = def.atom_sources().remove(0);

        assert!(matches!(
            EmbeddedLabelAtom::new(source.clone(), vec![1.0, 0.0], 3),
            Err(LabelSolverError::DimensionMismatch {
                expected: 3,
                actual: 2
            })
        ));

        let atom = EmbeddedLabelAtom::new(source, vec![1.0, 0.0, 0.0], 3).unwrap();
        let label = SemanticLabel::new(def, vec![atom]).unwrap();
        assert_eq!(label.embedding_dimension, 3);
    }

    #[test]
    fn atom_hits_are_aggregated_to_label_group_candidates() {
        let labels = vec![
            embedded_label("l_backend", "Backend", vec![vec![1.0, 0.0, 0.0]], vec![]),
            embedded_label("l_frontend", "Frontend", vec![vec![0.0, 1.0, 0.0]], vec![]),
        ];

        let candidates =
            retrieve_label_groups(&[1.0, 0.0, 0.0], &labels, &LabelSolverConfig::default())
                .unwrap();

        assert_eq!(candidates[0].label_id, "l_backend");
        assert!(candidates[0].score > 0.9);
        assert_eq!(candidates[0].evidence_atoms.len(), 1);
    }

    #[test]
    fn multi_label_selection_returns_non_negative_refit_weights() {
        let labels = vec![
            embedded_label("l_backend", "Backend", vec![vec![1.0, 0.0, 0.0]], vec![]),
            embedded_label("l_docs", "Docs", vec![vec![0.0, 1.0, 0.0]], vec![]),
        ];
        let config = LabelSolverConfig {
            max_selected_labels: 2,
            ..LabelSolverConfig::default()
        };

        let result = resolve_label_groups(&[1.0, 1.0, 0.0], &labels, &config).unwrap();

        assert_eq!(result.selected_labels.len(), 2);
        assert!(
            result
                .selected_labels
                .iter()
                .all(|label| label.weight >= 0.0)
        );
        assert!(result.coverage > 0.9);
        assert!(!result.needs_new_label);
    }

    #[test]
    fn negative_atom_suppresses_same_label_candidate() {
        let labels = vec![embedded_label(
            "l_frontend",
            "Frontend",
            vec![vec![1.0, 0.0, 0.0]],
            vec![vec![1.0, 0.0, 0.0]],
        )];

        let candidates =
            retrieve_label_groups(&[1.0, 0.0, 0.0], &labels, &LabelSolverConfig::default())
                .unwrap();

        assert!(candidates[0].suppressed);
        assert!(candidates[0].score < candidates[0].positive_score);
    }

    #[test]
    fn low_coverage_marks_needs_new_label() {
        let labels = vec![embedded_label(
            "l_backend",
            "Backend",
            vec![vec![1.0, 0.0, 0.0]],
            vec![],
        )];

        let result =
            resolve_label_groups(&[0.0, 0.0, 1.0], &labels, &LabelSolverConfig::default()).unwrap();

        assert!(result.needs_new_label);
        assert!(result.coverage < LabelSolverConfig::default().min_coverage);
    }

    #[test]
    fn query_dimension_mismatch_returns_error() {
        let labels = vec![embedded_label(
            "l_backend",
            "Backend",
            vec![vec![1.0, 0.0, 0.0]],
            vec![],
        )];

        assert!(matches!(
            retrieve_label_groups(&[1.0, 0.0], &labels, &LabelSolverConfig::default()),
            Err(LabelSolverError::DimensionMismatch {
                expected: 3,
                actual: 2
            })
        ));
    }
}
