use crate::{error::ApiError, state::AppState};
use kanban_protocol::{
    BuildContextPath, BuildContextQuery, BuildContextResponse, ContextDiagnostic, ContextEvidence,
    ContextItem, ContextPack, ContextPolicy, ContextProviderStatus, DataEnvelope,
};
use kanban_service::{ContextBuildOptions, ContextPack as ApplicationContextPack};
pub(crate) async fn build_context(
    state: AppState,
    BuildContextPath { task_id }: BuildContextPath,
    query: BuildContextQuery,
) -> Result<BuildContextResponse, ApiError> {
    let budget = query.budget.unwrap_or(query.max_items);
    let has_explicit_selector =
        query.task.is_some() || query.reference.is_some() || query.query.is_some();
    let pack = state
        .application()
        .build_context(ContextBuildOptions {
            board: query.board,
            task: query
                .task
                .or_else(|| (!has_explicit_selector).then_some(task_id)),
            reference: query.reference,
            query: query.query,
            depth: query.depth,
            lexical_limit: query.lexical_limit,
            graph_limit: query.graph_limit,
            vector_limit: query.vector_limit,
            budget,
        })
        .await?;
    Ok(DataEnvelope::new(context_pack(pack)))
}
fn context_pack(value: ApplicationContextPack) -> ContextPack {
    ContextPack {
        subject: value.subject,
        policy: ContextPolicy {
            depth: value.policy.depth,
            lexical_limit: value.policy.lexical_limit,
            graph_limit: value.policy.graph_limit,
            vector_limit: value.policy.vector_limit,
            max_items: value.policy.max_items,
            budget: Some(value.policy.budget),
        },
        items: value.items.into_iter().map(context_item).collect(),
        degraded: value.degraded,
        diagnostics: value
            .diagnostics
            .into_iter()
            .map(|diagnostic| ContextDiagnostic {
                source: diagnostic.source,
                code: diagnostic.code,
                message: diagnostic.message,
            })
            .collect(),
        providers: value
            .providers
            .into_iter()
            .map(|provider| ContextProviderStatus {
                provider: provider.provider,
                capability: provider.capability,
                available: provider.available,
                degraded: provider.degraded,
                reason: provider.reason,
            })
            .collect(),
        truncated: value.truncated,
        truncation_reason: value.truncation_reason,
    }
}
fn context_item(value: kanban_service::ContextItem) -> ContextItem {
    ContextItem {
        entity_uri: value.entity_uri,
        source: value.source,
        provenance: value.provenance,
        score: value.score,
        title: value.title,
        snippet: value.snippet,
        rank: value.rank,
        reason: value.reason,
        evidence: value
            .evidence
            .into_iter()
            .map(|evidence| ContextEvidence {
                kind: evidence.kind,
                entity_uri: evidence.entity_uri,
                task_id: evidence.task_id,
                relation_id: evidence.relation_id,
                predicate: evidence.predicate,
                summary: evidence.summary,
            })
            .collect(),
    }
}
