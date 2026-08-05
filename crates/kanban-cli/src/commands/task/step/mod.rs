pub(super) mod add;
pub(super) mod done;
pub(super) mod list;
pub(super) mod remove;
pub(super) mod reopen;
pub(super) mod skip;
pub(super) mod update;

use clap::Subcommand;
use kanban_client::KanbanClient;

use crate::{context::CliContext, error::CliFailure};

use super::plan_not_required;

#[derive(Debug, Subcommand)]
pub(crate) enum TaskStepCommand {
    /// 向任务 execution plan 添加 todo step。
    Add(add::AddArgs),
    /// 列出任务 execution plan 的 step。
    List(list::ListArgs),
    /// 使用 note 将任务 step 标记为完成。
    Done(done::DoneArgs),
    /// 使用 reason 跳过任务 step。
    Skip(skip::SkipArgs),
    /// 重新打开已解析的任务 step。
    Reopen(reopen::ReopenArgs),
    /// 删除任务 step。
    Remove(remove::RemoveArgs),
    /// 更新 execution-plan 可编辑字段，不改变 step status。
    Update(update::UpdateArgs),
    /// 将任务标记为不需要结构化 execution step。
    NotRequired(plan_not_required::PlanNotRequiredArgs),
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    command: &TaskStepCommand,
) -> Result<(), CliFailure> {
    match command {
        TaskStepCommand::Add(args) => add::run(ctx, client, args),
        TaskStepCommand::List(args) => list::run(ctx, client, args),
        TaskStepCommand::Done(args) => done::run(ctx, client, args),
        TaskStepCommand::Skip(args) => skip::run(ctx, client, args),
        TaskStepCommand::Reopen(args) => reopen::run(ctx, client, args),
        TaskStepCommand::Remove(args) => remove::run(ctx, client, args),
        TaskStepCommand::Update(args) => update::run(ctx, client, args),
        TaskStepCommand::NotRequired(args) => plan_not_required::run(ctx, client, args),
    }
}
