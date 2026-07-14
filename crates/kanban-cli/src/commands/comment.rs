use std::path::PathBuf;

use anyhow::{Context, Result};
use kanban_sqlite::api::{
    CommentRecord, CreateComment, create_comment_with_options, get_task, list_comments,
};

use crate::args::CommentCommand;
use crate::commands::common::resolve_required_text_input;
use crate::output::{api_comment_from_record, print_contract_or_human, print_human};

pub(crate) fn handle_comment(
    command: CommentCommand,
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    json: bool,
) -> Result<()> {
    match command {
        CommentCommand::Add(args) => {
            let body = resolve_required_text_input(
                args.body,
                args.body_file,
                "<body>",
                "--body-file",
                "comment add",
            )?;
            let metadata_json = crate::commands::common::resolve_optional_text_input(
                args.metadata_json,
                args.metadata_json_file,
                "--metadata-json",
                "--metadata-json-file",
            )?;
            if args.kind.as_deref() == Some("decision")
                && let Some(raw) = metadata_json.as_deref()
            {
                serde_json::from_str::<kanban_contract::DecisionMetadata>(raw)
                    .context("decision metadata violates the public contract")?;
            }
            let task = get_task(db_path, board, &args.task_ref)?;
            let comment = create_comment_with_options(
                db_path,
                &task.id,
                CreateComment {
                    author: actor.to_owned(),
                    body,
                    kind: args.kind,
                    author_type: args.author_type,
                    agent_type: args.agent_type,
                    metadata_json,
                },
            )?;
            if json {
                let output =
                    kanban_contract::CliCommentAddOutput::new(api_comment_from_record(&comment)?);
                print_contract_or_human(true, &output, String::new)?;
            } else {
                print_human(|| comment_line(&comment))?;
            }
        }
        CommentCommand::List { task_ref } => {
            let comments = if task_ref.starts_with("t_") || is_board_qualified_ref(&task_ref) {
                list_comments(db_path, &task_ref)?
            } else {
                let task = get_task(db_path, board, &task_ref)?;
                list_comments(db_path, &task.id)?
            };
            let output = kanban_contract::CliCommentListOutput::new(
                comments
                    .iter()
                    .map(api_comment_from_record)
                    .collect::<Result<Vec<_>>>()?,
            );
            print_contract_or_human(json, &output, || {
                comments
                    .iter()
                    .map(comment_line)
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
        }
    }
    Ok(())
}

fn is_board_qualified_ref(task_ref: &str) -> bool {
    task_ref
        .split_once("/#")
        .or_else(|| task_ref.split_once('#'))
        .is_some_and(|(board, seq)| !board.is_empty() && !seq.is_empty())
}

fn comment_line(comment: &CommentRecord) -> String {
    format!(
        "{} task={} created_at={} [{}] {}: {}",
        comment.id,
        comment.task_id,
        comment.created_at,
        comment.kind,
        author_label(comment),
        comment.body
    )
}

fn author_label(comment: &CommentRecord) -> String {
    match comment.agent_type.as_deref() {
        Some(agent_type) => format!(
            "{} ({}/{})",
            comment.author, comment.author_type, agent_type
        ),
        None => format!("{} ({})", comment.author, comment.author_type),
    }
}
