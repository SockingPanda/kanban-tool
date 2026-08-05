pub(super) mod add;
pub(super) mod list;
pub(super) mod output;
pub(super) mod remove;

use clap::Subcommand;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum DependencyCommand {
    /// Add a parent dependency to a child task.
    Add(add::AddArgs),
    /// Remove a parent dependency from a child task.
    Remove(remove::RemoveArgs),
    /// List direct parent and child dependencies for a task.
    List(list::ListArgs),
}

pub(crate) fn run(ctx: &CliContext, command: &DependencyCommand) -> Result<(), CliFailure> {
    match command {
        DependencyCommand::Add(args) => add::run(ctx, args),
        DependencyCommand::Remove(args) => remove::run(ctx, args),
        DependencyCommand::List(args) => list::run(ctx, args),
    }
}
