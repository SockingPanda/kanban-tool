use clap::Args;
use kanban_protocol::{
    BootstrapTaskLabelRequest,
    cli_labels::{CliLabelBootstrapOutput, CliLabelBootstrapResult},
};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct BootstrapArgs {
    pub(crate) task_ref: String,
    pub(crate) label: String,
    #[arg(long)]
    pub(crate) description: Option<String>,
    #[arg(long = "applies-when")]
    pub(crate) applies_when: Vec<String>,
    #[arg(long = "excludes-when")]
    pub(crate) excludes_when: Vec<String>,
    #[arg(long = "positive-example")]
    pub(crate) positive_examples: Vec<String>,
    #[arg(long = "negative-example")]
    pub(crate) negative_examples: Vec<String>,
    /// Reserved for the staged vector verification flow, which is host-owned and not yet exposed.
    #[arg(long)]
    pub(crate) verify: bool,
    #[arg(long = "min-verify-score", default_value_t = 0.50)]
    pub(crate) min_verify_score: f32,
    #[arg(long = "vector-config", alias = "config")]
    pub(crate) vector_config: Option<std::path::PathBuf>,
}

pub(crate) fn run(ctx: &CliContext, args: &BootstrapArgs) -> Result<(), CliFailure> {
    if args.verify || args.vector_config.is_some() {
        return Err(crate::error::feature_not_available(
            "label bootstrap verification must run through the canonical host",
        ));
    }
    if !(0.0..=1.0).contains(&args.min_verify_score) {
        return Err(CliFailure {
            code: "invalid_input",
            message: "min_verify_score must be between 0 and 1".to_owned(),
            exit_code: 2,
        });
    }
    let client = ctx.client()?;
    let response = client.bootstrap_task_label_by_selector(
        &ctx.board,
        &args.task_ref,
        &BootstrapTaskLabelRequest {
            name: args.label.clone(),
            description: args.description.clone(),
            applies_when: args.applies_when.clone(),
            excludes_when: args.excludes_when.clone(),
            positive_examples: args.positive_examples.clone(),
            negative_examples: args.negative_examples.clone(),
            actor: None,
        },
    )?;
    if ctx.json {
        output::print_json(&CliLabelBootstrapOutput {
            data: CliLabelBootstrapResult {
                task: response.data.task,
                semantics: response.data.semantics,
                verification: None,
            },
        });
    } else {
        println!(
            "{} label={} semantics={} labels={}",
            response.data.task.task_ref,
            response.data.semantics.label_name,
            response.data.semantics.semantics_hash,
            response
                .data
                .task
                .labels
                .iter()
                .map(|label| label.name.as_str())
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    Ok(())
}
