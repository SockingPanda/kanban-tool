use std::path::PathBuf;

use anyhow::Result;
use kanban_sqlite::{
    CommentRecord, CreateComment, create_comment_with_options, get_task, list_comments,
};

use crate::args::CommentCommand;
use crate::output::print_or_json;

pub(crate) fn handle_comment(
    command: CommentCommand,
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    json: bool,
) -> Result<()> {
    match command {
        CommentCommand::Add(args) => {
            let task = get_task(db_path, board, &args.task_ref)?;
            let comment = create_comment_with_options(
                db_path,
                &task.id,
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
            let comments = if task_ref.starts_with("t_") || is_board_qualified_ref(&task_ref) {
                list_comments(db_path, &task_ref)?
            } else {
                let task = get_task(db_path, board, &task_ref)?;
                list_comments(db_path, &task.id)?
            };
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
