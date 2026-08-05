pub(super) mod columns;
pub(super) mod list;

use clap::Subcommand;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum BoardCommand {
    /// List boards from the canonical application service.
    List(list::ListArgs),
    /// List a board's fixed status columns.
    Columns(columns::ColumnsArgs),
}

pub(crate) fn run(ctx: &CliContext, command: &BoardCommand) -> Result<(), CliFailure> {
    match command {
        BoardCommand::List(args) => list::run(ctx, args),
        BoardCommand::Columns(args) => columns::run(ctx, args),
    }
}
