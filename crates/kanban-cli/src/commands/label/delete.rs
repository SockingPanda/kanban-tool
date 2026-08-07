use clap::Args;
use kanban_protocol::cli_labels::{CliLabelDeleteOutput, CliLabelDeleteResult};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct DeleteArgs {
    /// label ID 或名称。
    pub(crate) label: String,
    /// 即使 label 仍绑定任务，也先移除任务绑定再删除 identity。
    #[arg(long)]
    pub(crate) force: bool,
}

pub(crate) fn run(ctx: &CliContext, args: &DeleteArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let result = client.delete_board_label(&ctx.board, &args.label, args.force)?;
    if ctx.json {
        output::print_json(&CliLabelDeleteOutput {
            data: CliLabelDeleteResult {
                label: result.label,
                forced: result.forced,
                removed_task_bindings: result.removed_task_bindings,
                removed_semantics: result.removed_semantics,
                removed_atoms: result.removed_atoms,
            },
        });
    } else {
        println!(
            "已删除 {} forced={} removed_task_bindings={} removed_semantics={} removed_atoms={}",
            result.label.name,
            result.forced,
            result.removed_task_bindings,
            result.removed_semantics,
            result.removed_atoms
        );
    }
    Ok(())
}
