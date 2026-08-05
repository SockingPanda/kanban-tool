use axum::{
    Json, Router,
    extract::{Path, Query, State, rejection::QueryRejection},
    routing::get,
};
use kanban_service::{ContextBuildOptions, ContextPack as ApplicationContextPack};
use kanban_core::KanbanError;
use kanban_protocol::{
    BuildContextPath, BuildContextQuery, BuildContextResponse, ContextDiagnostic, ContextEvidence,
    ContextItem, ContextPack, ContextPolicy, ContextProviderStatus, DataEnvelope,
};

use crate::{error::ApiError, state::AppState};

pub(crate) async fn build_context(
    State(state): State<AppState>,
    Path(BuildContextPath { task_id }): Path<BuildContextPath>,
    query: Result<Query<BuildContextQuery>, QueryRejection>,
) -> Result<Json<BuildContextResponse>, ApiError> {
    let Query(query) = query
        .map_err(|error| KanbanError::InvalidInput(format!("invalid context query: {error}")))?;
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
    Ok(Json(DataEnvelope::new(context_pack(pack))))
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

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/tasks/:task_id/context", get(build_context))
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;

    fn get_request(uri: &str) -> Request<Body> {
        Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn context_route_returns_subject_first_ranked_pack() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards/default/tasks",
                serde_json::json!({
                    "title": "上下文主题",
                    "description": "canonical subject",
                    "actor": "context-test"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let created: CreateTaskResponse = serde_json::from_slice(&bytes).unwrap();

        let response = router
            .oneshot(get_request(&format!(
                "/api/v1/tasks/{}/context?board=default&query=%E4%B8%8A%E4%B8%8B%E6%96%87&max_items=3",
                created.data.id
            )))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let context: BuildContextResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            context.data.subject,
            format!("kb://task/{}", created.data.id)
        );
        assert_eq!(context.data.items.first().unwrap().rank, 1);
        assert_eq!(context.data.policy.depth, 1);
        assert_eq!(context.data.policy.max_items, 3);
    }
}
