use std::path::PathBuf;

use anyhow::Result;
use kanban_sqlite::dag_snapshot;

use crate::{args::DagCommand, output::print_or_json};

pub(crate) fn handle_dag(
    command: DagCommand,
    db_path: &PathBuf,
    board: &str,
    json: bool,
) -> Result<()> {
    match command {
        DagCommand::Show => {
            let snapshot = dag_snapshot(db_path, board)?;
            print_or_json(json, &snapshot, || {
                format!(
                    "DAG {}: nodes={} edges={} frontier={}",
                    snapshot.board.slug,
                    snapshot.snapshot.node_count,
                    snapshot.snapshot.edge_count,
                    snapshot.derived.frontier.len()
                )
            })?;
        }
    }
    Ok(())
}
