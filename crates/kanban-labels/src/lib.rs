//! 纯内存 semantic label solver。
//!
//! 本 crate 只负责把上层传入的 label 定义、atom embedding 和 query embedding
//! 解析成可解释的候选与多 label 选择结果。它不连接 SQLite，不替代
//! `labels` / `task_labels` 的 canonical 存储。

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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
        if let Some(description) = self
            .description
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
        {
            let canonical = format!("label: {}\ndescription: {}", self.name.trim(), description);
            push_atom_source(
                &mut sources,
                self,
                LabelAtomPolarity::Positive,
                LabelAtomKind::Description,
                &canonical,
            );
        } else {
            push_atom_source(
                &mut sources,
                self,
                LabelAtomPolarity::Positive,
                LabelAtomKind::Name,
                &self.name,
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
    pub atom_id: Option<String>,
    pub source: LabelAtomSource,
    pub similarity: f32,
    pub contribution: f32,
    #[serde(skip)]
    pub vector: Option<Vec<f32>>,
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
    pub negative_evidence_atoms: Vec<LabelAtomEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectedLabel {
    pub label_id: String,
    pub label_name: String,
    pub weight: f32,
    pub score: f32,
    pub evidence_atoms: Vec<LabelAtomEvidence>,
    pub negative_evidence_atoms: Vec<LabelAtomEvidence>,
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
    pub min_refit_gain: f32,
    pub coverage_stop: f32,
    pub residual_norm_stop: f32,
    pub max_redundancy: f32,
    pub refit_iterations: usize,
    pub max_positive_atoms_per_label: usize,
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
            min_refit_gain: 0.025,
            coverage_stop: 0.85,
            residual_norm_stop: 0.30,
            max_redundancy: 0.92,
            refit_iterations: 40,
            max_positive_atoms_per_label: 3,
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
    #[serde(skip)]
    pub residual: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievedLabelAtom {
    pub atom_id: String,
    pub label_id: String,
    pub label_name: String,
    pub polarity: LabelAtomPolarity,
    pub kind: LabelAtomKind,
    pub text: String,
    pub vector: Vec<f32>,
    pub similarity: Option<f32>,
}

pub fn retrieve_label_groups(
    query_embedding: &[f32],
    labels: &[SemanticLabel],
    config: &LabelSolverConfig,
) -> Result<Vec<LabelGroupCandidate>, LabelSolverError> {
    validate_solver_inputs(query_embedding, labels)?;

    let mut candidates = labels
        .iter()
        .filter_map(|label| {
            let mut positive_score = 0.0_f32;
            let mut negative_score = 0.0_f32;
            let mut evidence_atoms = Vec::new();
            let mut negative_evidence_atoms = Vec::new();

            for atom in &label.atoms {
                let similarity = cosine_similarity(query_embedding, &atom.embedding);
                match atom.source.polarity {
                    LabelAtomPolarity::Positive => {
                        positive_score = positive_score.max(similarity);
                        if similarity >= config.min_evidence_score {
                            evidence_atoms.push(LabelAtomEvidence {
                                atom_id: None,
                                source: atom.source.clone(),
                                similarity,
                                contribution: similarity.max(0.0),
                                vector: Some(atom.embedding.clone()),
                            });
                        }
                    }
                    LabelAtomPolarity::Negative => {
                        negative_score = negative_score.max(similarity);
                        if similarity >= config.min_evidence_score {
                            negative_evidence_atoms.push(LabelAtomEvidence {
                                atom_id: None,
                                source: atom.source.clone(),
                                similarity,
                                contribution: similarity.max(0.0)
                                    * config.negative_suppression_factor,
                                vector: None,
                            });
                        }
                    }
                }
            }

            evidence_atoms.sort_by(|left, right| {
                right
                    .contribution
                    .total_cmp(&left.contribution)
                    .then_with(|| left.source.text.cmp(&right.source.text))
            });
            negative_evidence_atoms.sort_by(|left, right| {
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
            let keep_candidate = score >= config.min_candidate_score
                || (suppressed && positive_score >= config.min_candidate_score);
            keep_candidate.then(|| LabelGroupCandidate {
                label_id: label.definition.id.clone(),
                label_name: label.definition.name.clone(),
                score,
                positive_score,
                negative_score,
                suppressed,
                evidence_atoms,
                negative_evidence_atoms,
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
    validate_solver_inputs(query_embedding, labels)?;
    validate_selection_config(config)?;
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
            residual: normalize_for_fit(query_embedding),
        });
    }

    let mut selected_indices = Vec::new();
    let mut selected_vectors: Vec<Vec<f32>> = Vec::new();
    let mut selected_group_vectors: Vec<Vec<f32>> = Vec::new();
    let mut selected_candidate_indices = Vec::new();
    let mut residual = normalize_for_fit(query_embedding);

    while selected_indices.len() < config.max_selected_labels {
        let residual_norm = l2_norm(&residual);
        if selection_stop_reached(residual_norm, config) {
            break;
        }

        let mut ranked = candidates
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
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| {
            right.3.total_cmp(&left.3).then_with(|| {
                candidates[left.0]
                    .label_name
                    .cmp(&candidates[right.0].label_name)
            })
        });

        let mut accepted = false;
        for (candidate_index, label_index, vector, _gain) in ranked {
            if redundancy_exceeds(&vector, &selected_group_vectors, config) {
                continue;
            }
            let mut tentative_vectors = selected_vectors.clone();
            tentative_vectors.push(vector.clone());
            let weights =
                fit_non_negative(&tentative_vectors, query_embedding, config.refit_iterations);
            let fitted = fitted_vector(&tentative_vectors, &weights);
            let new_residual_norm = normalized_residual_norm(query_embedding, &fitted);
            let gain = refit_gain(residual_norm, new_residual_norm);
            if !refit_gain_is_sufficient(gain, config) {
                continue;
            }
            selected_candidate_indices.push(candidate_index);
            selected_indices.push(label_index);
            selected_vectors = tentative_vectors;
            selected_group_vectors.push(vector);
            residual = fit_residual(query_embedding, &selected_vectors, &weights, query_norm);
            accepted = true;
            break;
        }
        if !accepted {
            break;
        }
    }

    let weights = fit_non_negative(&selected_vectors, query_embedding, config.refit_iterations);
    let fitted = fitted_vector(&selected_vectors, &weights);
    let residual = fit_residual(query_embedding, &selected_vectors, &weights, query_norm);
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
                negative_evidence_atoms: candidate.negative_evidence_atoms.clone(),
            }
        })
        .collect::<Vec<_>>();

    Ok(LabelSolverResult {
        candidates,
        selected_labels,
        coverage,
        residual_norm,
        needs_new_label: coverage < config.min_coverage || residual_norm > config.max_residual_norm,
        residual,
    })
}

pub fn resolve_label_groups_by_residual<R>(
    query_embedding: &[f32],
    config: &LabelSolverConfig,
    mut retrieve: R,
) -> Result<LabelSolverResult, LabelSolverError>
where
    R: FnMut(&[f32], LabelAtomPolarity, usize) -> Result<Vec<RetrievedLabelAtom>, LabelSolverError>,
{
    validate_residual_inputs(query_embedding, config)?;
    let query_norm = l2_norm(query_embedding);
    let normalized_query = normalize_for_fit(query_embedding);
    let mut residual = normalize_for_fit(query_embedding);
    let mut selected_label_ids = HashSet::new();
    let mut selected_vectors = Vec::new();
    let mut selected_group_vectors: Vec<Vec<f32>> = Vec::new();
    let mut selected_basis = Vec::new();
    let mut candidates_by_label: HashMap<String, LabelGroupCandidate> = HashMap::new();
    let retrieval_limit = config
        .max_candidates
        .saturating_mul(config.max_positive_atoms_per_label.max(1))
        .max(config.max_candidates)
        .max(1);

    while selected_label_ids.len() < config.max_selected_labels {
        let residual_norm = l2_norm(&residual);
        if residual_norm <= f32::EPSILON || selection_stop_reached(residual_norm, config) {
            break;
        }
        let residual_query = normalize_for_fit(&residual);
        let positive_hits = retrieve(
            &residual_query,
            LabelAtomPolarity::Positive,
            retrieval_limit,
        )?;
        let negative_hits = retrieve(
            &normalized_query,
            LabelAtomPolarity::Negative,
            retrieval_limit,
        )?;
        let groups = residual_candidates(
            &residual,
            &normalized_query,
            positive_hits,
            negative_hits,
            &selected_label_ids,
            config,
        )?;
        for candidate in &groups {
            candidates_by_label
                .entry(candidate.label_id.clone())
                .and_modify(|existing| {
                    if candidate.score > existing.score {
                        *existing = candidate.clone();
                    }
                })
                .or_insert_with(|| candidate.clone());
        }

        let mut ranked = groups
            .into_iter()
            .filter(|candidate| candidate.score >= config.min_candidate_score)
            .filter_map(|candidate| {
                let vector = candidate_group_vector(&candidate)?;
                let gain = dot(&residual, &vector).max(0.0) * candidate.score.max(0.0);
                (gain > 0.0).then_some((candidate, vector, gain))
            })
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| {
            right
                .2
                .total_cmp(&left.2)
                .then_with(|| left.0.label_name.cmp(&right.0.label_name))
        });

        let mut accepted = false;
        for (candidate, vector, _gain) in ranked {
            if redundancy_exceeds(&vector, &selected_group_vectors, config) {
                continue;
            }
            let basis_atoms = candidate_basis_atoms(&candidate);
            if basis_atoms.is_empty() {
                continue;
            }
            let mut tentative_vectors = selected_vectors.clone();
            for (_evidence, atom_vector) in &basis_atoms {
                tentative_vectors.push(atom_vector.clone());
            }
            let weights =
                fit_non_negative(&tentative_vectors, query_embedding, config.refit_iterations);
            let fitted = fitted_vector(&tentative_vectors, &weights);
            let new_residual_norm = normalized_residual_norm(query_embedding, &fitted);
            let gain = refit_gain(residual_norm, new_residual_norm);
            if !refit_gain_is_sufficient(gain, config) {
                continue;
            }

            selected_label_ids.insert(candidate.label_id.clone());
            selected_vectors = tentative_vectors;
            selected_group_vectors.push(vector);
            for (evidence, _atom_vector) in basis_atoms {
                selected_basis.push(SelectedBasisAtom {
                    label_id: candidate.label_id.clone(),
                    label_name: candidate.label_name.clone(),
                    score: candidate.score,
                    evidence,
                    negative_evidence_atoms: candidate.negative_evidence_atoms.clone(),
                });
            }
            residual = fit_residual(query_embedding, &selected_vectors, &weights, query_norm);
            accepted = true;
            break;
        }
        if !accepted {
            break;
        }
    }

    let weights = fit_non_negative(&selected_vectors, query_embedding, config.refit_iterations);
    let fitted = fitted_vector(&selected_vectors, &weights);
    let residual = fit_residual(query_embedding, &selected_vectors, &weights, query_norm);
    let residual_norm = normalized_residual_norm(query_embedding, &fitted);
    let coverage = coverage_from_residual(residual_norm);
    let selected_labels = selected_labels_from_basis(&selected_basis, &weights);
    let mut candidates = candidates_by_label.into_values().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.label_name.cmp(&right.label_name))
    });
    candidates.truncate(config.max_candidates);
    Ok(LabelSolverResult {
        candidates,
        selected_labels,
        coverage,
        residual_norm,
        needs_new_label: coverage < config.min_coverage || residual_norm > config.max_residual_norm,
        residual,
    })
}

