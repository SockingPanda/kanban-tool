use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::Duration,
};

use kanban_context::{
    ContextBrokerInput, ContextDiagnostic, ContextError, ContextItem, ContextPack, ContextPolicy,
};
use kanban_core::{
    Clock, KanbanError, ReadinessFacts, Result, SystemClock, TaskStatus, can_complete_from,
    can_finish_to, can_promote_from, completed_at_for_finish,
    initial_status as core_initial_status, is_active_recomputable_status, is_claimable_task,
    new_event_id, new_run_id, new_task_id, new_typed_id,
    recompute_ready_status as core_recompute_ready_status, retry_decision,
    running_claim_is_present,
};
use kanban_entity::{EntityUri, Predicate, Provenance, Relation};
use kanban_graph::GraphStoreStatus;
#[cfg(feature = "graph-oxigraph")]
use kanban_graph::{OxigraphStore, RelationGraph};
use kanban_indexer::LANCEDB_CHUNKS_STORE;
#[cfg(feature = "graph-oxigraph")]
use kanban_indexer::OXIGRAPH_RELATIONS_STORE;
#[cfg(feature = "tantivy-backend")]
use kanban_indexer::TANTIVY_TASKS_STORE;
use kanban_indexer::{
    DERIVED_STORE_SEEDS, DerivedStoreUpdate, OUTBOX_FANOUT_TARGETS, OutboxTarget,
    derived_store_for_name,
};
#[cfg(feature = "tantivy-backend")]
use kanban_search::TaskSearchDocument;
use kanban_search::{SearchHit, SearchIndexStatus, SearchMeta, SearchQuery, SearchResults};
use kanban_vector::{ChunkBuilder, TaskChunkSource, VectorStore, VectorStoreStatus};
#[cfg(feature = "vector-lancedb")]
use kanban_vector::{LanceDbConfig, LanceDbStore};
#[cfg(feature = "vector-lancedb")]
use kanban_vector::{VectorHit, VectorQuery};
use rusqlite::{
    Connection, OptionalExtension, Row, params, params_from_iter,
    types::{Value, ValueRef},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    connect_file, default_pragmas, maintenance_lock_blocks, maintenance_lock_path,
    runtime_lock_blocks, runtime_lock_path,
};

/// Maximum task-list page size accepted by CLI, API, and SQLite service calls.
pub const MAX_TASK_LIST_LIMIT: usize = 1000;
/// Maximum search page size accepted by CLI, API, and SQLite service calls.
pub const MAX_SEARCH_LIMIT: usize = 1000;

mod boards;
mod comments;
mod context;
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
pub use context::*;
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
