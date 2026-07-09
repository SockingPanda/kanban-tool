use std::path::PathBuf;

use anyhow::Result;
use kanban_sqlite::api::{rebuild_search_index, search_index_status, sync_search_index};

use crate::args::IndexCommand;
use crate::output::print_or_json;

pub(crate) fn handle_index(
    command: IndexCommand,
    db_path: &PathBuf,
    board: &str,
    json: bool,
) -> Result<()> {
    let status = match command {
        IndexCommand::Status | IndexCommand::Doctor => search_index_status(db_path, board)?,
        IndexCommand::Rebuild => rebuild_search_index(db_path, board)?,
        IndexCommand::Sync => sync_search_index(db_path, board)?,
    };
    print_or_json(json, &status, || {
        format!(
            "search backend={} derived_index={} stale={} last_event_id={:?} lag={:?}: {}",
            status.backend,
            status.derived_index,
            status.stale,
            status.last_event_id,
            status.index_lag_events,
            status.message
        )
    })
}