fn validate_residual_inputs(
    query_embedding: &[f32],
    config: &LabelSolverConfig,
) -> Result<(), LabelSolverError> {
    validate_selection_config(config)?;
    if l2_norm(query_embedding) == 0.0 {
        return Err(LabelSolverError::ZeroQueryEmbedding);
    }
    if config.max_positive_atoms_per_label == 0 {
        return Err(LabelSolverError::InvalidConfig(
            "max_positive_atoms_per_label must be >= 1".to_owned(),
        ));
    }
    Ok(())
}

fn validate_selection_config(config: &LabelSolverConfig) -> Result<(), LabelSolverError> {
    if config.min_refit_gain < 0.0 {
        return Err(LabelSolverError::InvalidConfig(
            "min_refit_gain must be >= 0".to_owned(),
        ));
    }
    if !(0.0..=1.0).contains(&config.coverage_stop) {
        return Err(LabelSolverError::InvalidConfig(
            "coverage_stop must be between 0 and 1".to_owned(),
        ));
    }
    if config.residual_norm_stop < 0.0 {
        return Err(LabelSolverError::InvalidConfig(
            "residual_norm_stop must be >= 0".to_owned(),
        ));
    }
    if !(0.0..=1.0).contains(&config.max_redundancy) {
        return Err(LabelSolverError::InvalidConfig(
            "max_redundancy must be between 0 and 1".to_owned(),
        ));
    }
    Ok(())
}

