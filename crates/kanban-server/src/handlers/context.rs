use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
};
use kanban_contract::{
    BuildContextPath, BuildContextQuery, BuildContextResponse, ContextDiagnostic, ContextItem,
    ContextPack, ContextPolicy, DataEnvelope,
};

use crate::error::{ApiError, extractor_error, invalid_input, validate_page_bounds};
use crate::helper::{HelperKind, resolve_helper};
use crate::state::AppState;

pub(crate) async fn build_context(
    State(state): State<AppState>,
    Path(path): Path<BuildContextPath>,
    query: Result<Query<BuildContextQuery>, QueryRejection>,
) -> Result<Json<BuildContextResponse>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(query.lexical_limit, kanban_sqlite::api::MAX_SEARCH_LIMIT, 0)?;
    validate_page_bounds(
        query.graph_limit,
        kanban_sqlite::api::MAX_TASK_LIST_LIMIT,
        0,
    )?;
    validate_page_bounds(
        query.vector_limit,
        kanban_sqlite::api::MAX_TASK_LIST_LIMIT,
        0,
    )?;
    validate_page_bounds(query.max_items, kanban_sqlite::api::MAX_TASK_LIST_LIMIT, 0)?;
    if query.max_items == 0 {
        return Err(invalid_input(
            "max_items must be >= 1 because the subject item is mandatory",
        ));
    }
    let policy = kanban_context::ContextPolicy {
        lexical_limit: query.lexical_limit,
        graph_limit: query.graph_limit,
        vector_limit: query.vector_limit,
        max_items: query.max_items,
    };
    let store = kanban_vector::SubprocessVectorStore::new(
        resolve_helper(&state, HelperKind::Vector),
        state.db_path().to_path_buf(),
        query.board.clone(),
        state.vector_config_path().map(std::path::Path::to_path_buf),
    );
    let pack = kanban_sqlite::api::provider::build_context_pack_with_vector_store(
        state.db_path(),
        &query.board,
        &path.task_id,
        policy,
        &store,
    )?;
    Ok(Json(DataEnvelope::new(context_pack(pack))))
}

fn context_pack(value: kanban_context::ContextPack) -> ContextPack {
    ContextPack {
        subject: value.subject.to_string(),
        policy: ContextPolicy {
            lexical_limit: value.policy.lexical_limit,
            graph_limit: value.policy.graph_limit,
            vector_limit: value.policy.vector_limit,
            max_items: value.policy.max_items,
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
    }
}

fn context_item(value: kanban_context::ContextItem) -> ContextItem {
    ContextItem {
        entity_uri: value.entity_uri.to_string(),
        source: value.source,
        provenance: value.provenance,
        score: value.score,
        title: value.title,
        snippet: value.snippet,
    }
}
