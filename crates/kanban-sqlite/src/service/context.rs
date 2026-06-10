use crate::connect_file;

#[cfg(any(feature = "graph-oxigraph", feature = "vector-lancedb"))]
use super::store_target;
use super::{
    MAX_SEARCH_LIMIT, MAX_TASK_LIST_LIMIT, board_id, context_graph_items, context_vector_items,
    context_vector_status, get_task, graph_store_status, search_tasks, validate_page_bounds,
};
#[cfg(any(feature = "graph-oxigraph", feature = "vector-lancedb"))]
use super::{current_last_event_id, derived_status_by_name, has_pending_outbox_for_target};

use std::path::Path;

use kanban_context::{
    ContextBrokerInput, ContextDiagnostic, ContextError, ContextItem, ContextPack, ContextPolicy,
};

use kanban_core::{KanbanError, Result};

use kanban_entity::EntityUri;

use kanban_graph::GraphStoreStatus;

#[cfg(feature = "vector-lancedb")]
use kanban_indexer::LANCEDB_CHUNKS_STORE;
#[cfg(feature = "graph-oxigraph")]
use kanban_indexer::OXIGRAPH_RELATIONS_STORE;

use kanban_search::SearchQuery;

use kanban_vector::VectorStore;

use rusqlite::Connection;

pub fn build_context_pack(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: &str,
    policy: ContextPolicy,
) -> Result<ContextPack> {
    build_context_pack_inner(path.as_ref(), board, task_ref, policy, None)
}

#[cfg(feature = "vector-lancedb")]
pub fn build_context_pack_with_vector_store(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: &str,
    policy: ContextPolicy,
    vector_store: &impl VectorStore,
) -> Result<ContextPack> {
    build_context_pack_inner(path.as_ref(), board, task_ref, policy, Some(vector_store))
}

fn build_context_pack_inner(
    path_ref: &Path,
    board: &str,
    task_ref: &str,
    policy: ContextPolicy,
    #[cfg_attr(not(feature = "vector-lancedb"), allow(unused_variables))] vector_store: Option<
        &dyn VectorStore,
    >,
) -> Result<ContextPack> {
    validate_page_bounds(policy.lexical_limit, MAX_SEARCH_LIMIT, 0)?;
    validate_page_bounds(policy.graph_limit, MAX_TASK_LIST_LIMIT, 0)?;
    validate_page_bounds(policy.vector_limit, MAX_TASK_LIST_LIMIT, 0)?;
    validate_page_bounds(policy.max_items, MAX_TASK_LIST_LIMIT, 0)?;
    validate_context_max_items(policy.max_items)?;

    let conn = connect_file(path_ref)?;
    let board_id = board_id(&conn, board)?;
    let mut degraded = context_derived_degraded_markers(&conn, &board_id)?;
    let mut diagnostics = Vec::new();
    let task = get_task(path_ref, board, task_ref)?;
    let subject = EntityUri::task(&task.id);
    let lexical = search_tasks(
        path_ref,
        SearchQuery {
            board: board.to_owned(),
            q: Some(task.title.clone()),
            statuses: vec![],
            assignee: None,
            include_archived: true,
            limit: policy.lexical_limit,
            offset: 0,
        },
    )?;
    let graph_status = match graph_store_status(path_ref, board) {
        Ok(status) => status,
        Err(error) => {
            push_degraded_marker(&mut degraded, "graph_error");
            push_context_diagnostic(&mut diagnostics, "graph", "graph_error", &error);
            GraphStoreStatus {
                backend: graph_backend_name(),
                enabled: cfg!(feature = "graph-oxigraph"),
                message: error.to_string(),
            }
        }
    };
    let graph = match context_graph_items(path_ref, &subject, policy.graph_limit) {
        Ok(items) => items,
        Err(error) => {
            push_degraded_marker(&mut degraded, "graph_error");
            push_context_diagnostic(&mut diagnostics, "graph", "graph_error", &error);
            Vec::new()
        }
    };
    let vector_status = context_vector_status(
        path_ref,
        &conn,
        &board_id,
        board,
        vector_store,
        &mut degraded,
        &mut diagnostics,
    );
    let vector = match context_vector_items(
        path_ref,
        &task,
        &vector_status,
        policy.vector_limit,
        vector_store,
    ) {
        Ok(items) => items,
        Err(error) => {
            push_degraded_marker(&mut degraded, "vector_error");
            push_context_diagnostic(&mut diagnostics, "vector", "vector_error", &error);
            Vec::new()
        }
    };

    kanban_context::build_context_pack(
        subject.clone(),
        policy,
        ContextBrokerInput {
            subject_item: ContextItem {
                entity_uri: subject,
                source: "subject".to_owned(),
                provenance: vec!["sqlite:tasks".to_owned()],
                score: None,
                title: Some(task.title),
                snippet: task.description,
            },
            lexical,
            graph,
            vector,
            graph_status,
            vector_status,
            degraded,
            diagnostics,
        },
    )
    .map_err(context_error)
}

