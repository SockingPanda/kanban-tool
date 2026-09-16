use crate::{context::CliContext, error::CliFailure, output};

pub(crate) async fn run(ctx: &CliContext) -> Result<(), CliFailure> {
    let status = ctx.client()?.graph_rebuild(&ctx.board).await?;
    if ctx.json {
        output::print_json(&kanban_protocol::cli_helpers::CliGraphRebuildOutput { data: status });
    } else {
        println!("{}", status.message);
    }
    Ok(())
}
