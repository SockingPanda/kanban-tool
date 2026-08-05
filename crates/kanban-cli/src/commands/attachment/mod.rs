mod add;
mod download;
mod list;
mod remove;

use clap::Subcommand;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum AttachmentCommand {
    /// Add a file-backed attachment to a task.
    Add(add::AddArgs),
    /// List attachment metadata for a task.
    List(list::ListArgs),
    /// Download attachment bytes into a local path.
    Download(download::DownloadArgs),
    /// Remove an attachment while retaining a host-local trash copy.
    Remove(remove::RemoveArgs),
}

pub(crate) fn run(ctx: &CliContext, command: &AttachmentCommand) -> Result<(), CliFailure> {
    match command {
        AttachmentCommand::Add(args) => add::run(ctx, args),
        AttachmentCommand::List(args) => list::run(ctx, args),
        AttachmentCommand::Download(args) => download::run(ctx, args),
        AttachmentCommand::Remove(args) => remove::run(ctx, args),
    }
}
