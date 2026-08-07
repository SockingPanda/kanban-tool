use crate::{context::CliContext, error::CliFailure, output};

pub(crate) fn run(ctx: &CliContext) -> Result<(), CliFailure> {
    let status = ctx.client()?.graph_status(&ctx.board)?;
    let output_value = kanban_protocol::cli_helpers::CliGraphStatus {
        backend: status.backend,
        enabled: status.enabled,
        message: status.message,
    };
    if ctx.json {
        output::print_json(&kanban_protocol::cli_helpers::CliGraphStatusOutput {
            data: output_value,
        });
    } else {
        println!(
            "{} enabled={} {}",
            output_value.backend, output_value.enabled, output_value.message
        );
    }
    Ok(())
}
