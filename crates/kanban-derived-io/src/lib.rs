mod db;
mod graph_io;
mod label_atom_io;
mod status;
mod vector_io;

pub use db::{
    board_id, connect_file, current_last_event_id, default_pragmas, maintenance_lock_blocks,
    maintenance_lock_path,
};
pub use graph_io::{
    graph_entity_uris_for_board, graph_relation_snapshot_for_board, graph_relations_for_entity,
    has_pending_graph_outbox_for_board, pending_graph_outbox_for_board,
    rebuild_oxigraph_with_store, sync_oxigraph_with_store,
};
pub use kanban_helper_protocol::{HELPER_PROTOCOL, HelperEnvelope};
pub use label_atom_io::{
    label_atom_index_status_from_base, label_atom_index_status_with_conn,
    label_atom_vectors_for_board, mark_label_atom_store_dirty, mark_label_atom_store_failure,
    mark_label_atom_store_success, rebuild_lancedb_label_atoms_with_store,
    sync_lancedb_label_atoms_with_store,
};
pub use status::{
    DerivedStoreStatusRecord, IndexOutboxRecord, derived_status_by_name,
    has_pending_outbox_for_target, mark_derived_store_failure, mark_derived_store_success,
};
pub use vector_io::{
    has_pending_vector_outbox_for_board, pending_vector_outbox_for_board,
    rebuild_lancedb_chunks_with_store, sync_lancedb_chunks_with_store, vector_chunks_for_board,
    vector_chunks_for_entity_uris,
};

pub(crate) use db::{search_lag, storage};

pub(crate) use status::outbox_from_row;
