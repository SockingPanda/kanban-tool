pub(super) mod archive;
pub(super) mod block;
pub(super) mod claim;
pub(super) mod create;
pub(super) mod done;
pub(super) mod heartbeat;
pub(super) mod list;
pub(super) mod plan_not_required;
pub(super) mod promote;
pub(super) mod reclaim;
pub(super) mod release;
pub(super) mod reopen;
pub(super) mod review;
pub(super) mod show;
pub(super) mod specify;
pub(super) mod step;
pub(super) mod unblock;
pub(super) mod update;

use clap::Subcommand;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum TaskCommand {
    /// 通过共享 application service 创建任务。
    Create(create::CreateArgs),
    /// 通过共享 application service 列出任务。
    List(list::ListArgs),
    /// 按全局 ID 或看板内 reference 解析并查看任务。
    Show(show::ShowArgs),
    /// 管理任务的 execution plan。
    Step {
        #[command(subcommand)]
        command: step::TaskStepCommand,
    },
    /// 将符合条件的 todo 或到期 scheduled 任务提升为 ready。
    Promote(promote::PromoteArgs),
    /// 原子 claim 一个 ready 任务并启动 run。
    Claim(claim::ClaimArgs),
    /// 使用匹配 token 延长 active claim lease。
    Heartbeat(heartbeat::HeartbeatArgs),
    /// 将 active claim 的任务退回 ready。
    Release(release::ReleaseArgs),
    /// 完成 active run 并将任务提交 review。
    Review(review::ReviewArgs),
    /// 完成 running 或 review 状态的任务。
    #[command(visible_alias = "complete")]
    Done(done::DoneArgs),
    /// 使用必填原因阻塞 active 任务。
    Block(block::BlockArgs),
    /// 更新任务的安全字段。
    Update(update::UpdateArgs),
    /// 补充 triage 任务规格并重算状态。
    Specify(specify::SpecifyArgs),
    /// 解除 blocked 状态并重算队列状态。
    Unblock(unblock::UnblockArgs),
    /// 重新打开已完成任务。
    Reopen(reopen::ReopenArgs),
    /// 回收过期或强制回收 running claim。
    Reclaim(reclaim::ReclaimArgs),
    /// 归档任务。
    Archive(archive::ArchiveArgs),
}

pub(crate) fn run(ctx: &CliContext, command: &TaskCommand) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    match command {
        TaskCommand::Create(args) => create::run(ctx, &client, args),
        TaskCommand::List(args) => list::run(ctx, &client, args),
        TaskCommand::Show(args) => show::run(ctx, &client, args),
        TaskCommand::Step { command } => step::run(ctx, &client, command),
        TaskCommand::Promote(args) => promote::run(ctx, &client, args),
        TaskCommand::Claim(args) => claim::run(ctx, &client, args),
        TaskCommand::Heartbeat(args) => heartbeat::run(ctx, &client, args),
        TaskCommand::Release(args) => release::run(ctx, &client, args),
        TaskCommand::Review(args) => review::run(ctx, &client, args),
        TaskCommand::Done(args) => done::run(ctx, &client, args),
        TaskCommand::Block(args) => block::run(ctx, &client, args),
        TaskCommand::Update(args) => update::run(ctx, &client, args),
        TaskCommand::Specify(args) => specify::run(ctx, &client, args),
        TaskCommand::Unblock(args) => unblock::run(ctx, &client, args),
        TaskCommand::Reopen(args) => reopen::run(ctx, &client, args),
        TaskCommand::Reclaim(args) => reclaim::run(ctx, &client, args),
        TaskCommand::Archive(args) => archive::run(ctx, &client, args),
    }
}
