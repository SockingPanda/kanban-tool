use std::collections::BTreeMap;

use kanban_core::{Clock, KanbanError, Result};

use crate::KanbanService;
use crate::store_operations::StoreTaskNeighborhoodOptions;
use crate::store_operations::search::{StoreSearchQuery, StoreSearchResults};

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

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn build_context(&self, options: ContextBuildOptions) -> Result<ContextPack> {
        let options = normalize_options(options)?;
        let sources = self.context_sources(options.clone()).await?;
        merge_context_sources(options, sources)
    }

    async fn context_sources(&self, options: ContextBuildOptions) -> Result<ContextSources> {
        let store = &self.store;
        let board_id = store
            .vector_board_id(&options.board)
            .await
            .map_err(crate::error::store_error)?;

        let initial_query = options
            .query
            .clone()
            .or_else(|| options.reference.clone())
            .or_else(|| options.task.clone());
        let initial_results = search(
            store,
            &options.board,
            initial_query.as_deref(),
            options.lexical_limit.max(1),
        )
        .await?;
        let subject = resolve_subject(store, &board_id, &options, initial_results.as_ref()).await?;
        let lexical_query = options
            .query
            .clone()
            .unwrap_or_else(|| subject.title.clone());
        let lexical_results = search(
            store,
            &options.board,
            Some(&lexical_query),
            options.lexical_limit,
        )
        .await?;

        let mut degraded = Vec::new();
        let mut diagnostics = Vec::new();
        let lexical = lexical_candidates(
            store,
            lexical_results.as_ref(),
            &board_id,
            &mut degraded,
            &mut diagnostics,
        )
        .await?;
        let lexical_provider = lexical_provider(lexical_results.as_ref());

        let (graph, graph_provider) = graph_candidates(
            store,
            &subject.id,
            &board_id,
            options.depth,
            options.graph_limit,
            &mut degraded,
            &mut diagnostics,
        )
        .await?;

        let (vector, vector_provider) = vector_candidates(
            application_store,
            &lexical_query,
            &board_id,
            options.vector_limit,
            &mut degraded,
            &mut diagnostics,
        )
        .await?;

        Ok(ContextSources {
            board_id: board_id.clone(),
            subject: ContextCandidate {
                entity_uri: format!("kb://task/{}", subject.id),
                source: "subject".to_owned(),
                provenance: vec!["canonical:tasks".to_owned()],
                score: None,
                title: Some(subject.title.clone()),
                snippet: subject.description.clone(),
                reason: "subject".to_owned(),
                board_id: Some(board_id),
                evidence: vec![ContextEvidence {
                    kind: "task".to_owned(),
                    entity_uri: Some(format!("kb://task/{}", subject.id)),
                    task_id: Some(subject.id),
                    relation_id: None,
                    predicate: None,
                    summary: None,
                }],
            },
            lexical,
            graph,
            vector,
            providers: vec![lexical_provider, graph_provider, vector_provider],
            degraded,
            diagnostics,
            truncated: lexical_results
                .as_ref()
                .is_some_and(|results| results.hits.len() > options.lexical_limit),
        })
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

async fn search(
    store: &crate::TursoStore,
    board: &str,
    query: Option<&str>,
    limit: usize,
) -> Result<Option<StoreSearchResults>> {
    let Some(query) = query.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    store
        .search_tasks(StoreSearchQuery {
            board: board.to_owned(),
            q: Some(query.to_owned()),
            statuses: Vec::new(),
            labels: Vec::new(),
            assignee: None,
            include_archived: true,
            limit,
            offset: 0,
        })
        .await
        .map(Some)
        .map_err(crate::error::store_error)
}

async fn resolve_subject(
    store: &crate::TursoStore,
    board_id: &str,
    options: &ContextBuildOptions,
    initial_results: Option<&StoreSearchResults>,
) -> Result<crate::domain::TaskRecord> {
    if let Some(task) = options
        .task
        .as_deref()
        .filter(|value| value.starts_with("t_"))
    {
        let value = store
            .get_task_global(task)
            .await
            .map_err(crate::error::store_error)?;
        return ensure_board(value, board_id);
    }
    let selector = options.reference.as_deref().or(options.task.as_deref());
    let results = if selector.is_some() {
        search(store, &options.board, selector, 1).await?
    } else {
        initial_results.cloned()
    };
    let hit = results
        .and_then(|results| results.hits.into_iter().next())
        .ok_or_else(|| {
            KanbanError::NotFound(
                selector
                    .map(|value| format!("task {value}"))
                    .or_else(|| options.query.clone().map(|value| format!("query {value}")))
                    .unwrap_or_else(|| "context subject".to_owned()),
            )
        })?;
    let value = store
        .get_task_global(&hit.task_id)
        .await
        .map_err(crate::error::store_error)?;
    ensure_board(value, board_id)
}

fn ensure_board(
    task: crate::domain::TaskRecord,
    board_id: &str,
) -> Result<crate::domain::TaskRecord> {
    if task.board_id != board_id {
        return Err(KanbanError::NotFound("context subject".to_owned()));
    }
    Ok(task)
}

async fn lexical_candidates(
    store: &crate::TursoStore,
    results: Option<&StoreSearchResults>,
    board_id: &str,
    degraded: &mut Vec<String>,
    diagnostics: &mut Vec<ContextDiagnostic>,
) -> Result<Vec<ContextCandidate>> {
    let Some(results) = results else {
        return Ok(Vec::new());
    };
    if results.meta.backend != "turso_fts" {
        push_unique(degraded, "lexical_fallback");
    }
    if results.meta.stale {
        push_unique(degraded, "lexical_stale");
    }
    if let Some(reason) = results.meta.fallback_reason.as_deref() {
        diagnostics.push(ContextDiagnostic {
            source: "lexical".to_owned(),
            code: "provider_degraded".to_owned(),
            message: reason.to_owned(),
        });
    }
    let mut candidates = Vec::with_capacity(results.hits.len());
    for hit in &results.hits {
        let Ok(task) = store
            .get_task_global(&hit.task_id)
            .await
            .map_err(crate::error::store_error)
        else {
            continue;
        };
        if task.board_id != board_id {
            continue;
        }
        candidates.push(ContextCandidate {
            entity_uri: format!("kb://task/{}", task.id),
            source: "lexical".to_owned(),
            provenance: vec![format!("lexical:{}", results.meta.backend)],
            score: Some(hit.score),
            title: Some(task.title),
            snippet: hit.snippet.clone().or(task.description),
            reason: "lexical_match".to_owned(),
            board_id: Some(board_id.to_owned()),
            evidence: vec![ContextEvidence {
                kind: "task".to_owned(),
                entity_uri: Some(format!("kb://task/{}", hit.task_id)),
                task_id: Some(hit.task_id.clone()),
                relation_id: None,
                predicate: None,
                summary: Some("lexical task match".to_owned()),
            }],
        });
    }
    Ok(candidates)
}

fn lexical_provider(results: Option<&StoreSearchResults>) -> ContextProviderStatus {
    let Some(results) = results else {
        return ContextProviderStatus {
            provider: "lexical".to_owned(),
            capability: "turso_fts".to_owned(),
            available: false,
            degraded: true,
            reason: Some("query_empty".to_owned()),
        };
    };
    let degraded = results.meta.backend != "turso_fts" || results.meta.stale;
    ContextProviderStatus {
        provider: "lexical".to_owned(),
        capability: results.meta.backend.clone(),
        available: true,
        degraded,
        reason: results.meta.fallback_reason.clone(),
    }
}

async fn graph_candidates(
    store: &crate::TursoStore,
    task_id: &str,
    board_id: &str,
    depth: usize,
    limit: usize,
    degraded: &mut Vec<String>,
    diagnostics: &mut Vec<ContextDiagnostic>,
) -> Result<(Vec<ContextCandidate>, ContextProviderStatus)> {
    let status = store.graph_status(board_id).await;
    let graph_status = match status {
        Ok(status) => status,
        Err(error) => {
            push_unique(degraded, "graph_unavailable");
            diagnostics.push(ContextDiagnostic {
                source: "graph".to_owned(),
                code: "provider_error".to_owned(),
                message: bounded_message(&format!("{error:?}")),
            });
            return Ok((
                Vec::new(),
                ContextProviderStatus {
                    provider: "graph".to_owned(),
                    capability: "canonical_relations_bfs".to_owned(),
                    available: false,
                    degraded: true,
                    reason: Some("status_error".to_owned()),
                },
            ));
        }
    };
    let neighborhood = match store
        .task_neighborhood(
            task_id,
            StoreTaskNeighborhoodOptions {
                depth,
                limit_nodes: limit,
                include_archived_context: false,
            },
        )
        .await
    {
        Ok(value) => value,
        Err(error) => {
            push_unique(degraded, "graph_unavailable");
            diagnostics.push(ContextDiagnostic {
                source: "graph".to_owned(),
                code: "provider_error".to_owned(),
                message: bounded_message(&format!("{error:?}")),
            });
            return Ok((
                Vec::new(),
                graph_provider_from_status(&graph_status, Some("query_error")),
            ));
        }
    };
    let mut candidates = Vec::new();
    for node in neighborhood.nodes {
        if node.task.id == task_id || node.task.board_id != board_id {
            continue;
        }
        let evidence = neighborhood
            .edges
            .iter()
            .filter(|edge| {
                (edge.source_task_id == task_id && edge.target_task_id == node.task.id)
                    || (edge.target_task_id == task_id && edge.source_task_id == node.task.id)
            })
            .map(|edge| ContextEvidence {
                kind: "relation".to_owned(),
                entity_uri: Some(format!("kb://task/{}", node.task.id)),
                task_id: Some(node.task.id.clone()),
                relation_id: Some(edge.id.clone()),
                predicate: Some(edge.kind.clone()),
                summary: Some(format!("{} relation", edge.kind)),
            })
            .collect::<Vec<_>>();
        candidates.push(ContextCandidate {
            entity_uri: format!("kb://task/{}", node.task.id),
            source: "graph".to_owned(),
            provenance: vec!["graph:canonical_relations".to_owned()],
            score: None,
            title: Some(node.task.title),
            snippet: node.task.description,
            reason: "graph_neighbor".to_owned(),
            board_id: Some(board_id.to_owned()),
            evidence,
        });
    }
    Ok((candidates, graph_provider_from_status(&graph_status, None)))
}

fn graph_provider_from_status(
    status: &crate::domain::GraphStatusRecord,
    error: Option<&str>,
) -> ContextProviderStatus {
    ContextProviderStatus {
        provider: "graph".to_owned(),
        capability: "canonical_relations_bfs".to_owned(),
        available: status.enabled && error.is_none(),
        degraded: error.is_some() || status.projection.dirty,
        reason: error
            .map(ToOwned::to_owned)
            .or_else(|| status.projection.last_error.clone()),
    }
}

async fn vector_candidates(
    store: &crate::TursoStore,
    query: &str,
    board_id: &str,
    limit: usize,
    degraded: &mut Vec<String>,
    diagnostics: &mut Vec<ContextDiagnostic>,
) -> Result<(Vec<ContextCandidate>, ContextProviderStatus)> {
    let status = match store.vector_status(Some(board_id)).await {
        Ok(status) => status,
        Err(error) => {
            push_unique(degraded, "vector_unavailable");
            diagnostics.push(ContextDiagnostic {
                source: "vector".to_owned(),
                code: "provider_error".to_owned(),
                message: bounded_message(&error.to_string()),
            });
            return Ok((
                Vec::new(),
                ContextProviderStatus {
                    provider: "vector".to_owned(),
                    capability: "turso_vector32_ollama".to_owned(),
                    available: false,
                    degraded: true,
                    reason: Some("status_error".to_owned()),
                },
            ));
        }
    };
    if store
        .vector_config()
        .await
        .map_err(crate::error::store_error)?
        .is_none()
    {
        push_unique(degraded, "vector_disabled");
        return Ok((
            Vec::new(),
            ContextProviderStatus {
                provider: "vector".to_owned(),
                capability: "turso_vector32_ollama".to_owned(),
                available: false,
                degraded: true,
                reason: Some("provider_not_configured".to_owned()),
            },
        ));
    }
    let (config, embedding) = match store.embed_query(query).await {
        Ok(value) => value,
        Err(error) => {
            push_unique(degraded, "vector_provider_unavailable");
            diagnostics.push(ContextDiagnostic {
                source: "vector".to_owned(),
                code: "provider_error".to_owned(),
                message: bounded_message(&format!("{error:?}")),
            });
            return Ok((
                Vec::new(),
                ContextProviderStatus {
                    provider: "vector".to_owned(),
                    capability: "turso_vector32_ollama".to_owned(),
                    available: false,
                    degraded: true,
                    reason: Some("embedding_error".to_owned()),
                },
            ));
        }
    };
    let hits = match store
        .query_vector_chunks(board_id, &embedding, &config.model, limit)
        .await
    {
        Ok(value) => value,
        Err(error) => {
            push_unique(degraded, "vector_query_unavailable");
            diagnostics.push(ContextDiagnostic {
                source: "vector".to_owned(),
                code: "query_error".to_owned(),
                message: bounded_message(&error.to_string()),
            });
            return Ok((
                Vec::new(),
                ContextProviderStatus {
                    provider: "vector".to_owned(),
                    capability: "turso_vector32_ollama".to_owned(),
                    available: false,
                    degraded: true,
                    reason: Some("query_error".to_owned()),
                },
            ));
        }
    };
    let candidates = hits
        .into_iter()
        .filter_map(|hit| {
            let entity_uri = hit.entity_uri?;
            if hit.board_id.as_deref() != Some(board_id) {
                return None;
            }
            Some(ContextCandidate {
                entity_uri,
                source: "vector".to_owned(),
                provenance: vec![format!("vector:{}", hit.embedding_model)],
                score: Some(f64::from(hit.score)),
                title: None,
                snippet: Some(hit.content),
                reason: "semantic_match".to_owned(),
                board_id: Some(board_id.to_owned()),
                evidence: vec![ContextEvidence {
                    kind: "task".to_owned(),
                    entity_uri: None,
                    task_id: None,
                    relation_id: None,
                    predicate: None,
                    summary: Some("vector semantic match".to_owned()),
                }],
            })
        })
        .collect();
    Ok((
        candidates,
        ContextProviderStatus {
            provider: "vector".to_owned(),
            capability: "turso_vector32_ollama".to_owned(),
            available: status.enabled,
            degraded: !status.enabled || status.dirty.unwrap_or(false) || status.failed_jobs > 0,
            reason: status
                .diagnostics
                .first()
                .cloned()
                .or_else(|| (!status.enabled).then_some("provider_disabled".to_owned())),
        },
    ))
}

fn bounded_message(message: &str) -> String {
    message.chars().take(240).collect()
}

fn push_unique(values: &mut Vec<String>, value: impl AsRef<str>) {
    let value = value.as_ref();
    if !values.iter().any(|item| item == value) {
        values.push(value.to_owned());
    }
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
            // 从看板隔离角度看，provider 是不可信的 projection。看板不匹配的 candidate
            // 会静默丢弃；provider 状态和诊断仍可审计。
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

    // provider 可以报告降级而不触发硬失败。诊断在 service 边界受限；此处只保留稳定
    // code，让 client 可以分支处理而不必解析消息。
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sources() -> ContextSources {
        ContextSources {
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
        }
    }

    #[tokio::test]
    async fn context_merge_is_stable_deduplicated_and_ranked() {
        let pack = merge_context_sources(
            ContextBuildOptions {
                budget: 10,
                ..ContextBuildOptions::default()
            },
            sources(),
        )
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
        let mut sources = sources();
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
