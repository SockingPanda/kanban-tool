mod map;
mod neighborhood;
mod neighbors;
mod query;
mod rebuild;
mod status;
mod sync;

use clap::Subcommand;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum GraphCommand {
    /// Show canonical graph and projection status.
    Status,
    /// List outgoing canonical relation facts for an entity.
    Neighbors(neighbors::NeighborsArgs),
    /// Run the bounded, read-only graph query compatibility surface.
    Query(query::QueryArgs),
    /// Show one task's bounded neighborhood.
    Neighborhood(neighborhood::NeighborhoodArgs),
    /// Show a board's bounded task map.
    Map(map::MapArgs),
    /// Reconcile graph facts (canonical relation facts are already current).
    Rebuild,
    /// Synchronize graph facts (canonical relation facts are already current).
    Sync,
}

pub(crate) fn run(ctx: &CliContext, command: &GraphCommand) -> Result<(), CliFailure> {
    match command {
        GraphCommand::Status => status::run(ctx),
        GraphCommand::Neighbors(args) => neighbors::run(ctx, args),
        GraphCommand::Query(args) => query::run(ctx, args),
        GraphCommand::Neighborhood(args) => neighborhood::run(ctx, args),
        GraphCommand::Map(args) => map::run(ctx, args),
        GraphCommand::Rebuild => rebuild::run(ctx),
        GraphCommand::Sync => sync::run(ctx),
    }
}