fn residual_candidates(
    positive_query: &[f32],
    negative_query: &[f32],
    positive_hits: Vec<RetrievedLabelAtom>,
    negative_hits: Vec<RetrievedLabelAtom>,
    selected_label_ids: &HashSet<String>,
    config: &LabelSolverConfig,
) -> Result<Vec<LabelGroupCandidate>, LabelSolverError> {
    let mut positives_by_label: HashMap<String, Vec<RetrievedLabelAtom>> = HashMap::new();
    for hit in positive_hits {
        validate_retrieved_atom(&hit, positive_query.len(), LabelAtomPolarity::Positive)?;
        if !selected_label_ids.contains(&hit.label_id) {
            positives_by_label
                .entry(hit.label_id.clone())
                .or_default()
                .push(hit);
        }
    }
    let mut negatives_by_label: HashMap<String, Vec<RetrievedLabelAtom>> = HashMap::new();
    for hit in negative_hits {
        validate_retrieved_atom(&hit, negative_query.len(), LabelAtomPolarity::Negative)?;
        negatives_by_label
            .entry(hit.label_id.clone())
            .or_default()
            .push(hit);
    }

    let mut candidates = Vec::new();
    for (label_id, mut positives) in positives_by_label {
        positives.sort_by(|left, right| {
            retrieved_similarity(positive_query, right)
                .total_cmp(&retrieved_similarity(positive_query, left))
                .then_with(|| left.atom_id.cmp(&right.atom_id))
        });
        positives.truncate(config.max_positive_atoms_per_label);
        let Some(first) = positives.first() else {
            continue;
        };
        let mut evidence_atoms = positives
            .iter()
            .map(|hit| atom_evidence(hit, positive_query))
            .collect::<Vec<_>>();
        evidence_atoms.sort_by(|left, right| {
            right
                .contribution
                .total_cmp(&left.contribution)
                .then_with(|| left.source.text.cmp(&right.source.text))
        });
        let positive_score = evidence_atoms
            .iter()
            .map(|atom| atom.similarity)
            .fold(0.0_f32, f32::max);
        let mut negative_evidence_atoms = negatives_by_label
            .remove(&label_id)
            .unwrap_or_default()
            .iter()
            .map(|hit| atom_evidence(hit, negative_query))
            .filter(|evidence| evidence.similarity >= config.min_evidence_score)
            .collect::<Vec<_>>();
        negative_evidence_atoms.sort_by(|left, right| {
            right
                .contribution
                .total_cmp(&left.contribution)
                .then_with(|| left.source.text.cmp(&right.source.text))
        });
        negative_evidence_atoms.truncate(config.max_positive_atoms_per_label);
        let negative_score = negative_evidence_atoms
            .iter()
            .map(|atom| atom.similarity)
            .fold(0.0_f32, f32::max);
        let suppressed = negative_score >= config.negative_suppression_threshold;
        let suppression = if suppressed {
            negative_score * config.negative_suppression_factor
        } else {
            0.0
        };
        let score = (positive_score - suppression).max(0.0);
        if score >= config.min_candidate_score
            || (suppressed && positive_score >= config.min_candidate_score)
        {
            candidates.push(LabelGroupCandidate {
                label_id,
                label_name: first.label_name.clone(),
                score,
                positive_score,
                negative_score,
                suppressed,
                evidence_atoms,
                negative_evidence_atoms,
            });
        }
    }
    candidates.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.label_name.cmp(&right.label_name))
    });
    candidates.truncate(config.max_candidates);
    Ok(candidates)
}

#[derive(Debug, Clone)]
struct SelectedBasisAtom {
    label_id: String,
    label_name: String,
    score: f32,
    evidence: LabelAtomEvidence,
    negative_evidence_atoms: Vec<LabelAtomEvidence>,
}

fn candidate_basis_atoms(candidate: &LabelGroupCandidate) -> Vec<(LabelAtomEvidence, Vec<f32>)> {
    candidate
        .evidence_atoms
        .iter()
        .filter_map(|evidence| {
            let mut vector = evidence.vector.clone()?;
            normalize_in_place(&mut vector);
            Some((evidence.clone(), vector))
        })
        .collect()
}

fn selected_labels_from_basis(
    selected_basis: &[SelectedBasisAtom],
    weights: &[f32],
) -> Vec<SelectedLabel> {
    let mut by_label: Vec<SelectedLabel> = Vec::new();
    for (basis, weight) in selected_basis.iter().zip(weights) {
        if *weight <= 0.0 {
            continue;
        }
        let mut evidence = basis.evidence.clone();
        evidence.contribution = *weight;
        if let Some(existing) = by_label
            .iter_mut()
            .find(|label| label.label_id == basis.label_id)
        {
            existing.weight += *weight;
            existing.evidence_atoms.push(evidence);
        } else {
            by_label.push(SelectedLabel {
                label_id: basis.label_id.clone(),
                label_name: basis.label_name.clone(),
                weight: *weight,
                score: basis.score,
                evidence_atoms: vec![evidence],
                negative_evidence_atoms: basis.negative_evidence_atoms.clone(),
            });
        }
    }
    for label in &mut by_label {
        label.evidence_atoms.sort_by(|left, right| {
            right
                .contribution
                .total_cmp(&left.contribution)
                .then_with(|| left.source.text.cmp(&right.source.text))
        });
    }
    by_label
}

