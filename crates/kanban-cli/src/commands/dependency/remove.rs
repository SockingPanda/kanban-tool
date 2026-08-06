use clap::Args;
use kanban_protocol::ApiDependencyEdge;

use crate::{context::CliContext, error::CliFailure, output};

use super::output::{api_dependency_task, cli_dependency_edge, cli_dependency_snapshot};

#[derive(Debug, Args)]
pub(crate) struct RemoveArgs {
    pub(crate) child_task_ref: String,
    pub(crate) parent_task_ref: String,
}

pub(crate) fn run(ctx: &CliContext, args: &RemoveArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let child = client.get_task_by_selector(&ctx.board, &args.child_task_ref)?;
    let parent = client.get_task_by_selector(&ctx.board, &args.parent_task_ref)?;
    let before = client.list_dependencies(&child.id)?;
    let dependencies = client.remove_dependency(&child.id, &parent.id)?;
    if ctx.json {
        let edge = before
            .edges
            .iter()
            .find(|edge| edge.child.id == child.id && edge.parent.id == parent.id)
            .cloned()
            .unwrap_or_else(|| ApiDependencyEdge {
                parent: api_dependency_task(&parent),
                child: api_dependency_task(&child),
            });
        output::print_json(&kanban_protocol::CliDependencyRemoveOutput {
            data: kanban_protocol::CliDependencyMutation {
                edge: cli_dependency_edge(&edge),
                dependencies: cli_dependency_snapshot(&dependencies),
            },
        });
    } else {
        println!(
            "已移除 {} depends_on {} ({})",
            child.task_ref,
            parent.task_ref,
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
    fn parses_dependency_remove_command() {
        let cli = Cli::try_parse_from(["kanban", "dep", "remove", "default#2", "default#1"])
            .expect("dependency remove args");
        let Command::Dependency {
            command: crate::commands::dependency::DependencyCommand::Remove(args),
        } = cli.command
        else {
            panic!("expected dependency remove");
        };
        assert_eq!(args.child_task_ref, "default#2");
        assert_eq!(args.parent_task_ref, "default#1");
    }
}
