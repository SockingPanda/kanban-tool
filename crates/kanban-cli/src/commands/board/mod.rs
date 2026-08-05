pub(super) mod archive;
pub(super) mod columns;
pub(super) mod create;
pub(super) mod list;
pub(super) mod show;

use clap::Subcommand;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum BoardCommand {
    /// 创建看板并初始化默认列。
    Create(create::CreateArgs),
    /// List boards from the canonical application service.
    List(list::ListArgs),
    /// 查看看板，包括已归档历史。
    Show(show::ShowArgs),
    /// 归档看板。
    Archive(archive::ArchiveArgs),
    /// List a board's fixed status columns.
    Columns(columns::ColumnsArgs),
}

pub(crate) fn run(ctx: &CliContext, command: &BoardCommand) -> Result<(), CliFailure> {
    match command {
        BoardCommand::Create(args) => create::run(ctx, args),
        BoardCommand::List(args) => list::run(ctx, args),
        BoardCommand::Show(args) => show::run(ctx, args),
        BoardCommand::Archive(args) => archive::run(ctx, args),
        BoardCommand::Columns(args) => columns::run(ctx, args),
    }
}
