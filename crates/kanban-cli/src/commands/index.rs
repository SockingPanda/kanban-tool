use std::path::PathBuf;

use anyhow::Result;
use kanban_contract::cli_helpers::{CliIndexRebuildOutput, CliIndexSyncOutput};
use kanban_contract::{CliIndexDoctorOutput, CliIndexStatusOutput};
use kanban_sqlite::api::{rebuild_search_index, search_index_status, sync_search_index};

use crate::args::IndexCommand;
use crate::output::print_contract_or_human;

pub(crate) fn handle_index(
    command: IndexCommand,
    db_path: &PathBuf,
    board: &str,
    json: bool,
) -> Result<()> {
    match command {
        IndexCommand::Status => {
            let status = search_index_status(db_path, board)?;
            let human = status_summary(&status);
            let output: CliIndexStatusOutput =
                CliIndexStatusOutput::new(kanban_server::search_status_from_record(status));
            print_contract_or_human(json, &output, || human)
        }
        IndexCommand::Doctor => {
            let status = search_index_status(db_path, board)?;
            let human = status_summary(&status);
            let output: CliIndexDoctorOutput =
                CliIndexDoctorOutput::new(kanban_server::search_status_from_record(status));
            print_contract_or_human(json, &output, || human)
        }
        IndexCommand::Rebuild => {
            let status = rebuild_search_index(db_path, board)?;
            let human = status_summary(&status);
            let output =
                CliIndexRebuildOutput::new(kanban_server::search_status_from_record(status));
            print_contract_or_human(json, &output, || human)
        }
        IndexCommand::Sync => {
            let status = sync_search_index(db_path, board)?;
            let human = status_summary(&status);
            let output = CliIndexSyncOutput::new(kanban_server::search_status_from_record(status));
            print_contract_or_human(json, &output, || human)
        }
    }
}

fn status_summary(status: &kanban_search::SearchIndexStatus) -> String {
    format!(
        "search backend={} derived_index={} stale={} last_event_id={:?} lag={:?}: {}",
        status.backend,
        status.derived_index,
        status.stale,
        status.last_event_id,
        status.index_lag_events,
        status.message
    )
}
