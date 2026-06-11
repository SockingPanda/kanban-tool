/// Maximum task-list page size accepted by CLI, API, and SQLite service calls.
pub const MAX_TASK_LIST_LIMIT: usize = 1000;
/// Maximum search page size accepted by CLI, API, and SQLite service calls.
pub const MAX_SEARCH_LIMIT: usize = 1000;

mod boards;
mod comment_identity;
mod comments;
mod completions;
mod context;
mod dag;
mod dependencies;
mod dispatch;
mod entities;
mod events;
mod graph;
mod graph_api;
mod import_export;
mod maintenance;
mod projections;
mod run_logs;
mod runs;
mod search;
mod tasks;
mod transaction;
mod transitions;
mod types;
mod vector;

pub use boards::*;
pub use comments::*;
pub use completions::*;
pub use context::*;
pub use dag::*;
pub use dependencies::*;
pub use dispatch::*;
pub use entities::{
    DerivedStoreStatusRecord, DoctorDerivedStoreReport, DoctorReport, EntityListOptions,
    EntityRecord, IndexOutboxRecord, OutboxListOptions, derived_store_statuses, get_entity,
    list_entities, list_outbox,
};
pub use events::*;
pub use graph::*;
pub use graph_api::*;
pub use import_export::*;
pub use maintenance::*;
pub use run_logs::{resolve_run_log_path, run_log_path_status};
pub use runs::*;
pub use search::*;
pub use tasks::*;
pub use transitions::*;
pub use types::*;
pub use vector::*;

#[cfg(feature = "vector-lancedb")]
pub(crate) use context::{push_context_diagnostic, push_degraded_marker};
pub(crate) use entities::{
    derived_store_status_from_row, derived_store_statuses_conn, outbox_from_row,
};
pub(crate) use graph::{
    context_graph_items, context_vector_items, context_vector_status, derived_status_by_name,
    graph_relation_snapshot_for_board,
};
pub(crate) use projections::*;
pub(crate) use run_logs::{
    allowed_run_log_roots, normalize_existing_aware, run_log_path_status_for_db_dir,
};
#[cfg(any(feature = "graph-oxigraph", feature = "vector-lancedb"))]
pub(crate) use vector::has_pending_outbox_for_target;
#[cfg(feature = "vector-lancedb")]
pub(crate) use vector::{vector_storage, vector_store_path, vector_store_status_with};

use transaction::{storage, with_immediate_tx, with_read_tx};
