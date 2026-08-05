use crate::{context::CliContext, error::CliFailure, output};
use clap::Args;

use super::output::cli_dependency_snapshot;

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    pub(crate) task_ref: String,
}

pub(crate) fn run(ctx: &CliContext, args: &ListArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let dependencies = client.list_dependencies_by_selector(&ctx.board, &args.task_ref)?;
    if ctx.json {
        output::print_json(&kanban_contract::CliDependencyListOutput {
            data: cli_dependency_snapshot(&dependencies),
        });
    } else {
        println!("{}", dependencies.task.task_ref);
        for parent in &dependencies.parents {
            println!("  parent {} {}", parent.task_ref, parent.status.as_str());
        }
        for child in &dependencies.children {
            println!("  child {} {}", child.task_ref, child.status.as_str());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{Cli, Command};
    use clap::Parser;

    #[test]
    fn parses_dependency_list_command() {
        let cli = Cli::try_parse_from(["kanban", "dep", "list", "default#2"])
            .expect("dependency list args");
        let Command::Dependency {
            command: crate::commands::dependency::DependencyCommand::List(args),
        } = cli.command
        else {
            panic!("expected dependency list");
        };
        assert_eq!(args.task_ref, "default#2");
    }
}
