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
    /// 查看 canonical graph 和 projection 状态。
    Status,
    /// 列出 entity 的 canonical 出向关系事实。
    Neighbors(neighbors::NeighborsArgs),
    /// 执行有界只读 graph query 兼容 surface。
    Query(query::QueryArgs),
    /// 查看一个任务的有界 neighborhood。
    Neighborhood(neighborhood::NeighborhoodArgs),
    /// 查看一个看板的有界任务 map。
    Map(map::MapArgs),
    /// 对账 graph 事实（canonical 关系事实已是最新）。
    Rebuild,
    /// 同步 graph 事实（canonical 关系事实已是最新）。
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
