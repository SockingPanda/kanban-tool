mod list;
mod show;
mod upsert;

use clap::Subcommand;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum EntityCommand {
    /// List canonical entities.
    List(list::ListArgs),
    /// Show one canonical entity by URI.
    Show(show::ShowArgs),
    /// Insert or update one canonical entity.
    Upsert(upsert::UpsertArgs),
}

pub(crate) fn run(ctx: &CliContext, command: &EntityCommand) -> Result<(), CliFailure> {
    match command {
        EntityCommand::List(args) => list::run(ctx, args),
        EntityCommand::Show(args) => show::run(ctx, args),
        EntityCommand::Upsert(args) => upsert::run(ctx, args),
    }
}
