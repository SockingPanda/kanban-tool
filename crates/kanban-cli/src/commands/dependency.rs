pub(super) mod add;
pub(super) mod list;
pub(super) mod output;
pub(super) mod remove;

use clap::Subcommand;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum DependencyCommand {
    /// 为子任务添加父任务依赖。
    Add(add::AddArgs),
    /// 移除子任务的父任务依赖。
    Remove(remove::RemoveArgs),
    /// 列出任务的直接父子依赖。
    List(list::ListArgs),
}

pub(crate) fn run(ctx: &CliContext, command: &DependencyCommand) -> Result<(), CliFailure> {
    match command {
        DependencyCommand::Add(args) => add::run(ctx, args),
        DependencyCommand::Remove(args) => remove::run(ctx, args),
        DependencyCommand::List(args) => list::run(ctx, args),
    }
}
