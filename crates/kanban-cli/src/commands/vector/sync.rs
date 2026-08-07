use crate::{context::CliContext, error::CliFailure, output};
use clap::Args as ClapArgs;
use kanban_client::KanbanClient;
use kanban_protocol::cli_helpers::{CliVectorStatus, CliVectorSyncOutput};

#[derive(Debug, ClapArgs)]
pub(crate) struct Args {}

pub(crate) fn run(ctx: &CliContext, client: &KanbanClient, _args: &Args) -> Result<(), CliFailure> {
    let value = client.sync_vector(&ctx.board)?;
    let status = CliVectorStatus {
        backend: value.backend,
        enabled: value.enabled,
        message: value.message,
        diagnostics: value.diagnostics,
        dirty: value.dirty,
        board_dirty: value.board_dirty,
        generation: value.generation,
    };
    let output_value = CliVectorSyncOutput::new(status.clone());
    if ctx.json {
        output::print_json(&output_value);
    } else {
        println!("{}: {}", status.backend, status.message);
    }
    Ok(())
}
