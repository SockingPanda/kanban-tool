use clap::Args;
use kanban_contract::CliAttachmentListOutput;

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    pub(crate) task_ref: String,
}

pub(crate) fn run(ctx: &CliContext, args: &ListArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let task_id = client.resolve_task_id(&ctx.board, &args.task_ref)?;
    let attachments = client.list_attachments(&task_id)?;
    if ctx.json {
        output::print_json(&CliAttachmentListOutput { data: attachments });
    } else {
        for attachment in attachments {
            println!(
                "{} task={} {} bytes {}",
                attachment.id, attachment.task_id, attachment.size_bytes, attachment.filename
            );
        }
    }
    Ok(())
}