fn validate_retrieved_atom(
    atom: &RetrievedLabelAtom,
    expected_dimension: usize,
    expected_polarity: LabelAtomPolarity,
) -> Result<(), LabelSolverError> {
    if atom.polarity != expected_polarity {
        return Err(LabelSolverError::RetrievedPolarityMismatch {
            expected: expected_polarity,
            actual: atom.polarity,
        });
    }
    ensure_dimension(atom.vector.len(), expected_dimension)
}

fn atom_evidence(atom: &RetrievedLabelAtom, residual: &[f32]) -> LabelAtomEvidence {
    let similarity = retrieved_similarity(residual, atom);
    LabelAtomEvidence {
        atom_id: Some(atom.atom_id.clone()),
        source: LabelAtomSource {
            label_id: atom.label_id.clone(),
            label_name: atom.label_name.clone(),
            polarity: atom.polarity,
            kind: atom.kind,
            text: atom.text.clone(),
        },
        similarity,
        contribution: similarity,
        vector: Some(atom.vector.clone()),
    }
}

fn retrieved_similarity(query: &[f32], atom: &RetrievedLabelAtom) -> f32 {
    // Retrieved vectors are authoritative for solver math. External store
    // scores/distances are optional diagnostics and must not inflate evidence.
    cosine_similarity(query, &atom.vector).max(0.0)
}

