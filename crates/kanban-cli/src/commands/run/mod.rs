mod list;
mod log;
mod show;

use clap::Subcommand;
pub(crate) use list::ListArgs;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum RunCommand {
    /// Show one execution run.
    Show(show::ShowArgs),
}

pub(crate) fn list(ctx: &CliContext, args: &ListArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    list::run(ctx, &client, args)
}

pub(crate) fn run(ctx: &CliContext, command: &RunCommand) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    match command {
        RunCommand::Show(args) => show::run(ctx, &client, args),
    }
}
