use crate::{context::CliContext, error::CliFailure, output};
use clap::Args as ClapArgs;
use kanban_client::KanbanClient;
use kanban_protocol::{
    VectorConfigureRequest,
    cli_helpers::{CliVectorConfig, CliVectorConfigureOutput},
};

#[derive(Debug, ClapArgs)]
pub(crate) struct Args {
    #[arg(long, default_value = "ollama")]
    pub(crate) provider: String,
    #[arg(long, default_value = "http://127.0.0.1:11434")]
    pub(crate) endpoint: String,
    #[arg(long)]
    pub(crate) model: String,
    #[arg(long)]
    pub(crate) dimensions: usize,
}

pub(crate) fn run(ctx: &CliContext, client: &KanbanClient, args: &Args) -> Result<(), CliFailure> {
    let value = client.configure_vector(VectorConfigureRequest {
        provider: args.provider.clone(),
        endpoint: args.endpoint.clone(),
        model: args.model.clone(),
        dimensions: args.dimensions,
    })?;
    let output_value = CliVectorConfigureOutput::new(CliVectorConfig {
        provider: value.provider,
        endpoint: value.endpoint,
        model: value.model,
        dimensions: value.dimensions,
    });
    if ctx.json {
        output::print_json(&output_value);
    } else {
        println!("vector 已配置：{} {}", args.model, args.dimensions);
    }
    Ok(())
}
