/// Maximum task-list page size accepted by CLI, API, and SQLite service calls.
pub const MAX_TASK_LIST_LIMIT: usize = 1000;
/// Maximum search page size accepted by CLI, API, and SQLite service calls.
pub const MAX_SEARCH_LIMIT: usize = 1000;

mod boards;
mod comment_identity;
mod comment_metadata;
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
mod label_ontology;
mod label_proposals;
mod label_semantics;
mod label_suggestions;
mod maintenance;
mod projections;
mod run_logs;
mod runs;
mod search;
mod sql;
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
pub use label_ontology::*;
pub use label_proposals::*;
pub use label_semantics::*;
pub use label_suggestions::*;
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
pub(crate) use label_semantics::{
    mark_label_atom_store_dirty, rebuild_label_atoms_for_stable_hash_migration,
    stable_label_atom_hash_backfill_needed, upsert_label_semantics_candidate_in_tx,
};
pub(crate) use projections::*;
pub(crate) use run_logs::{
    allowed_run_log_roots, normalize_existing_aware, run_log_path_status_for_db_dir,
};
pub(crate) use sql::{
    SqlFilter, all, all_values, ensure_changed_one, exec, exec_named, exec_one, exec_one_named,
    exists, optional, required_row, scalar,
};
#[cfg(any(feature = "graph-oxigraph", feature = "vector-lancedb"))]
pub(crate) use vector::has_pending_outbox_for_target;
pub(crate) use vector::vector_storage;
#[cfg(feature = "vector-lancedb")]
pub(crate) use vector::{vector_store_path, vector_store_status_with_conn};

pub(crate) const fn dependency_parent_is_satisfied(status: kanban_core::TaskStatus) -> bool {
    matches!(
        status,
        kanban_core::TaskStatus::Done | kanban_core::TaskStatus::Archived
    )
}

use transaction::{storage, with_immediate_tx, with_read_tx};
