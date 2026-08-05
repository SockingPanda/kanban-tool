use std::collections::BTreeMap;

use clap::{Args, ValueEnum};
use kanban_contract::{CommentAuthorType, CommentKind, CreateCommentRequest};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct AddArgs {
    pub(crate) task_ref: String,
    pub(crate) body: String,
    #[arg(long, value_enum)]
    pub(crate) kind: Option<CommentKindArg>,
    #[arg(long)]
    pub(crate) author: Option<String>,
    #[arg(long, value_enum)]
    pub(crate) author_type: Option<CommentAuthorTypeArg>,
    #[arg(long)]
    pub(crate) agent_type: Option<String>,
    /// JSON object stored as comment metadata.
    #[arg(long = "metadata-json")]
    pub(crate) metadata_json: Option<String>,
    #[arg(long)]
    pub(crate) idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum CommentKindArg {
    Note,
    Decision,
    Signal,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum CommentAuthorTypeArg {
    User,
    Agent,
}

pub(crate) fn run(ctx: &CliContext, args: &AddArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let metadata = parse_metadata(args.metadata_json.as_deref())?;
    let comment = client.create_comment_by_selector(
        &ctx.board,
        &args.task_ref,
        &CreateCommentRequest {
            idempotency_key: args.idempotency_key.clone(),
            author: args.author.clone(),
            body: args.body.clone(),
            kind: args.kind.map(api_comment_kind),
            author_type: args.author_type.map(api_comment_author_type),
            agent_type: args.agent_type.clone(),
            metadata: metadata
                .map(|metadata| serde_json::Value::Object(metadata.into_iter().collect())),
        },
    )?;
    if ctx.json {
        output::print_json(&kanban_contract::CliCommentAddOutput::new(comment));
    } else {
        println!(
            "{} task={} created_at={} [{}] {} ({}): {}",
            comment.id,
            comment.task_id,
            comment.created_at,
            comment.kind.as_str(),
            comment.author,
            comment.author_type.as_str(),
            comment.body
        );
    }
    Ok(())
}

fn api_comment_kind(kind: CommentKindArg) -> CommentKind {
    match kind {
        CommentKindArg::Note => CommentKind::Note,
        CommentKindArg::Decision => CommentKind::Decision,
        CommentKindArg::Signal => CommentKind::Signal,
    }
}

fn api_comment_author_type(author_type: CommentAuthorTypeArg) -> CommentAuthorType {
    match author_type {
        CommentAuthorTypeArg::User => CommentAuthorType::User,
        CommentAuthorTypeArg::Agent => CommentAuthorType::Agent,
    }
}

fn parse_metadata(
    metadata: Option<&str>,
) -> Result<Option<BTreeMap<String, serde_json::Value>>, CliFailure> {
    metadata
        .map(|metadata| {
            serde_json::from_str(metadata).map_err(|error| CliFailure {
                code: "invalid_input",
                message: format!("--metadata must be a JSON object: {error}"),
                exit_code: 2,
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::{CommentAuthorTypeArg, CommentKindArg};
    use crate::{Cli, Command};
    use clap::Parser;

    #[test]
    fn parses_comment_add_command() {
        let cli = Cli::try_parse_from([
            "kanban",
            "comment",
            "add",
            "default#1",
            "handoff",
            "--kind",
            "decision",
            "--author-type",
            "agent",
            "--agent-type",
            "executor",
            "--metadata-json",
            "{\"options\":[]}",
        ])
        .expect("comment add args");
        let Command::Comment {
            command: crate::commands::comment::CommentCommand::Add(args),
        } = cli.command
        else {
            panic!("expected comment add");
        };
        assert_eq!(args.task_ref, "default#1");
        assert_eq!(args.body, "handoff");
        assert!(matches!(args.kind, Some(CommentKindArg::Decision)));
        assert!(matches!(
            args.author_type,
            Some(CommentAuthorTypeArg::Agent)
        ));
    }
}
