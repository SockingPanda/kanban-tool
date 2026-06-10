use std::path::PathBuf;

use anyhow::Result;
use kanban_sqlite::{CommentRecord, CreateComment, create_comment_with_options, list_comments};

use crate::args::CommentCommand;
use crate::output::print_or_json;

pub(crate) fn handle_comment(
    command: CommentCommand,
    db_path: &PathBuf,
    actor: &str,
    json: bool,
) -> Result<()> {
    match command {
        CommentCommand::Add(args) => {
            let comment = create_comment_with_options(
                db_path,
                &args.task_ref,
                CreateComment {
                    author: actor.to_owned(),
                    body: args.body,
                    kind: args.kind,
                    author_type: args.author_type,
                    agent_type: args.agent_type,
                },
            )?;
            print_or_json(json, &comment, || comment_line(&comment))?;
        }
        CommentCommand::List { task_ref } => {
            let comments = list_comments(db_path, &task_ref)?;
            print_or_json(json, &comments, || {
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
