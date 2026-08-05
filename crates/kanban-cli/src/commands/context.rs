use clap::{Args, Subcommand};
use kanban_contract::{BuildContextQuery, CliContextBuildOutput};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Subcommand)]
pub(crate) enum ContextCommand {
    /// 构建 task/reference/query 的只读混合上下文包。
    Build(BuildArgs),
}

#[derive(Debug, Args)]
pub(crate) struct BuildArgs {
    /// 全局 task id 或 board-local reference；query-only 时可省略。
    pub(crate) subject: Option<String>,
    #[arg(long)]
    pub(crate) task: Option<String>,
    #[arg(long)]
    pub(crate) reference: Option<String>,
    #[arg(long)]
    pub(crate) query: Option<String>,
    #[arg(long, default_value_t = 1)]
    pub(crate) depth: usize,
    #[arg(long, default_value_t = 5)]
    pub(crate) lexical_limit: usize,
    #[arg(long, default_value_t = 10)]
    pub(crate) graph_limit: usize,
    #[arg(long, default_value_t = 5)]
    pub(crate) vector_limit: usize,
    #[arg(long, default_value_t = 20)]
    pub(crate) max_items: usize,
    #[arg(long)]
    pub(crate) budget: Option<usize>,
}

pub(crate) fn run(ctx: &CliContext, command: &ContextCommand) -> Result<(), CliFailure> {
    let ContextCommand::Build(args) = command;
    let client = ctx.client()?;
    let mut task = args.task.clone();
    let mut reference = args.reference.clone();
    let query = args.query.clone();
    let path_subject = args.subject.as_deref().unwrap_or("query");
    if args.subject.is_some() && task.is_none() && reference.is_none() && query.is_none() {
        if path_subject.starts_with("t_") {
            task = Some(path_subject.to_owned());
        } else {
            reference = Some(path_subject.to_owned());
        }
    }
    if task.is_none() && reference.is_none() && query.is_none() {
        return Err(CliFailure {
            code: "invalid_input",
            message: "one of subject, --task, --reference or --query is required".to_owned(),
            exit_code: 2,
        });
    }
    let response = client.build_context(
        path_subject,
        &BuildContextQuery {
            board: ctx.board.clone(),
            lexical_limit: args.lexical_limit,
            graph_limit: args.graph_limit,
            vector_limit: args.vector_limit,
            max_items: args.max_items,
            task,
            reference,
            query,
            depth: args.depth,
            budget: args.budget,
        },
    )?;
    if ctx.json {
        output::print_json(&CliContextBuildOutput::new(response.data));
    } else {
        for item in response.data.items {
            println!(
                "{} {} {} [{:?}] {}",
                item.rank, item.source, item.entity_uri, item.score, item.reason
            );
        }
        if response.data.truncated {
            println!(
                "truncated: {}",
                response
                    .data
                    .truncation_reason
                    .as_deref()
                    .unwrap_or("unknown")
            );
        }
        for provider in response.data.providers {
            println!(
                "provider {} capability={} available={} degraded={} reason={}",
                provider.provider,
                provider.capability,
                provider.available,
                provider.degraded,
                provider.reason.as_deref().unwrap_or("-")
            );
        }
    }
    Ok(())
}
