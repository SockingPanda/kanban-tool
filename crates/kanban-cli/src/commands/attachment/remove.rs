use clap::Args;
use kanban_contract::CliAttachmentRemoveOutput;

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct RemoveArgs {
    pub(crate) task_ref: String,
    pub(crate) attachment_id: String,
}

pub(crate) fn run(ctx: &CliContext, args: &RemoveArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let task_id = client.resolve_task_id(&ctx.board, &args.task_ref)?;
    let deleted = client.delete_attachment(&task_id, &args.attachment_id)?;
    if ctx.json {
        output::print_json(&CliAttachmentRemoveOutput {
            data: kanban_contract::DeleteResult { deleted },
        });
    } else {
        println!("{}", if deleted { "removed" } else { "not found" });
    }
    Ok(())
}
