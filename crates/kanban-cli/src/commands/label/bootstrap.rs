use clap::Args;
use kanban_protocol::{
    BootstrapTaskLabelRequest, VectorConfigureRequest,
    cli_labels::{CliLabelBootstrapOutput, CliLabelBootstrapResult, CliLabelBootstrapVerification},
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
    /// 在 canonical host 提交前运行 provider 驱动的 staged verification。
    #[arg(long)]
    pub(crate) verify: bool,
    #[arg(long = "min-verify-score", default_value_t = 0.50)]
    pub(crate) min_verify_score: f32,
    #[arg(long = "vector-config", alias = "config")]
    pub(crate) vector_config: Option<std::path::PathBuf>,
}

pub(crate) fn run(ctx: &CliContext, args: &BootstrapArgs) -> Result<(), CliFailure> {
    let verify = args.verify || args.vector_config.is_some();
    if verify && !(0.0..=1.0).contains(&args.min_verify_score) {
        return Err(CliFailure {
            code: "invalid_input",
            message: "min_verify_score 必须在 0 到 1 之间".to_owned(),
            exit_code: 2,
        });
    }
    let vector_config = args
        .vector_config
        .as_deref()
        .map(read_vector_config)
        .transpose()?;
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
            verify,
            min_verify_score: args.min_verify_score,
            vector_config,
            actor: None,
        },
    )?;
    if ctx.json {
        output::print_json(&CliLabelBootstrapOutput {
            data: CliLabelBootstrapResult {
                task: response.data.task,
                semantics: response.data.semantics,
                verification: response.data.verification.map(|value| {
                    CliLabelBootstrapVerification {
                        label_name: value.label_name,
                        score: value.score,
                        source: value.source,
                        min_score: value.min_score,
                        degraded: value.degraded,
                        diagnostics: value.diagnostics,
                    }
                }),
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
        if let Some(verification) = response.data.verification {
            println!(
                "verification label={} score={:.3} min_score={:.3} source={}",
                verification.label_name,
                verification.score,
                verification.min_score,
                verification.source
            );
        }
    }
    Ok(())
}

fn read_vector_config(path: &std::path::Path) -> Result<VectorConfigureRequest, CliFailure> {
    let config = crate::config::read_project_config(path)?;
    let vector = config.vector.ok_or_else(|| CliFailure {
        code: "invalid_input",
        message: format!("vector config {} 缺少 [vector] 配置", path.display()),
        exit_code: 2,
    })?;
    Ok(VectorConfigureRequest {
        provider: vector.provider,
        endpoint: vector.endpoint,
        model: vector.model,
        dimensions: vector.dimensions,
    })
}
