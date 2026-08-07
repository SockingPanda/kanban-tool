mod add;
mod download;
mod list;
mod remove;

use clap::Subcommand;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum AttachmentCommand {
    /// 向任务添加文件型附件。
    Add(add::AddArgs),
    /// 列出任务的附件元数据。
    List(list::ListArgs),
    /// 将附件字节下载到本地路径。
    Download(download::DownloadArgs),
    /// 移除附件，同时保留 host 本地 trash 副本。
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
