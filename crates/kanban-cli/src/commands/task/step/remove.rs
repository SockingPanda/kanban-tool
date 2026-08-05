use clap::Args;
use kanban_client::KanbanClient;
use kanban_contract::{CliTaskStepRemoveOutput, CliTaskStepRemoveResult};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct RemoveArgs {
    pub(crate) task_ref: String,
    pub(crate) step_ref: String,
}

pub(crate) fn run(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &RemoveArgs,
) -> Result<(), CliFailure> {
    let before = client.list_steps_by_selector(&ctx.board, &args.task_ref)?;
    let step_id = resolve_step_id(&before, &args.step_ref)?;
    let removed = before
        .steps
        .iter()
        .find(|step| step.id == step_id)
        .cloned()
        .ok_or_else(|| CliFailure {
            code: "invalid_response",
            message: format!("step 响应缺少 {step_id}"),
            exit_code: 2,
        })?;
    client.remove_step_by_selector(&ctx.board, &args.task_ref, &args.step_ref)?;
    if ctx.json {
        output::print_json(&CliTaskStepRemoveOutput::new(CliTaskStepRemoveResult {
            removed: true,
            step: removed,
        }));
    } else {
        println!("已从 {} 移除 step {}", args.task_ref, args.step_ref);
    }
    Ok(())
}

fn resolve_step_id(
    steps: &kanban_contract::ApiTaskSteps,
    selector: &str,
) -> Result<String, CliFailure> {
    let selector = selector.trim();
    if selector.starts_with("step_") && selector.len() > 5 {
        return Ok(selector.to_owned());
    }
    let index = selector
        .strip_prefix('S')
        .or_else(|| selector.strip_prefix('s'))
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|index| *index > 0)
        .ok_or_else(|| CliFailure {
            code: "invalid_input",
            message: "step selector 必须是全局 step_... ID 或 S<n>".to_owned(),
            exit_code: 2,
        })?;
    steps
        .steps
        .get(index - 1)
        .map(|step| step.id.clone())
        .ok_or_else(|| CliFailure {
            code: "not_found",
            message: format!("未找到 step：{selector}"),
            exit_code: 3,
        })
}
