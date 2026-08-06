mod list;
mod log;
mod show;

use clap::Subcommand;
pub(crate) use list::ListArgs;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum RunCommand {
    /// 查看一个执行 run。
    Show(show::ShowArgs),
    /// 读取一个执行 run 的有界日志快照。
    #[command(visible_alias = "log")]
    Logs(log::LogArgs),
}

pub(crate) fn list(ctx: &CliContext, args: &ListArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    list::run(ctx, &client, args)
}

pub(crate) fn run(ctx: &CliContext, command: &RunCommand) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    match command {
        RunCommand::Show(args) => show::run(ctx, &client, args),
        RunCommand::Logs(args) => log::run(ctx, &client, args),
    }
}
