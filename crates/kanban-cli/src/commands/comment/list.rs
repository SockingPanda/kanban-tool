use crate::{context::CliContext, error::CliFailure, output};
use clap::Args;

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    pub(crate) task_ref: String,
}

pub(crate) fn run(ctx: &CliContext, args: &ListArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let comments = client.list_comments_by_selector(&ctx.board, &args.task_ref)?;
    if ctx.json {
        output::print_json(&kanban_protocol::CliCommentListOutput::new(comments));
    } else {
        for comment in comments {
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
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{Cli, Command};
    use clap::Parser;

    #[test]
    fn parses_comment_list_command() {
        let cli = Cli::try_parse_from(["kanban", "comment", "list", "default#1"])
            .expect("comment list args");
        let Command::Comment {
            command: crate::commands::comment::CommentCommand::List(args),
        } = cli.command
        else {
            panic!("expected comment list");
        };
        assert_eq!(args.task_ref, "default#1");
    }
}