fn candidate_group_vector(candidate: &LabelGroupCandidate) -> Option<Vec<f32>> {
    let dimension = candidate
        .evidence_atoms
        .iter()
        .find_map(|evidence| evidence.vector.as_ref().map(Vec::len))?;
    let mut vector = vec![0.0; dimension];
    let mut total_weight = 0.0_f32;
    for evidence in &candidate.evidence_atoms {
        let Some(atom_vector) = evidence.vector.as_ref() else {
            continue;
        };
        let weight = evidence.contribution.max(0.0);
        add_scaled(&mut vector, atom_vector, weight);
        total_weight += weight;
    }
    if total_weight == 0.0 {
        return None;
    }
    for value in &mut vector {
        *value /= total_weight;
    }
    normalize_in_place(&mut vector);
    Some(vector)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LabelSolverError {
    #[error("embedding dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
    #[error("label atom belongs to {actual}, expected {expected}")]
    LabelMismatch { expected: String, actual: String },
    #[error("semantic label requires at least one embedded atom")]
    EmptyAtoms,
    #[error("query embedding has no semantic signal")]
    ZeroQueryEmbedding,
    #[error("invalid label solver config: {0}")]
    InvalidConfig(String),
    #[error("retrieved atom polarity mismatch: expected {expected:?}, got {actual:?}")]
    RetrievedPolarityMismatch {
        expected: LabelAtomPolarity,
        actual: LabelAtomPolarity,
    },
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

fn validate_solver_inputs(
    query_embedding: &[f32],
    labels: &[SemanticLabel],
) -> Result<(), LabelSolverError> {
    if l2_norm(query_embedding) == 0.0 {
        return Err(LabelSolverError::ZeroQueryEmbedding);
    }

    let Some(first_label) = labels.first() else {
        return Ok(());
    };

    validate_semantic_label(first_label)?;
    ensure_dimension(query_embedding.len(), first_label.embedding_dimension)?;

    for label in labels.iter().skip(1) {
        validate_semantic_label(label)?;
        ensure_dimension(label.embedding_dimension, first_label.embedding_dimension)?;
    }
    Ok(())
}

fn validate_semantic_label(label: &SemanticLabel) -> Result<(), LabelSolverError> {
    if label.atoms.is_empty() {
        return Err(LabelSolverError::EmptyAtoms);
    }

    for atom in &label.atoms {
        if atom.source.label_id != label.definition.id {
            return Err(LabelSolverError::LabelMismatch {
                expected: label.definition.id.clone(),
                actual: atom.source.label_id.clone(),
            });
        }
        if atom.source.label_name != label.definition.name {
            return Err(LabelSolverError::LabelMismatch {
                expected: label.definition.name.clone(),
                actual: atom.source.label_name.clone(),
            });
        }
        ensure_dimension(atom.embedding.len(), label.embedding_dimension)?;
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

fn selection_stop_reached(residual_norm: f32, config: &LabelSolverConfig) -> bool {
    residual_norm <= config.residual_norm_stop
        || coverage_from_residual(residual_norm) >= config.coverage_stop
}

fn refit_gain(old_residual_norm: f32, new_residual_norm: f32) -> f32 {
    (old_residual_norm - new_residual_norm).max(0.0)
}

fn refit_gain_is_sufficient(gain: f32, config: &LabelSolverConfig) -> bool {
    gain + f32::EPSILON >= config.min_refit_gain
}

fn redundancy_exceeds(
    vector: &[f32],
    selected_group_vectors: &[Vec<f32>],
    config: &LabelSolverConfig,
) -> bool {
    selected_group_vectors
        .iter()
        .map(|selected| cosine_similarity(vector, selected))
        .fold(0.0_f32, f32::max)
        > config.max_redundancy
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

    fn retrieved_atom(
        label_id: &str,
        label_name: &str,
        polarity: LabelAtomPolarity,
        vector: Vec<f32>,
        similarity: f32,
        ordinal: usize,
    ) -> RetrievedLabelAtom {
        RetrievedLabelAtom {
            atom_id: format!("{label_id}_{ordinal}"),
            label_id: label_id.to_owned(),
            label_name: label_name.to_owned(),
            polarity,
            kind: match polarity {
                LabelAtomPolarity::Positive => LabelAtomKind::AppliesWhen,
                LabelAtomPolarity::Negative => LabelAtomKind::ExcludesWhen,
            },
            text: format!("{label_name} {ordinal}"),
            vector,
            similarity: Some(similarity),
        }
    }

    #[test]
    fn atom_source_generation_trims_and_skips_empty_text() {
        let sources = definition("l_backend", "Backend").atom_sources();

        assert_eq!(sources.len(), 5);
        assert!(sources.iter().all(|source| !source.text.is_empty()));
        let canonical = sources
            .iter()
            .find(|source| source.kind == LabelAtomKind::Description)
            .unwrap();
        assert_eq!(
            canonical.text,
            "label: Backend\ndescription: Core Rust work"
        );
        assert!(
            sources
                .iter()
                .filter(|source| source.polarity == LabelAtomPolarity::Positive)
                .all(|source| source.kind != LabelAtomKind::Name)
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
    fn atom_source_generation_falls_back_to_name_without_description() {
        let mut definition = definition("l_backend", "Backend");
        definition.description = None;
        let sources = definition.atom_sources();

        assert!(
            sources
                .iter()
                .any(|source| source.kind == LabelAtomKind::Name && source.text == "Backend")
        );
        assert!(
            sources
                .iter()
                .all(|source| source.kind != LabelAtomKind::Description)
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
    fn residual_solver_activates_later_labels_from_dynamic_residual_queries() {
        let config = LabelSolverConfig {
            max_selected_labels: 3,
            min_candidate_score: 0.01,
            ..LabelSolverConfig::default()
        };
        let mut positive_queries = Vec::new();

        let result = resolve_label_groups_by_residual(
            &[1.0, 1.0, 1.0],
            &config,
            |residual, polarity, limit| {
                assert!(limit >= 3);
                match polarity {
                    LabelAtomPolarity::Positive => {
                        positive_queries.push(residual.to_vec());
                        let axis = if residual[0] > 0.4 {
                            (0, "l_backend", "backend")
                        } else if residual[1] > 0.4 {
                            (1, "l_docs", "docs")
                        } else {
                            (2, "l_tests", "tests")
                        };
                        let mut vector = vec![0.0, 0.0, 0.0];
                        vector[axis.0] = 1.0;
                        Ok(vec![retrieved_atom(
                            axis.1,
                            axis.2,
                            LabelAtomPolarity::Positive,
                            vector,
                            0.95,
                            positive_queries.len(),
                        )])
                    }
                    LabelAtomPolarity::Negative => Ok(Vec::new()),
                }
            },
        )
        .unwrap();

        assert_eq!(
            result
                .selected_labels
                .iter()
                .map(|label| label.label_name.as_str())
                .collect::<Vec<_>>(),
            vec!["backend", "docs", "tests"]
        );
        assert!(positive_queries.len() >= 3);
        assert!(
            positive_queries
                .iter()
                .all(|query| (l2_norm(query) - 1.0).abs() < 0.0001)
        );
        assert!(result.coverage > 0.99);
        assert!(result.residual_norm < 0.01);
    }

    #[test]
    fn residual_solver_keeps_negative_atoms_out_of_refit_basis() {
        let config = LabelSolverConfig {
            max_selected_labels: 1,
            negative_suppression_factor: 1.0,
            min_candidate_score: 0.0,
            ..LabelSolverConfig::default()
        };

        let result = resolve_label_groups_by_residual(
            &[1.0, 0.0, 0.0],
            &config,
            |_residual, polarity, _limit| match polarity {
                LabelAtomPolarity::Positive => Ok(vec![retrieved_atom(
                    "l_frontend",
                    "frontend",
                    LabelAtomPolarity::Positive,
                    vec![1.0, 0.0, 0.0],
                    1.0,
                    1,
                )]),
                LabelAtomPolarity::Negative => Ok(vec![retrieved_atom(
                    "l_frontend",
                    "frontend",
                    LabelAtomPolarity::Negative,
                    vec![1.0, 0.0, 0.0],
                    1.0,
                    2,
                )]),
            },
        )
        .unwrap();

        assert!(result.candidates[0].suppressed);
        assert_eq!(result.candidates[0].score, 0.0);
        assert!(result.selected_labels.is_empty());
        assert_eq!(result.coverage, 0.0);
        assert_eq!(result.residual_norm, 1.0);
    }

    #[test]
    fn residual_solver_scores_negative_atoms_against_original_query() {
        let config = LabelSolverConfig {
            max_selected_labels: 2,
            max_positive_atoms_per_label: 1,
            min_candidate_score: 0.01,
            negative_suppression_factor: 1.0,
            negative_suppression_threshold: 0.8,
            ..LabelSolverConfig::default()
        };
        let mut negative_queries = Vec::new();

        let result = resolve_label_groups_by_residual(
            &[1.0, 1.0, 0.0],
            &config,
            |query, polarity, _limit| match polarity {
                LabelAtomPolarity::Positive => {
                    if query[0] > 0.4 {
                        Ok(vec![retrieved_atom(
                            "l_backend",
                            "backend",
                            LabelAtomPolarity::Positive,
                            vec![1.0, 0.0, 0.0],
                            1.0,
                            1,
                        )])
                    } else {
                        Ok(vec![retrieved_atom(
                            "l_docs",
                            "docs",
                            LabelAtomPolarity::Positive,
                            vec![0.0, 1.0, 0.0],
                            1.0,
                            2,
                        )])
                    }
                }
                LabelAtomPolarity::Negative => {
                    negative_queries.push(query.to_vec());
                    Ok(vec![retrieved_atom(
                        "l_docs",
                        "docs",
                        LabelAtomPolarity::Negative,
                        vec![1.0, 1.0, 0.0],
                        1.0,
                        3,
                    )])
                }
            },
        )
        .unwrap();

        assert_eq!(
            result
                .selected_labels
                .iter()
                .map(|label| label.label_name.as_str())
                .collect::<Vec<_>>(),
            vec!["backend"]
        );
        assert!(
            result
                .candidates
                .iter()
                .any(|candidate| candidate.label_name == "docs" && candidate.suppressed)
        );
        assert!(negative_queries.len() >= 2);
        assert!(negative_queries.iter().all(|query| {
            (query[0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.0001
                && (query[1] - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.0001
                && query[2].abs() < 0.0001
        }));
    }

    #[test]
    fn residual_solver_refits_selected_label_with_atom_level_basis() {
        let config = LabelSolverConfig {
            max_selected_labels: 1,
            max_positive_atoms_per_label: 2,
            min_candidate_score: 0.01,
            ..LabelSolverConfig::default()
        };

        let result = resolve_label_groups_by_residual(
            &[1.0, 1.0, 0.0],
            &config,
            |_query, polarity, _limit| match polarity {
                LabelAtomPolarity::Positive => Ok(vec![
                    retrieved_atom(
                        "l_fullstack",
                        "fullstack",
                        LabelAtomPolarity::Positive,
                        vec![1.0, 0.0, 0.0],
                        1.0,
                        1,
                    ),
                    retrieved_atom(
                        "l_fullstack",
                        "fullstack",
                        LabelAtomPolarity::Positive,
                        vec![0.0, 1.0, 0.0],
                        1.0,
                        2,
                    ),
                ]),
                LabelAtomPolarity::Negative => Ok(Vec::new()),
            },
        )
        .unwrap();

        assert_eq!(result.selected_labels.len(), 1);
        let selected = &result.selected_labels[0];
        assert_eq!(selected.label_name, "fullstack");
        assert_eq!(selected.evidence_atoms.len(), 2);
        assert!(
            selected
                .evidence_atoms
                .iter()
                .all(|atom| (atom.contribution - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.0001)
        );
        assert!((selected.weight - std::f32::consts::SQRT_2).abs() < 0.0001);
        assert!(result.coverage > 0.99);
        assert!(result.residual_norm < 0.01);
    }

    #[test]
    fn residual_solver_does_not_promote_retrieved_score_over_local_cosine() {
        let config = LabelSolverConfig {
            max_selected_labels: 1,
            min_candidate_score: 0.05,
            ..LabelSolverConfig::default()
        };

        let result = resolve_label_groups_by_residual(
            &[1.0, 0.0, 0.0],
            &config,
            |_query, polarity, _limit| match polarity {
                LabelAtomPolarity::Positive => Ok(vec![retrieved_atom(
                    "l_unrelated",
                    "unrelated",
                    LabelAtomPolarity::Positive,
                    vec![0.0, 1.0, 0.0],
                    0.99,
                    1,
                )]),
                LabelAtomPolarity::Negative => Ok(Vec::new()),
            },
        )
        .unwrap();

        assert!(result.candidates.is_empty());
        assert!(result.selected_labels.is_empty());
        assert_eq!(result.coverage, 0.0);
        assert_eq!(result.residual_norm, 1.0);
    }

    #[test]
    fn residual_solver_stops_before_retrieving_zero_residual() {
        let config = LabelSolverConfig {
            max_selected_labels: 2,
            min_candidate_score: 0.01,
            ..LabelSolverConfig::default()
        };
        let mut positive_queries = Vec::new();

        let result = resolve_label_groups_by_residual(
            &[1.0, 0.0, 0.0],
            &config,
            |query, polarity, _limit| match polarity {
                LabelAtomPolarity::Positive => {
                    positive_queries.push(query.to_vec());
                    Ok(vec![retrieved_atom(
                        "l_backend",
                        "backend",
                        LabelAtomPolarity::Positive,
                        vec![1.0, 0.0, 0.0],
                        1.0,
                        positive_queries.len(),
                    )])
                }
                LabelAtomPolarity::Negative => Ok(Vec::new()),
            },
        )
        .unwrap();

        assert_eq!(positive_queries.len(), 1);
        assert!((l2_norm(&positive_queries[0]) - 1.0).abs() < 0.0001);
        assert_eq!(result.selected_labels.len(), 1);
        assert!(result.coverage > 0.99);
        assert!(result.residual_norm < 0.01);
    }

    #[test]
    fn residual_solver_rolls_back_label_when_refit_gain_is_too_small() {
        let config = LabelSolverConfig {
            max_selected_labels: 2,
            min_candidate_score: 0.01,
            min_refit_gain: 0.25,
            coverage_stop: 1.0,
            residual_norm_stop: 0.0,
            ..LabelSolverConfig::default()
        };
        let mut positive_queries = Vec::new();

        let result = resolve_label_groups_by_residual(
            &[1.0, 0.2, 0.0],
            &config,
            |query, polarity, _limit| match polarity {
                LabelAtomPolarity::Positive => {
                    positive_queries.push(query.to_vec());
                    if positive_queries.len() == 1 {
                        Ok(vec![
                            retrieved_atom(
                                "l_primary",
                                "primary",
                                LabelAtomPolarity::Positive,
                                vec![1.0, 0.0, 0.0],
                                1.0,
                                1,
                            ),
                            retrieved_atom(
                                "l_tail",
                                "tail",
                                LabelAtomPolarity::Positive,
                                vec![0.0, 1.0, 0.0],
                                1.0,
                                2,
                            ),
                        ])
                    } else {
                        Ok(vec![retrieved_atom(
                            "l_tail",
                            "tail",
                            LabelAtomPolarity::Positive,
                            vec![0.0, 1.0, 0.0],
                            1.0,
                            3,
                        )])
                    }
                }
                LabelAtomPolarity::Negative => Ok(Vec::new()),
            },
        )
        .unwrap();

        assert_eq!(
            result
                .selected_labels
                .iter()
                .map(|label| label.label_name.as_str())
                .collect::<Vec<_>>(),
            vec!["primary"]
        );
        assert!(positive_queries.len() >= 2);
        assert!(
            result.residual_norm > 0.15,
            "tail label should be rolled back instead of fully fitting the residual"
        );
    }

    #[test]
    fn residual_solver_accepts_label_when_refit_gain_is_large_enough() {
        let config = LabelSolverConfig {
            max_selected_labels: 2,
            min_candidate_score: 0.01,
            min_refit_gain: 0.25,
            coverage_stop: 1.0,
            residual_norm_stop: 0.0,
            ..LabelSolverConfig::default()
        };

        let result = resolve_label_groups_by_residual(
            &[1.0, 1.0, 0.0],
            &config,
            |query, polarity, _limit| match polarity {
                LabelAtomPolarity::Positive => {
                    if query[0] > 0.4 {
                        Ok(vec![
                            retrieved_atom(
                                "l_primary",
                                "primary",
                                LabelAtomPolarity::Positive,
                                vec![1.0, 0.0, 0.0],
                                1.0,
                                1,
                            ),
                            retrieved_atom(
                                "l_tail",
                                "tail",
                                LabelAtomPolarity::Positive,
                                vec![0.0, 1.0, 0.0],
                                1.0,
                                2,
                            ),
                        ])
                    } else {
                        Ok(vec![retrieved_atom(
                            "l_tail",
                            "tail",
                            LabelAtomPolarity::Positive,
                            vec![0.0, 1.0, 0.0],
                            1.0,
                            3,
                        )])
                    }
                }
                LabelAtomPolarity::Negative => Ok(Vec::new()),
            },
        )
        .unwrap();

        assert_eq!(
            result
                .selected_labels
                .iter()
                .map(|label| label.label_name.as_str())
                .collect::<Vec<_>>(),
            vec!["primary", "tail"]
        );
        assert!(result.coverage > 0.99);
        assert!(result.residual_norm < 0.01);
    }

    #[test]
    fn residual_solver_skips_high_redundancy_label_groups() {
        let config = LabelSolverConfig {
            max_selected_labels: 2,
            min_candidate_score: 0.01,
            min_refit_gain: 0.0,
            coverage_stop: 1.0,
            residual_norm_stop: 0.0,
            max_redundancy: 0.92,
            ..LabelSolverConfig::default()
        };
        let mut positive_queries = Vec::new();

        let result = resolve_label_groups_by_residual(
            &[1.0, 0.2, 0.0],
            &config,
            |query, polarity, _limit| match polarity {
                LabelAtomPolarity::Positive => {
                    positive_queries.push(query.to_vec());
                    if positive_queries.len() == 1 {
                        Ok(vec![retrieved_atom(
                            "l_backend",
                            "backend",
                            LabelAtomPolarity::Positive,
                            vec![1.0, 0.0, 0.0],
                            1.0,
                            1,
                        )])
                    } else {
                        Ok(vec![retrieved_atom(
                            "l_service",
                            "service",
                            LabelAtomPolarity::Positive,
                            vec![0.95, 0.31, 0.0],
                            1.0,
                            2,
                        )])
                    }
                }
                LabelAtomPolarity::Negative => Ok(Vec::new()),
            },
        )
        .unwrap();

        assert_eq!(
            result
                .selected_labels
                .iter()
                .map(|label| label.label_name.as_str())
                .collect::<Vec<_>>(),
            vec!["backend"]
        );
        assert!(positive_queries.len() >= 2);
        assert!(
            result
                .candidates
                .iter()
                .any(|candidate| candidate.label_name == "service")
        );
    }

    #[test]
    fn residual_solver_allows_low_redundancy_label_groups() {
        let config = LabelSolverConfig {
            max_selected_labels: 2,
            min_candidate_score: 0.01,
            min_refit_gain: 0.0,
            coverage_stop: 1.0,
            residual_norm_stop: 0.0,
            max_redundancy: 0.92,
            ..LabelSolverConfig::default()
        };

        let result = resolve_label_groups_by_residual(
            &[1.0, 1.0, 0.0],
            &config,
            |query, polarity, _limit| match polarity {
                LabelAtomPolarity::Positive => {
                    if query[0] > 0.4 {
                        Ok(vec![retrieved_atom(
                            "l_backend",
                            "backend",
                            LabelAtomPolarity::Positive,
                            vec![1.0, 0.0, 0.0],
                            1.0,
                            1,
                        )])
                    } else {
                        Ok(vec![retrieved_atom(
                            "l_docs",
                            "docs",
                            LabelAtomPolarity::Positive,
                            vec![0.0, 1.0, 0.0],
                            1.0,
                            2,
                        )])
                    }
                }
                LabelAtomPolarity::Negative => Ok(Vec::new()),
            },
        )
        .unwrap();

        assert_eq!(
            result
                .selected_labels
                .iter()
                .map(|label| label.label_name.as_str())
                .collect::<Vec<_>>(),
            vec!["backend", "docs"]
        );
        assert!(result.coverage > 0.99);
        assert!(result.residual_norm < 0.01);
    }

    #[test]
    fn residual_solver_stops_when_residual_norm_threshold_is_reached() {
        let config = LabelSolverConfig {
            max_selected_labels: 2,
            min_candidate_score: 0.01,
            min_refit_gain: 0.0,
            coverage_stop: 1.0,
            residual_norm_stop: 0.30,
            ..LabelSolverConfig::default()
        };
        let mut positive_queries = Vec::new();

        let result = resolve_label_groups_by_residual(
            &[1.0, 0.2, 0.0],
            &config,
            |query, polarity, _limit| match polarity {
                LabelAtomPolarity::Positive => {
                    positive_queries.push(query.to_vec());
                    if positive_queries.len() == 1 {
                        Ok(vec![
                            retrieved_atom(
                                "l_primary",
                                "primary",
                                LabelAtomPolarity::Positive,
                                vec![1.0, 0.0, 0.0],
                                1.0,
                                1,
                            ),
                            retrieved_atom(
                                "l_tail",
                                "tail",
                                LabelAtomPolarity::Positive,
                                vec![0.0, 1.0, 0.0],
                                1.0,
                                2,
                            ),
                        ])
                    } else {
                        Ok(vec![retrieved_atom(
                            "l_tail",
                            "tail",
                            LabelAtomPolarity::Positive,
                            vec![0.0, 1.0, 0.0],
                            1.0,
                            3,
                        )])
                    }
                }
                LabelAtomPolarity::Negative => Ok(Vec::new()),
            },
        )
        .unwrap();

        assert_eq!(positive_queries.len(), 1);
        assert_eq!(result.selected_labels.len(), 1);
        assert_eq!(result.selected_labels[0].label_name, "primary");
        assert!(result.residual_norm < 0.30);
    }

    #[test]
    fn residual_solver_stops_when_coverage_threshold_is_reached() {
        let config = LabelSolverConfig {
            max_selected_labels: 2,
            min_candidate_score: 0.01,
            min_refit_gain: 0.0,
            coverage_stop: 0.75,
            residual_norm_stop: 0.0,
            ..LabelSolverConfig::default()
        };
        let mut positive_queries = Vec::new();

        let result = resolve_label_groups_by_residual(
            &[1.0, 0.2, 0.0],
            &config,
            |query, polarity, _limit| match polarity {
                LabelAtomPolarity::Positive => {
                    positive_queries.push(query.to_vec());
                    if positive_queries.len() == 1 {
                        Ok(vec![
                            retrieved_atom(
                                "l_primary",
                                "primary",
                                LabelAtomPolarity::Positive,
                                vec![1.0, 0.0, 0.0],
                                1.0,
                                1,
                            ),
                            retrieved_atom(
                                "l_tail",
                                "tail",
                                LabelAtomPolarity::Positive,
                                vec![0.0, 1.0, 0.0],
                                1.0,
                                2,
                            ),
                        ])
                    } else {
                        Ok(vec![retrieved_atom(
                            "l_tail",
                            "tail",
                            LabelAtomPolarity::Positive,
                            vec![0.0, 1.0, 0.0],
                            1.0,
                            3,
                        )])
                    }
                }
                LabelAtomPolarity::Negative => Ok(Vec::new()),
            },
        )
        .unwrap();

        assert_eq!(positive_queries.len(), 1);
        assert_eq!(result.selected_labels.len(), 1);
        assert_eq!(result.selected_labels[0].label_name, "primary");
        assert!(result.coverage >= 0.75);
    }

    #[test]
    fn residual_solver_limits_top_positive_atoms_per_label() {
        let config = LabelSolverConfig {
            max_selected_labels: 1,
            max_positive_atoms_per_label: 2,
            min_candidate_score: 0.01,
            ..LabelSolverConfig::default()
        };

        let result = resolve_label_groups_by_residual(
            &[1.0, 0.0, 0.0],
            &config,
            |_residual, polarity, _limit| match polarity {
                LabelAtomPolarity::Positive => Ok(vec![
                    retrieved_atom(
                        "l_backend",
                        "backend",
                        LabelAtomPolarity::Positive,
                        vec![1.0, 0.0, 0.0],
                        1.0,
                        1,
                    ),
                    retrieved_atom(
                        "l_backend",
                        "backend",
                        LabelAtomPolarity::Positive,
                        vec![0.9, 0.1, 0.0],
                        0.9,
                        2,
                    ),
                    retrieved_atom(
                        "l_backend",
                        "backend",
                        LabelAtomPolarity::Positive,
                        vec![0.8, 0.2, 0.0],
                        0.8,
                        3,
                    ),
                ]),
                LabelAtomPolarity::Negative => Ok(Vec::new()),
            },
        )
        .unwrap();

        assert_eq!(result.candidates[0].evidence_atoms.len(), 2);
        assert_eq!(
            result.candidates[0].evidence_atoms[0].atom_id.as_deref(),
            Some("l_backend_1")
        );
        assert_eq!(
            result.candidates[0].evidence_atoms[1].atom_id.as_deref(),
            Some("l_backend_2")
        );
    }

    #[test]
    fn residual_solver_rejects_zero_query_and_retrieved_dimension_mismatch() {
        let config = LabelSolverConfig::default();
        assert!(matches!(
            resolve_label_groups_by_residual(&[0.0, 0.0, 0.0], &config, |_, _, _| Ok(Vec::new())),
            Err(LabelSolverError::ZeroQueryEmbedding)
        ));

        let error = resolve_label_groups_by_residual(
            &[1.0, 0.0, 0.0],
            &config,
            |_residual, polarity, _limit| match polarity {
                LabelAtomPolarity::Positive => Ok(vec![retrieved_atom(
                    "l_backend",
                    "backend",
                    LabelAtomPolarity::Positive,
                    vec![1.0, 0.0],
                    1.0,
                    1,
                )]),
                LabelAtomPolarity::Negative => Ok(Vec::new()),
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            LabelSolverError::DimensionMismatch {
                expected: 3,
                actual: 2
            }
        ));
    }

    #[test]
    fn negative_atom_suppresses_same_label_candidate() {
        let labels = vec![embedded_label(
            "l_frontend",
            "Frontend",
            vec![vec![1.0, 0.0, 0.0]],
            vec![vec![1.0, 0.0, 0.0]],
        )];
        let config = LabelSolverConfig {
            negative_suppression_factor: 1.0,
            ..LabelSolverConfig::default()
        };

        let candidates = retrieve_label_groups(&[1.0, 0.0, 0.0], &labels, &config).unwrap();

        assert!(candidates[0].suppressed);
        assert_eq!(candidates[0].score, 0.0);
        assert!(candidates[0].score < candidates[0].positive_score);
        assert_eq!(candidates[0].negative_evidence_atoms.len(), 1);
        assert_eq!(
            candidates[0].negative_evidence_atoms[0].source.polarity,
            LabelAtomPolarity::Negative
        );
        assert!(candidates[0].negative_evidence_atoms[0].similarity > 0.9);
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
        assert!(result.candidates.is_empty());
        assert_eq!(result.coverage, 0.0);
        assert_eq!(result.residual_norm, 1.0);
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

    #[test]
    fn public_semantic_label_dimension_mismatch_returns_error() {
        let def = definition("l_backend", "Backend");
        let source = def.atom_sources().remove(0);
        let labels = vec![SemanticLabel {
            definition: def,
            atoms: vec![EmbeddedLabelAtom {
                source,
                embedding: vec![1.0, 0.0],
            }],
            embedding_dimension: 3,
        }];

        assert!(matches!(
            resolve_label_groups(&[1.0, 0.0, 0.0], &labels, &LabelSolverConfig::default()),
            Err(LabelSolverError::DimensionMismatch {
                expected: 3,
                actual: 2
            })
        ));
    }

    #[test]
    fn public_semantic_label_source_mismatch_returns_error() {
        let def = definition("l_backend", "Backend");
        let mut source = def.atom_sources().remove(0);
        source.label_id = "l_frontend".to_owned();
        let labels = vec![SemanticLabel {
            definition: def,
            atoms: vec![EmbeddedLabelAtom {
                source,
                embedding: vec![1.0, 0.0, 0.0],
            }],
            embedding_dimension: 3,
        }];

        assert!(matches!(
            retrieve_label_groups(&[1.0, 0.0, 0.0], &labels, &LabelSolverConfig::default()),
            Err(LabelSolverError::LabelMismatch { expected, actual })
                if expected == "l_backend" && actual == "l_frontend"
        ));
    }

    #[test]
    fn zero_query_embedding_returns_error() {
        let labels = vec![embedded_label(
            "l_backend",
            "Backend",
            vec![vec![1.0, 0.0, 0.0]],
            vec![],
        )];

        assert!(matches!(
            resolve_label_groups(&[0.0, 0.0, 0.0], &labels, &LabelSolverConfig::default()),
            Err(LabelSolverError::ZeroQueryEmbedding)
        ));
    }
}
