pub(super) mod add;
pub(super) mod list;

use clap::Subcommand;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum CommentCommand {
    /// 向任务添加备注或决策评论。
    Add(add::AddArgs),
    /// 从 canonical application host 列出任务评论。
    List(list::ListArgs),
}

pub(crate) fn run(ctx: &CliContext, command: &CommentCommand) -> Result<(), CliFailure> {
    match command {
        CommentCommand::Add(args) => add::run(ctx, args),
        CommentCommand::List(args) => list::run(ctx, args),
    }
}
