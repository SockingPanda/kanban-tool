//! Context pack provider adapter for the canonical Turso host.
//!
//! The application service owns merge, ranking, budgets and board isolation.
//! This adapter only translates Turso/host provider records into source
//! candidates and preserves degraded provider diagnostics.

use kanban_application::{
    ContextBuild, ContextBuildOptions, ContextCandidate, ContextDiagnostic, ContextEvidence,
    ContextProviderStatus, ContextSources,
};
use kanban_core::{KanbanError, Result};
use kanban_store_turso::{
    GraphStatusRecord, StoreSearchQuery, StoreSearchResults, TaskNeighborhoodOptions, TursoStore,
};

use crate::adapter::{TursoApplicationStore, store_error};

impl ContextBuild for TursoApplicationStore {
    async fn context_sources(&self, options: ContextBuildOptions) -> Result<ContextSources> {
        let board_id = self
            .store
            .vector_board_id(&options.board)
            .await
            .map_err(store_error)?;

        let initial_query = options
            .query
            .clone()
            .or_else(|| options.reference.clone())
            .or_else(|| options.task.clone());
        let initial_results = search(
            &self.store,
            &options.board,
            initial_query.as_deref(),
            options.lexical_limit.max(1),
        )
        .await?;
        let subject =
            resolve_subject(&self.store, &board_id, &options, initial_results.as_ref()).await?;
        let lexical_query = options
            .query
            .clone()
            .unwrap_or_else(|| subject.title.clone());
        let lexical_results = search(
            &self.store,
            &options.board,
            Some(&lexical_query),
            options.lexical_limit,
        )
        .await?;

        let mut degraded = Vec::new();
        let mut diagnostics = Vec::new();
        let lexical = lexical_candidates(
            &self.store,
            lexical_results.as_ref(),
            &board_id,
            &mut degraded,
            &mut diagnostics,
        )
        .await?;
        let lexical_provider = lexical_provider(lexical_results.as_ref());

        let (graph, graph_provider) = graph_candidates(
            &self.store,
            &subject.id,
            &board_id,
            options.depth,
            options.graph_limit,
            &mut degraded,
            &mut diagnostics,
        )
        .await?;

        let (vector, vector_provider) = vector_candidates(
            &self.store,
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

async fn search(
    store: &TursoStore,
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
        .map_err(store_error)
}

async fn resolve_subject(
    store: &TursoStore,
    board_id: &str,
    options: &ContextBuildOptions,
    initial_results: Option<&StoreSearchResults>,
) -> Result<kanban_store_turso::TaskRecord> {
    if let Some(task) = options
        .task
        .as_deref()
        .filter(|value| value.starts_with("t_"))
    {
        let value = store.get_task_global(task).await.map_err(store_error)?;
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
        .map_err(store_error)?;
    ensure_board(value, board_id)
}

fn ensure_board(
    task: kanban_store_turso::TaskRecord,
    board_id: &str,
) -> Result<kanban_store_turso::TaskRecord> {
    if task.board_id != board_id {
        return Err(KanbanError::NotFound("context subject".to_owned()));
    }
    Ok(task)
}

async fn lexical_candidates(
    store: &TursoStore,
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
            .map_err(store_error)
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
    store: &TursoStore,
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
            TaskNeighborhoodOptions {
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
    status: &GraphStatusRecord,
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
    store: &TursoStore,
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
    let Some(_config) = store.vector_config().await.map_err(store_error)? else {
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
    };
    let (config, embedding) = match crate::vector::embed_query(store, query).await {
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

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|item| item == value) {
        values.push(value.to_owned());
    }
}
