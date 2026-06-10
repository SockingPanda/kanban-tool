use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
};

use crate::dto::{ContextBuildQuery, Envelope};
use crate::error::{ApiError, extractor_error, invalid_input, validate_page_bounds};
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
    Ok(Json(Envelope {
        data: kanban_sqlite::build_context_pack(state.db_path(), &query.board, &task_id, policy)?,
        meta: None,
    }))
}
