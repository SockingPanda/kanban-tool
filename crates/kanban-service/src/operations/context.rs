use std::{collections::BTreeMap, future::Future};

use kanban_core::{Clock, KanbanError, Result};

use crate::{ApplicationService, ApplicationStore};

/// 上下文构建的边界，防止一次请求无限扫描 canonical 数据或派生索引。
pub const MAX_CONTEXT_LIMIT: usize = 1_000;
pub const MAX_CONTEXT_DEPTH: usize = 8;
pub const MAX_CONTEXT_BUDGET: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextBuildOptions {
    pub board: String,
    pub task: Option<String>,
    pub reference: Option<String>,
    pub query: Option<String>,
    pub depth: usize,
    pub lexical_limit: usize,
    pub graph_limit: usize,
    pub vector_limit: usize,
    pub budget: usize,
}

impl Default for ContextBuildOptions {
    fn default() -> Self {
        Self {
            board: "default".to_owned(),
            task: None,
            reference: None,
            query: None,
            depth: 1,
            lexical_limit: 5,
            graph_limit: 10,
            vector_limit: 5,
            budget: 20,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextPolicy {
    pub depth: usize,
    pub lexical_limit: usize,
    pub graph_limit: usize,
    pub vector_limit: usize,
    pub max_items: usize,
    pub budget: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextCandidate {
    pub entity_uri: String,
    pub source: String,
    pub provenance: Vec<String>,
    pub score: Option<f64>,
    pub title: Option<String>,
    pub snippet: Option<String>,
    pub reason: String,
    pub board_id: Option<String>,
    pub evidence: Vec<ContextEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextEvidence {
    pub kind: String,
    pub entity_uri: Option<String>,
    pub task_id: Option<String>,
    pub relation_id: Option<String>,
    pub predicate: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextProviderStatus {
    pub provider: String,
    pub capability: String,
    pub available: bool,
    pub degraded: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextDiagnostic {
    pub source: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextSources {
    pub board_id: String,
    pub subject: ContextCandidate,
    pub lexical: Vec<ContextCandidate>,
    pub graph: Vec<ContextCandidate>,
    pub vector: Vec<ContextCandidate>,
    pub providers: Vec<ContextProviderStatus>,
    pub degraded: Vec<String>,
    pub diagnostics: Vec<ContextDiagnostic>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextItem {
    pub entity_uri: String,
    pub source: String,
    pub provenance: Vec<String>,
    pub score: Option<f64>,
    pub rank: usize,
    pub reason: String,
    pub title: Option<String>,
    pub snippet: Option<String>,
    pub evidence: Vec<ContextEvidence>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextPack {
    pub subject: String,
    pub policy: ContextPolicy,
    pub items: Vec<ContextItem>,
    pub providers: Vec<ContextProviderStatus>,
    pub degraded: Vec<String>,
    pub diagnostics: Vec<ContextDiagnostic>,
    pub truncated: bool,
    pub truncation_reason: Option<String>,
}

/// Context retrieval is a read-only application capability.  Implementations
/// may use canonical facts and rebuildable providers, but must never mutate
/// those facts while satisfying the request.
pub trait ContextBuild: ApplicationStore {
    fn context_sources(
        &self,
        options: ContextBuildOptions,
    ) -> impl Future<Output = Result<ContextSources>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: ContextBuild,
    C: Clock,
{
    pub async fn build_context(&self, options: ContextBuildOptions) -> Result<ContextPack> {
        let options = normalize_options(options)?;
        let sources = self.store.context_sources(options.clone()).await?;
        merge_context_sources(options, sources)
    }
}

fn normalize_options(mut options: ContextBuildOptions) -> Result<ContextBuildOptions> {
    options.board = options.board.trim().to_owned();
    if options.board.is_empty() {
        return Err(KanbanError::InvalidInput("board is required".to_owned()));
    }
    for (name, value) in [
        ("lexical_limit", options.lexical_limit),
        ("graph_limit", options.graph_limit),
        ("vector_limit", options.vector_limit),
        ("budget", options.budget),
    ] {
        if value == 0 || value > MAX_CONTEXT_LIMIT {
            return Err(KanbanError::InvalidInput(format!(
                "{name} must be between 1 and {MAX_CONTEXT_LIMIT}"
            )));
        }
    }
    if options.depth > MAX_CONTEXT_DEPTH {
        return Err(KanbanError::InvalidInput(format!(
            "depth must be <= {MAX_CONTEXT_DEPTH}"
        )));
    }
    if options.budget > MAX_CONTEXT_BUDGET {
        return Err(KanbanError::InvalidInput(format!(
            "budget must be <= {MAX_CONTEXT_BUDGET}"
        )));
    }
    options.task = normalize_selector(options.task);
    options.reference = normalize_selector(options.reference);
    options.query = normalize_selector(options.query);
    if options.task.is_none() && options.reference.is_none() && options.query.is_none() {
        return Err(KanbanError::InvalidInput(
            "one of task, reference or query is required".to_owned(),
        ));
    }
    Ok(options)
}

fn normalize_selector(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

fn merge_context_sources(
    options: ContextBuildOptions,
    sources: ContextSources,
) -> Result<ContextPack> {
    if sources.board_id.trim().is_empty() {
        return Err(KanbanError::Storage(
            "context provider returned an empty board id".to_owned(),
        ));
    }
    let policy = ContextPolicy {
        depth: options.depth,
        lexical_limit: options.lexical_limit,
        graph_limit: options.graph_limit,
        vector_limit: options.vector_limit,
        max_items: options.budget,
        budget: options.budget,
    };

    let mut degraded = Vec::new();
    for marker in sources.degraded {
        push_unique(&mut degraded, marker);
    }
    let mut diagnostics = sources.diagnostics;
    let mut merged = BTreeMap::<String, ContextItem>::new();
    let mut order = Vec::<String>::new();

    let mut add = |candidate: ContextCandidate| {
        if candidate
            .board_id
            .as_deref()
            .is_some_and(|board| board != sources.board_id)
        {
            // Providers are untrusted projections from the point of view of
            // board isolation.  A mismatched candidate is dropped silently;
            // the provider status/diagnostic remains auditable.
            return;
        }
        let key = candidate.entity_uri.clone();
        if let Some(existing) = merged.get_mut(&key) {
            merge_candidate(existing, candidate);
            return;
        }
        order.push(key.clone());
        merged.insert(
            key,
            ContextItem {
                entity_uri: candidate.entity_uri,
                source: candidate.source,
                provenance: candidate.provenance,
                score: candidate.score,
                rank: 0,
                reason: candidate.reason,
                title: candidate.title,
                snippet: candidate.snippet,
                evidence: candidate.evidence,
            },
        );
    };

    add(sources.subject);
    for candidate in sources.lexical.into_iter().take(options.lexical_limit) {
        add(candidate);
    }
    for candidate in sources.graph.into_iter().take(options.graph_limit) {
        add(candidate);
    }
    for candidate in sources.vector.into_iter().take(options.vector_limit) {
        add(candidate);
    }

    let mut items = order
        .into_iter()
        .filter_map(|key| merged.remove(&key))
        .collect::<Vec<_>>();
    let mut truncated = sources.truncated;
    let truncation_reason = if items.len() > options.budget {
        truncated = true;
        Some("budget".to_owned())
    } else if truncated {
        Some("provider_limit".to_owned())
    } else {
        None
    };
    items.truncate(options.budget);
    for (index, item) in items.iter_mut().enumerate() {
        item.rank = index + 1;
    }

    // A provider can signal degradation without requiring a hard failure.  A
    // diagnostic is bounded at the adapter boundary; keep only stable codes
    // here so clients can branch without parsing messages.
    for provider in &sources.providers {
        if provider.degraded {
            push_unique(&mut degraded, format!("{}_degraded", provider.provider));
            if let Some(reason) = provider.reason.as_deref() {
                diagnostics.push(ContextDiagnostic {
                    source: provider.provider.clone(),
                    code: "provider_degraded".to_owned(),
                    message: reason.to_owned(),
                });
            }
        }
    }

    Ok(ContextPack {
        subject: items
            .first()
            .map(|item| item.entity_uri.clone())
            .unwrap_or_default(),
        policy,
        items,
        providers: sources.providers,
        degraded,
        diagnostics,
        truncated,
        truncation_reason,
    })
}

fn merge_candidate(existing: &mut ContextItem, candidate: ContextCandidate) {
    if existing.source != candidate.source
        && !existing
            .provenance
            .iter()
            .any(|value| value == &candidate.source)
    {
        existing.provenance.push(candidate.source.clone());
    }
    for value in candidate.provenance {
        push_unique(&mut existing.provenance, value);
    }
    for value in candidate.evidence {
        if !existing.evidence.contains(&value) {
            existing.evidence.push(value);
        }
    }
    if existing.title.is_none() {
        existing.title = candidate.title;
    }
    if existing.snippet.is_none() {
        existing.snippet = candidate.snippet;
    }
    if existing.score.is_none() || candidate.score > existing.score {
        existing.score = candidate.score;
    }
    if existing.reason.is_empty() {
        existing.reason = candidate.reason;
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::test_support::{FixedClock, StubStore};
    use std::sync::{Arc, atomic::AtomicUsize};

    impl ContextBuild for StubStore {
        async fn context_sources(&self, _options: ContextBuildOptions) -> Result<ContextSources> {
            Ok(ContextSources {
                board_id: "b_default".to_owned(),
                subject: ContextCandidate {
                    entity_uri: "kb://task/t_subject".to_owned(),
                    source: "subject".to_owned(),
                    provenance: vec!["canonical:tasks".to_owned()],
                    score: None,
                    title: Some("Subject".to_owned()),
                    snippet: None,
                    reason: "subject".to_owned(),
                    board_id: Some("b_default".to_owned()),
                    evidence: vec![ContextEvidence {
                        kind: "task".to_owned(),
                        entity_uri: Some("kb://task/t_subject".to_owned()),
                        task_id: Some("t_subject".to_owned()),
                        relation_id: None,
                        predicate: None,
                        summary: None,
                    }],
                },
                lexical: vec![ContextCandidate {
                    entity_uri: "kb://task/t_lexical".to_owned(),
                    source: "lexical".to_owned(),
                    provenance: vec!["turso_fts".to_owned()],
                    score: Some(0.8),
                    title: None,
                    snippet: Some("lexical".to_owned()),
                    reason: "lexical_match".to_owned(),
                    board_id: Some("b_default".to_owned()),
                    evidence: Vec::new(),
                }],
                graph: vec![ContextCandidate {
                    entity_uri: "kb://task/t_graph".to_owned(),
                    source: "graph".to_owned(),
                    provenance: vec!["canonical:relations".to_owned()],
                    score: None,
                    title: None,
                    snippet: Some("depends_on".to_owned()),
                    reason: "graph_neighbor".to_owned(),
                    board_id: Some("b_default".to_owned()),
                    evidence: Vec::new(),
                }],
                vector: vec![ContextCandidate {
                    entity_uri: "kb://task/t_lexical".to_owned(),
                    source: "vector".to_owned(),
                    provenance: vec!["turso_vector32".to_owned()],
                    score: Some(0.9),
                    title: Some("semantic".to_owned()),
                    snippet: None,
                    reason: "semantic_match".to_owned(),
                    board_id: Some("b_default".to_owned()),
                    evidence: Vec::new(),
                }],
                providers: vec![],
                degraded: vec![],
                diagnostics: vec![],
                truncated: false,
            })
        }
    }

    fn service() -> ApplicationService<StubStore, FixedClock> {
        ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        )
    }

    #[tokio::test]
    async fn context_merge_is_stable_deduplicated_and_ranked() {
        let pack = service()
            .build_context(ContextBuildOptions {
                board: "default".to_owned(),
                task: Some("t_subject".to_owned()),
                budget: 10,
                ..ContextBuildOptions::default()
            })
            .await
            .unwrap();
        assert_eq!(pack.items.len(), 3);
        assert_eq!(pack.items[0].entity_uri, "kb://task/t_subject");
        assert_eq!(pack.items[1].entity_uri, "kb://task/t_lexical");
        assert_eq!(
            pack.items[1].provenance,
            vec!["turso_fts", "vector", "turso_vector32"]
        );
        assert_eq!(pack.items[1].rank, 2);
        assert_eq!(pack.items[2].rank, 3);
    }

    #[tokio::test]
    async fn context_merge_enforces_budget_and_board_isolation() {
        let mut sources = service()
            .store
            .context_sources(ContextBuildOptions::default())
            .await
            .unwrap();
        sources.lexical.push(ContextCandidate {
            entity_uri: "kb://task/t_foreign".to_owned(),
            source: "lexical".to_owned(),
            provenance: Vec::new(),
            score: Some(1.0),
            title: None,
            snippet: None,
            reason: "lexical_match".to_owned(),
            board_id: Some("b_other".to_owned()),
            evidence: Vec::new(),
        });
        let result = merge_context_sources(
            ContextBuildOptions {
                budget: 2,
                ..ContextBuildOptions::default()
            },
            sources,
        )
        .unwrap();
        assert_eq!(result.items.len(), 2);
        assert!(
            result
                .items
                .iter()
                .all(|item| item.entity_uri != "kb://task/t_foreign")
        );
        assert!(result.truncated);
        assert_eq!(result.truncation_reason.as_deref(), Some("budget"));
    }
}
