use std::path::PathBuf;

use clap::Args;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Args)]
pub(crate) struct DownloadArgs {
    pub(crate) task_ref: String,
    pub(crate) attachment_id: String,
    pub(crate) out: PathBuf,
}

pub(crate) fn run(ctx: &CliContext, args: &DownloadArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let task_id = client.resolve_task_id(&ctx.board, &args.task_ref)?;
    let downloaded = client.download_attachment(&task_id, &args.attachment_id)?;
    std::fs::write(&args.out, downloaded.content).map_err(|error| CliFailure {
        code: "storage",
        message: format!("写入 {} 失败：{error}", args.out.display()),
        exit_code: 1,
    })?;
    if !ctx.json {
        println!("{}", args.out.display());
    }
    Ok(())
}
