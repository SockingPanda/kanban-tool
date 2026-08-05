mod configure;
mod query_chunks;
mod query_label_atoms;
mod rebuild;
mod status;
mod sync;

use clap::Subcommand;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum VectorCommand {
    /// 配置 host 内 Ollama embedding provider。
    Configure(configure::Args),
    /// 查看 Turso vector projection 状态。
    Status(status::Args),
    /// 重建 vector projection jobs。
    Rebuild(rebuild::Args),
    /// 同步 vector projection jobs。
    Sync(sync::Args),
    /// 查询 task chunks 的 cosine 相似度。
    QueryChunks(query_chunks::Args),
    /// 查询 label atoms 的 cosine 相似度。
    QueryLabelAtoms(query_label_atoms::Args),
}

pub(crate) fn run(ctx: &CliContext, command: &VectorCommand) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    match command {
        VectorCommand::Configure(args) => configure::run(ctx, &client, args),
        VectorCommand::Status(args) => status::run(ctx, &client, args),
        VectorCommand::Rebuild(args) => rebuild::run(ctx, &client, args),
        VectorCommand::Sync(args) => sync::run(ctx, &client, args),
        VectorCommand::QueryChunks(args) => query_chunks::run(ctx, &client, args),
        VectorCommand::QueryLabelAtoms(args) => query_label_atoms::run(ctx, &client, args),
    }
}
