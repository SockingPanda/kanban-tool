use clap::Args;

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct QueryArgs {
    pub(crate) query: String,
    #[arg(long, default_value_t = 100)]
    pub(crate) limit: usize,
}

pub(crate) fn run(ctx: &CliContext, args: &QueryArgs) -> Result<(), CliFailure> {
    let response = ctx
        .client()?
        .graph_query(&ctx.board, &args.query, args.limit)?;
    if ctx.json {
        output::print_json(&response);
    } else {
        for row in response.data {
            println!(
                "{}",
                row.bindings
                    .into_iter()
                    .map(|binding| format!("{}={}", binding.name, binding.value))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
    }
    Ok(())
}
