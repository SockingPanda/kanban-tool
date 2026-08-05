pub(super) mod add;
pub(super) mod list;

use clap::Subcommand;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum CommentCommand {
    /// Add one note or decision comment to a task.
    Add(add::AddArgs),
    /// List task comments from the canonical application host.
    List(list::ListArgs),
}

pub(crate) fn run(ctx: &CliContext, command: &CommentCommand) -> Result<(), CliFailure> {
    match command {
        CommentCommand::Add(args) => add::run(ctx, args),
        CommentCommand::List(args) => list::run(ctx, args),
    }
}
