use std::path::PathBuf;

use clap::Args;
use kanban_protocol::{CliAttachmentAddOutput, CreateAttachmentRequest};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct AddArgs {
    pub(crate) task_ref: String,
    pub(crate) path: PathBuf,
    #[arg(long)]
    pub(crate) filename: Option<String>,
    #[arg(long)]
    pub(crate) content_type: Option<String>,
    #[arg(long)]
    pub(crate) attachment_id: Option<String>,
}

pub(crate) fn run(ctx: &CliContext, args: &AddArgs) -> Result<(), CliFailure> {
    let content = std::fs::read(&args.path).map_err(|error| CliFailure {
        code: "invalid_input",
        message: format!(
            "cannot read attachment file {}: {error}",
            args.path.display()
        ),
        exit_code: 2,
    })?;
    let filename = args
        .filename
        .clone()
        .or_else(|| {
            args.path
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
        })
        .ok_or_else(|| CliFailure {
            code: "invalid_input",
            message: "attachment path has no filename".to_owned(),
            exit_code: 2,
        })?;
    let client = ctx.client()?;
    let attachment = client.create_attachment(
        &client.resolve_task_id(&ctx.board, &args.task_ref)?,
        &CreateAttachmentRequest {
            id: args.attachment_id.clone(),
            filename,
            content,
            content_type: args.content_type.clone(),
            rel_path: None,
            sha256: None,
            actor: Some(ctx.actor()),
        },
    )?;
    if ctx.json {
        output::print_json(&CliAttachmentAddOutput { data: attachment });
    } else {
        println!("{} {} bytes", attachment.id, attachment.size_bytes);
    }
    Ok(())
}
