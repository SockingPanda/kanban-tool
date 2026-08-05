use crate::{context::CliContext, error::CliFailure, output};
use clap::Args;

use super::output::{cli_dependency_edge, cli_dependency_snapshot};

#[derive(Debug, Args)]
pub(crate) struct AddArgs {
    pub(crate) child_task_ref: String,
    pub(crate) parent_task_ref: String,
}

pub(crate) fn run(ctx: &CliContext, args: &AddArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let parent_id = client
        .get_task_by_selector(&ctx.board, &args.parent_task_ref)?
        .id;
    let dependencies = client.add_dependency_by_selector(
        &ctx.board,
        &args.child_task_ref,
        &args.parent_task_ref,
    )?;
    if ctx.json {
        let edge = dependencies
            .edges
            .iter()
            .find(|edge| edge.child.id == dependencies.task.id && edge.parent.id == parent_id)
            .cloned()
            .ok_or_else(|| CliFailure {
                code: "invalid_response",
                message: "dependency add response omitted the new edge".to_owned(),
                exit_code: 2,
            })?;
        output::print_json(&kanban_protocol::CliDependencyAddOutput {
            data: kanban_protocol::CliDependencyMutation {
                edge: cli_dependency_edge(&edge),
                dependencies: cli_dependency_snapshot(&dependencies),
            },
        });
    } else {
        println!(
            "{} depends_on {} ({})",
            dependencies.task.task_ref,
            args.parent_task_ref,
            dependencies.task.status.as_str()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{Cli, Command};
    use clap::Parser;

    #[test]
    fn parses_dependency_add_command() {
        let cli = Cli::try_parse_from(["kanban", "dep", "add", "default#2", "default#1"])
            .expect("dependency add args");
        let Command::Dependency {
            command: crate::commands::dependency::DependencyCommand::Add(args),
        } = cli.command
        else {
            panic!("expected dependency add");
        };
        assert_eq!(args.child_task_ref, "default#2");
        assert_eq!(args.parent_task_ref, "default#1");
    }
}
