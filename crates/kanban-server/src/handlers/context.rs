use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
};

use crate::dto::{ContextBuildQuery, Envelope};
use crate::error::{ApiError, extractor_error, invalid_input, validate_page_bounds};
#[cfg(feature = "vector-lancedb")]
use crate::handlers::shared::configured_lancedb_store;
use crate::state::AppState;

pub(crate) async fn build_context(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    query: Result<Query<ContextBuildQuery>, QueryRejection>,
) -> Result<Json<Envelope<kanban_context::ContextPack>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(query.lexical_limit, kanban_sqlite::MAX_SEARCH_LIMIT, 0)?;
    validate_page_bounds(query.graph_limit, kanban_sqlite::MAX_TASK_LIST_LIMIT, 0)?;
    validate_page_bounds(query.vector_limit, kanban_sqlite::MAX_TASK_LIST_LIMIT, 0)?;
    validate_page_bounds(query.max_items, kanban_sqlite::MAX_TASK_LIST_LIMIT, 0)?;
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
    #[cfg(feature = "vector-lancedb")]
    {
        let vector_state = state.clone();
        let vector_board = query.board.clone();
        let vector_task_id = task_id.clone();
        let vector_policy = policy.clone();
        let vector_pack = tokio::task::spawn_blocking(
            move || -> Result<Option<kanban_context::ContextPack>, ApiError> {
                match configured_lancedb_store(&vector_state) {
                    Ok(Some(store)) => {
                        Ok(Some(kanban_sqlite::build_context_pack_with_vector_store(
                            vector_state.db_path(),
                            &vector_board,
                            &vector_task_id,
                            vector_policy,
                            &store,
                        )?))
                    }
                    Ok(None) => Ok(None),
                    Err(error) => {
                        let pack = kanban_sqlite::build_context_pack(
                            vector_state.db_path(),
                            &vector_board,
                            &vector_task_id,
                            vector_policy,
                        )?;
                        Ok(Some(mark_vector_store_construction_error(pack, &error)))
                    }
                }
            },
        )
        .await
        .map_err(|error| {
            ApiError(kanban_core::KanbanError::Storage(format!(
                "context vector worker failed: {error}"
            )))
        })??;

        if let Some(pack) = vector_pack {
            return Ok(Json(Envelope {
                data: pack,
                meta: None,
            }));
        }
    }
    Ok(Json(Envelope {
        data: kanban_sqlite::build_context_pack(state.db_path(), &query.board, &task_id, policy)?,
        meta: None,
    }))
}

#[cfg(feature = "vector-lancedb")]
fn mark_vector_store_construction_error(
    mut pack: kanban_context::ContextPack,
    error: &impl std::fmt::Display,
) -> kanban_context::ContextPack {
    if !pack.degraded.iter().any(|marker| marker == "vector_error") {
        pack.degraded.push("vector_error".to_owned());
    }
    pack.diagnostics.push(kanban_context::ContextDiagnostic {
        source: "vector".to_owned(),
        code: "vector_error".to_owned(),
        message: bounded_diagnostic_message(error),
    });
    pack
}

#[cfg(feature = "vector-lancedb")]
fn bounded_diagnostic_message(error: &impl std::fmt::Display) -> String {
    const MAX_DIAGNOSTIC_MESSAGE_LEN: usize = 240;
    let mut message = error.to_string().replace(['\r', '\n'], " ");
    if message.len() > MAX_DIAGNOSTIC_MESSAGE_LEN {
        message.truncate(MAX_DIAGNOSTIC_MESSAGE_LEN);
        message.push_str("...");
    }
    message
}