fn validate_context_max_items(max_items: usize) -> Result<()> {
    if max_items == 0 {
        return Err(KanbanError::InvalidInput(
            "max_items must be >= 1 because the subject item is mandatory".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn push_degraded_marker(degraded: &mut Vec<String>, marker: &str) {
    if !degraded.iter().any(|value| value == marker) {
        degraded.push(marker.to_owned());
    }
}

pub(crate) fn push_context_diagnostic(
    diagnostics: &mut Vec<ContextDiagnostic>,
    source: &str,
    code: &str,
    error: &impl std::fmt::Display,
) {
    diagnostics.push(ContextDiagnostic {
        source: source.to_owned(),
        code: code.to_owned(),
        message: bounded_diagnostic_message(error),
    });
}

fn bounded_diagnostic_message(error: &impl std::fmt::Display) -> String {
    const MAX_DIAGNOSTIC_MESSAGE_LEN: usize = 240;
    let mut message = error.to_string().replace(['\r', '\n'], " ");
    if message.len() > MAX_DIAGNOSTIC_MESSAGE_LEN {
        message.truncate(MAX_DIAGNOSTIC_MESSAGE_LEN);
        message.push_str("...");
    }
    message
}

fn context_error(error: ContextError) -> KanbanError {
    match error {
        ContextError::InvalidInput(message) => KanbanError::InvalidInput(message),
        ContextError::Retrieval(message) => KanbanError::Storage(message),
    }
}

fn context_derived_degraded_markers(
    #[cfg_attr(
        not(any(feature = "graph-oxigraph", feature = "vector-lancedb")),
        allow(unused_variables)
    )]
    conn: &Connection,
    #[cfg_attr(
        not(any(feature = "graph-oxigraph", feature = "vector-lancedb")),
        allow(unused_variables)
    )]
    board_id: &str,
) -> Result<Vec<String>> {
    #[cfg_attr(
        not(any(feature = "graph-oxigraph", feature = "vector-lancedb")),
        allow(unused_mut)
    )]
    let mut degraded = Vec::new();
    #[cfg(feature = "graph-oxigraph")]
    push_store_state_markers(
        conn,
        board_id,
        OXIGRAPH_RELATIONS_STORE,
        "graph",
        &mut degraded,
    )?;
    #[cfg(feature = "vector-lancedb")]
    push_store_state_markers(
        conn,
        board_id,
        LANCEDB_CHUNKS_STORE,
        "vector",
        &mut degraded,
    )?;
    Ok(degraded)
}

#[cfg(any(feature = "graph-oxigraph", feature = "vector-lancedb"))]
fn push_store_state_markers(
    conn: &Connection,
    board_id: &str,
    store_name: &str,
    marker_prefix: &str,
    degraded: &mut Vec<String>,
) -> Result<()> {
    let state = derived_status_by_name(conn, store_name)?;
    let current_last_event_id = current_last_event_id(conn, board_id)?;
    let target = store_target(store_name)?;
    let pending = has_pending_outbox_for_target(conn, target, board_id, current_last_event_id)?;
    if state.dirty {
        push_degraded_marker(degraded, &format!("{marker_prefix}_dirty"));
    }
    if pending {
        push_degraded_marker(degraded, &format!("{marker_prefix}_stale"));
    }
    if state.last_error.is_some() {
        push_degraded_marker(degraded, &format!("{marker_prefix}_error"));
    }
    Ok(())
}

#[cfg(feature = "graph-oxigraph")]
fn graph_backend_name() -> String {
    "oxigraph".to_owned()
}

#[cfg(not(feature = "graph-oxigraph"))]
fn graph_backend_name() -> String {
    "disabled".to_owned()
}
