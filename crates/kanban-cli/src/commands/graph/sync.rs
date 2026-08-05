use crate::{context::CliContext, error::CliFailure, output};

pub(crate) fn run(ctx: &CliContext) -> Result<(), CliFailure> {
    let status = ctx.client()?.graph_sync(&ctx.board)?;
    if ctx.json {
        output::print_json(&kanban_protocol::cli_helpers::CliGraphSyncOutput { data: status });
    } else {
        println!("{}", status.message);
    }
    Ok(())
}
