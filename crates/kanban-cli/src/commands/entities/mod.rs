mod list;
mod show;
mod upsert;

use clap::Subcommand;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum EntityCommand {
    /// 列出 canonical entity。
    List(list::ListArgs),
    /// 按 URI 查看一个 canonical entity。
    Show(show::ShowArgs),
    /// 插入或更新一个 canonical entity。
    Upsert(upsert::UpsertArgs),
}

pub(crate) fn run(ctx: &CliContext, command: &EntityCommand) -> Result<(), CliFailure> {
    match command {
        EntityCommand::List(args) => list::run(ctx, args),
        EntityCommand::Show(args) => show::run(ctx, args),
        EntityCommand::Upsert(args) => upsert::run(ctx, args),
    }
}
