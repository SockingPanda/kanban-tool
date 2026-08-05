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
    /// Create a task through the shared application service.
    Create(create::CreateArgs),
    /// List tasks through the shared application service.
    List(list::ListArgs),
    /// Show a task resolved by global id or board-local reference.
    Show(show::ShowArgs),
    /// Manage a task's execution plan.
    Step {
        #[command(subcommand)]
        command: step::TaskStepCommand,
    },
    /// Promote an eligible todo or due scheduled task to ready.
    Promote(promote::PromoteArgs),
    /// Atomically claim a ready task and start a run.
    Claim(claim::ClaimArgs),
    /// Extend the active claim lease with a matching token.
    Heartbeat(heartbeat::HeartbeatArgs),
    /// Return an actively claimed task to ready.
    Release(release::ReleaseArgs),
    /// Finish the active run and submit the task for review.
    Review(review::ReviewArgs),
    /// Complete a running or reviewed task.
    #[command(visible_alias = "complete")]
    Done(done::DoneArgs),
    /// Block an active task with a required reason.
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
