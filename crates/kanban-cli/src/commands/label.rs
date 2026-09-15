mod add;
mod bootstrap;
mod create;
mod delete;
mod list;
pub(crate) mod ontology;
mod remove;

use clap::Subcommand;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum LabelCommand {
    /// 列出所选看板定义的 label。
    List(list::ListArgs),
    /// 创建看板 label；若同名则返回已有 label。
    Create(create::CreateArgs),
    /// 删除看板 label identity；存在任务绑定时需显式 `--force`。
    Delete(delete::DeleteArgs),
    /// 将一个或多个 label 绑定到任务。
    Add(add::AddArgs),
    /// 从任务上下文创建首个 label semantics 并绑定任务。
    Bootstrap(bootstrap::BootstrapArgs),
    /// 按 ID 或名称移除任务的 label。
    Remove(remove::RemoveArgs),
    /// 管理 label semantics 文本和示例。
    Semantics {
        #[command(subcommand)]
        command: ontology::SemanticsCommand,
    },
    /// 查看 label ontology atom。
    #[command(alias = "atom")]
    Atoms {
        #[command(subcommand)]
        command: ontology::AtomCommand,
    },
    /// 维护并查询 label atom index。
    #[command(name = "atom-index")]
    AtomIndex {
        #[command(subcommand)]
        command: ontology::AtomIndexCommand,
    },
    /// 为任务建议 label。
    Suggest(ontology::SuggestArgs),
    /// 根据任务上下文或 JSON 输入创建 label proposal。
    Propose(ontology::ProposeArgs),
    /// 审核并决定待处理的 label proposal。
    #[command(alias = "proposal")]
    Proposals {
        #[command(subcommand)]
        command: ontology::ProposalCommand,
    },
    /// 记录、审核、应用并校验 label ontology signal。
    Ontology {
        #[command(subcommand)]
        command: ontology::LedgerCommand,
    },
}

pub(crate) async fn run(ctx: &CliContext, command: &LabelCommand) -> Result<(), CliFailure> {
    match command {
        LabelCommand::List(args) => list::run(ctx, args).await,
        LabelCommand::Create(args) => create::run(ctx, args).await,
        LabelCommand::Delete(args) => delete::run(ctx, args).await,
        LabelCommand::Add(args) => add::run(ctx, args).await,
        LabelCommand::Bootstrap(args) => bootstrap::run(ctx, args).await,
        LabelCommand::Remove(args) => remove::run(ctx, args).await,
        LabelCommand::Semantics { command } => ontology::run_semantics(ctx, command).await,
        LabelCommand::Atoms { command } => ontology::run_atoms(ctx, command).await,
        LabelCommand::AtomIndex { command } => ontology::run_atom_index(ctx, command).await,
        LabelCommand::Suggest(args) => ontology::run_suggest(ctx, args).await,
        LabelCommand::Propose(args) => ontology::run_propose(ctx, args).await,
        LabelCommand::Proposals { command } => ontology::run_proposals(ctx, command).await,
        LabelCommand::Ontology { command } => ontology::run_ledger(ctx, command).await,
    }
}
