//! service-private 的语义标签建议 solver。
//!
//! 该模块只处理已经从 canonical Turso projection 读取的 atom/vector 快照。
//! 它不读写数据库，也不调用 provider，因此可以被 projection bootstrap 和
//! 在线建议路径复用。检索边界由调用方通过闭包提供，solver 负责残差迭代、
//! 正负 evidence、非负 refit 和 coverage 指标。

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomPolarity {
    Positive,
    Negative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomKind {
    Name,
    Description,
    AppliesWhen,
    PositiveExample,
    ExcludesWhen,
    NegativeExample,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RetrievedAtom {
    pub atom_id: String,
    pub label_id: String,
    pub label_name: String,
    pub polarity: AtomPolarity,
    pub kind: AtomKind,
    pub text: String,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Evidence {
    pub atom_id: String,
    pub label_id: String,
    pub label_name: String,
    pub polarity: AtomPolarity,
    pub kind: AtomKind,
    pub text: String,
    pub similarity: f32,
    pub contribution: f32,
    vector: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Candidate {
    pub label_id: String,
    pub label_name: String,
    pub score: f32,
    pub positive_score: f32,
    pub negative_score: f32,
    pub suppressed: bool,
    pub evidence_atoms: Vec<Evidence>,
    pub negative_evidence_atoms: Vec<Evidence>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SelectedLabel {
    pub label_id: String,
    pub label_name: String,
    pub weight: f32,
    pub score: f32,
    pub evidence_atoms: Vec<Evidence>,
    pub negative_evidence_atoms: Vec<Evidence>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SolverConfig {
    pub max_candidates: usize,
    pub max_selected_labels: usize,
    /// 每轮 residual 检索时保留的 polarity-specific top hits。
    pub retrieval_limit: usize,
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

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            max_candidates: 8,
            max_selected_labels: 3,
            retrieval_limit: 80,
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SolverResult {
    pub candidates: Vec<Candidate>,
    pub selected_labels: Vec<SelectedLabel>,
    pub coverage: f32,
    pub coverage_cosine: f32,
    pub residual_norm: f32,
    pub needs_new_label: bool,
}

/// 按 residual 轮次检索并选择多标签。
pub(crate) fn resolve_by_residual<R>(
    query_embedding: &[f32],
    config: &SolverConfig,
    mut retrieve: R,
) -> Result<SolverResult, SolverError>
where
    R: FnMut(&[f32], AtomPolarity, usize) -> Result<Vec<RetrievedAtom>, SolverError>,
{
    validate_inputs(query_embedding, config)?;
    let query_norm = l2_norm(query_embedding);
    let normalized_query = normalize_for_fit(query_embedding);
    let mut residual = normalize_for_fit(query_embedding);
    let mut selected_label_ids = HashSet::new();
    let mut selected_vectors = Vec::new();
    let mut selected_group_vectors: Vec<Vec<f32>> = Vec::new();
    let mut selected_basis = Vec::new();
    let mut candidates_by_label: HashMap<String, Candidate> = HashMap::new();
    let retrieval_limit = config.retrieval_limit.max(1);

    while selected_label_ids.len() < config.max_selected_labels {
        let residual_norm = l2_norm(&residual);
        if residual_norm <= f32::EPSILON || selection_stop_reached(residual_norm, config) {
            break;
        }
        let residual_query = normalize_for_fit(&residual);
        let positive_hits = retrieve(&residual_query, AtomPolarity::Positive, retrieval_limit)?;
        let negative_hits = retrieve(&normalized_query, AtomPolarity::Negative, retrieval_limit)?;
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
    let _residual = fit_residual(query_embedding, &selected_vectors, &weights, query_norm);
    let residual_norm = normalized_residual_norm(query_embedding, &fitted);
    let coverage = coverage_from_residual(residual_norm);
    let coverage_cosine = coverage_cosine(query_embedding, &fitted);
    let selected_labels = selected_labels_from_basis(&selected_basis, &weights);
    let mut candidates = candidates_by_label.into_values().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.label_name.cmp(&right.label_name))
    });
    candidates.truncate(config.max_candidates);
    Ok(SolverResult {
        candidates,
        selected_labels,
        coverage,
        coverage_cosine,
        residual_norm,
        needs_new_label: coverage < config.min_coverage || residual_norm > config.max_residual_norm,
    })
}

/// 使用已经 staged 到内存的 atom/vector 快照执行建议。
///
/// 每一轮仍按 residual 重新计算 cosine 并只把 polarity/limit 对应的 top hits
/// 交给同一个 solver，因此 bootstrap 可以复用这条路径而不复制选择公式。
pub(crate) fn resolve_from_atoms(
    query_embedding: &[f32],
    config: &SolverConfig,
    atoms: &[RetrievedAtom],
) -> Result<SolverResult, SolverError> {
    resolve_by_residual(query_embedding, config, |query, polarity, limit| {
        let mut hits = atoms
            .iter()
            .filter(|atom| atom.polarity == polarity)
            .filter_map(|atom| {
                let similarity = cosine_similarity(query, &atom.vector);
                (similarity > 0.0).then_some((similarity, atom))
            })
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| {
            right
                .0
                .total_cmp(&left.0)
                .then_with(|| left.1.atom_id.cmp(&right.1.atom_id))
        });
        Ok(hits
            .into_iter()
            .take(limit)
            .map(|(_similarity, atom)| atom.clone())
            .collect())
    })
}

#[derive(Debug, Clone)]
struct SelectedBasisAtom {
    label_id: String,
    label_name: String,
    score: f32,
    evidence: Evidence,
    negative_evidence_atoms: Vec<Evidence>,
}

fn validate_inputs(query_embedding: &[f32], config: &SolverConfig) -> Result<(), SolverError> {
    validate_selection_config(config)?;
    if l2_norm(query_embedding) == 0.0 {
        return Err(SolverError::ZeroQueryEmbedding);
    }
    if config.max_positive_atoms_per_label == 0 {
        return Err(SolverError::InvalidConfig(
            "max_positive_atoms_per_label 必须 >= 1".to_owned(),
        ));
    }
    if config.max_candidates == 0 || config.max_selected_labels == 0 || config.retrieval_limit == 0
    {
        return Err(SolverError::InvalidConfig(
            "max_candidates/max_selected_labels/retrieval_limit 必须 >= 1".to_owned(),
        ));
    }
    if !(0.0..=1.0).contains(&config.min_candidate_score)
        || !(0.0..=1.0).contains(&config.min_evidence_score)
    {
        return Err(SolverError::InvalidConfig(
            "candidate/evidence score 必须在 0 到 1 之间".to_owned(),
        ));
    }
    Ok(())
}

fn validate_selection_config(config: &SolverConfig) -> Result<(), SolverError> {
    if config.min_refit_gain < 0.0 {
        return Err(SolverError::InvalidConfig(
            "min_refit_gain 必须 >= 0".to_owned(),
        ));
    }
    if !(0.0..=1.0).contains(&config.coverage_stop) {
        return Err(SolverError::InvalidConfig(
            "coverage_stop 必须在 0 到 1 之间".to_owned(),
        ));
    }
    if config.residual_norm_stop < 0.0 {
        return Err(SolverError::InvalidConfig(
            "residual_norm_stop 必须 >= 0".to_owned(),
        ));
    }
    if !(0.0..=1.0).contains(&config.max_redundancy) {
        return Err(SolverError::InvalidConfig(
            "max_redundancy 必须在 0 到 1 之间".to_owned(),
        ));
    }
    Ok(())
}

fn residual_candidates(
    positive_query: &[f32],
    negative_query: &[f32],
    positive_hits: Vec<RetrievedAtom>,
    negative_hits: Vec<RetrievedAtom>,
    selected_label_ids: &HashSet<String>,
    config: &SolverConfig,
) -> Result<Vec<Candidate>, SolverError> {
    let mut positives_by_label: HashMap<String, Vec<RetrievedAtom>> = HashMap::new();
    for hit in positive_hits {
        validate_retrieved_atom(&hit, positive_query.len(), AtomPolarity::Positive)?;
        if !selected_label_ids.contains(&hit.label_id) {
            positives_by_label
                .entry(hit.label_id.clone())
                .or_default()
                .push(hit);
        }
    }
    let mut negatives_by_label: HashMap<String, Vec<RetrievedAtom>> = HashMap::new();
    for hit in negative_hits {
        validate_retrieved_atom(&hit, negative_query.len(), AtomPolarity::Negative)?;
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
                .then_with(|| left.text.cmp(&right.text))
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
                .then_with(|| left.text.cmp(&right.text))
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
            candidates.push(Candidate {
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

fn candidate_basis_atoms(candidate: &Candidate) -> Vec<(Evidence, Vec<f32>)> {
    candidate
        .evidence_atoms
        .iter()
        .map(|evidence| {
            let mut vector = evidence_vector(evidence);
            normalize_in_place(&mut vector);
            (evidence.clone(), vector)
        })
        .collect()
}

fn evidence_vector(evidence: &Evidence) -> Vec<f32> {
    evidence.vector.clone()
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
                .then_with(|| left.text.cmp(&right.text))
        });
    }
    by_label
}

fn validate_retrieved_atom(
    atom: &RetrievedAtom,
    expected_dimension: usize,
    expected_polarity: AtomPolarity,
) -> Result<(), SolverError> {
    if atom.polarity != expected_polarity {
        return Err(SolverError::RetrievedPolarityMismatch {
            expected: expected_polarity,
            actual: atom.polarity,
        });
    }
    ensure_dimension(atom.vector.len(), expected_dimension)
}

fn atom_evidence(atom: &RetrievedAtom, residual: &[f32]) -> Evidence {
    let similarity = retrieved_similarity(residual, atom);
    Evidence {
        atom_id: atom.atom_id.clone(),
        label_id: atom.label_id.clone(),
        label_name: atom.label_name.clone(),
        polarity: atom.polarity,
        kind: atom.kind,
        text: atom.text.clone(),
        similarity,
        contribution: similarity,
        vector: atom.vector.clone(),
    }
}

fn retrieved_similarity(query: &[f32], atom: &RetrievedAtom) -> f32 {
    cosine_similarity(query, &atom.vector).max(0.0)
}

fn candidate_group_vector(candidate: &Candidate) -> Option<Vec<f32>> {
    let dimension = candidate
        .evidence_atoms
        .iter()
        .map(|evidence| evidence_vector(evidence).len())
        .find(|dimension| *dimension > 0)?;
    let mut vector = vec![0.0; dimension];
    let mut total_weight = 0.0_f32;
    for evidence in &candidate.evidence_atoms {
        let atom_vector = evidence_vector(evidence);
        if atom_vector.len() != dimension {
            continue;
        }
        let weight = evidence.contribution.max(0.0);
        add_scaled(&mut vector, &atom_vector, weight);
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

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SolverError {
    DimensionMismatch {
        expected: usize,
        actual: usize,
    },
    ZeroQueryEmbedding,
    InvalidConfig(String),
    RetrievedPolarityMismatch {
        expected: AtomPolarity,
        actual: AtomPolarity,
    },
}

impl std::fmt::Display for SolverError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DimensionMismatch { expected, actual } => {
                write!(
                    formatter,
                    "embedding 维度不匹配：期望 {expected}，实际 {actual}"
                )
            }
            Self::ZeroQueryEmbedding => formatter.write_str("query embedding 没有语义信号"),
            Self::InvalidConfig(message) => {
                write!(formatter, "标签 solver 配置无效：{message}")
            }
            Self::RetrievedPolarityMismatch { expected, actual } => write!(
                formatter,
                "检索 atom polarity 不匹配：期望 {expected:?}，实际 {actual:?}"
            ),
        }
    }
}

impl std::error::Error for SolverError {}

fn ensure_dimension(actual: usize, expected: usize) -> Result<(), SolverError> {
    if actual == expected {
        Ok(())
    } else {
        Err(SolverError::DimensionMismatch { expected, actual })
    }
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let left_norm = l2_norm(left);
    let right_norm = l2_norm(right);
    if left_norm == 0.0 || right_norm == 0.0 {
        return 0.0;
    }
    dot(left, right) / (left_norm * right_norm)
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

fn coverage_cosine(query_embedding: &[f32], fitted: &[f32]) -> f32 {
    cosine_similarity(query_embedding, fitted)
        .max(0.0)
        .clamp(0.0, 1.0)
}

fn selection_stop_reached(residual_norm: f32, config: &SolverConfig) -> bool {
    residual_norm <= config.residual_norm_stop
        || coverage_from_residual(residual_norm) >= config.coverage_stop
}

fn refit_gain(old_residual_norm: f32, new_residual_norm: f32) -> f32 {
    (old_residual_norm - new_residual_norm).max(0.0)
}

fn refit_gain_is_sufficient(gain: f32, config: &SolverConfig) -> bool {
    gain + f32::EPSILON >= config.min_refit_gain
}

fn redundancy_exceeds(
    vector: &[f32],
    selected_group_vectors: &[Vec<f32>],
    config: &SolverConfig,
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
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

fn l2_norm(vector: &[f32]) -> f32 {
    dot(vector, vector).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(
        label_id: &str,
        label_name: &str,
        polarity: AtomPolarity,
        vector: Vec<f32>,
    ) -> RetrievedAtom {
        RetrievedAtom {
            atom_id: format!("{label_id}_atom"),
            label_id: label_id.to_owned(),
            label_name: label_name.to_owned(),
            polarity,
            kind: AtomKind::AppliesWhen,
            text: label_name.to_owned(),
            vector,
        }
    }

    #[test]
    fn residual_solver_selects_orthogonal_labels_and_reports_coverage() {
        let mut calls = 0;
        let result = resolve_by_residual(
            &[1.0, 1.0, 0.0],
            &SolverConfig {
                max_selected_labels: 2,
                min_candidate_score: 0.01,
                min_refit_gain: 0.0,
                coverage_stop: 1.0,
                residual_norm_stop: 0.0,
                ..SolverConfig::default()
            },
            |_query, polarity, _limit| {
                if polarity == AtomPolarity::Negative {
                    return Ok(Vec::new());
                }
                calls += 1;
                if calls == 1 {
                    Ok(vec![
                        atom(
                            "l_backend",
                            "backend",
                            AtomPolarity::Positive,
                            vec![1.0, 0.0, 0.0],
                        ),
                        atom(
                            "l_docs",
                            "docs",
                            AtomPolarity::Positive,
                            vec![0.0, 1.0, 0.0],
                        ),
                    ])
                } else {
                    Ok(vec![atom(
                        "l_docs",
                        "docs",
                        AtomPolarity::Positive,
                        vec![0.0, 1.0, 0.0],
                    )])
                }
            },
        )
        .expect("solver result");

        assert_eq!(
            result
                .selected_labels
                .iter()
                .map(|label| label.label_name.as_str())
                .collect::<Vec<_>>(),
            vec!["backend", "docs"]
        );
        assert!(result.coverage > 0.99);
        assert!(result.coverage_cosine > 0.99);
        assert!(result.residual_norm < 0.01);
    }
}
