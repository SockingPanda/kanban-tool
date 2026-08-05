pub(super) mod block;
pub(super) mod claim;
pub(super) mod create;
pub(super) mod done;
pub(super) mod heartbeat;
pub(super) mod list;
pub(super) mod plan_not_required;
pub(super) mod promote;
pub(super) mod release;
pub(super) mod review;
pub(super) mod show;
pub(super) mod step;

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
    }
}
