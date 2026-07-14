use std::path::PathBuf;

use anyhow::Result;
use kanban_core::i18n::{current_locale, dep_added, dep_removed};
use kanban_sqlite::api::{
    DependencyMutation, add_dependency, dependency_edge, dependency_snapshot, remove_dependency,
};

use crate::args::DepCommand;
use crate::output::{
    cli_dependency_mutation, cli_dependency_snapshot, print_contract_or_human, print_human,
};

pub(crate) fn handle_dep(
    command: DepCommand,
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    json: bool,
) -> Result<()> {
    match command {
        DepCommand::Add {
            parent_ref,
            child_ref,
        } => {
            add_dependency(db_path, board, actor, &parent_ref, &child_ref)?;
            let edge = dependency_edge(db_path, board, &parent_ref, &child_ref)?;
            let dependencies = dependency_snapshot(db_path, board, &child_ref)?;
            let output = DependencyMutation { edge, dependencies };
            if json {
                let contract =
                    kanban_contract::CliDependencyAddOutput::new(cli_dependency_mutation(&output));
                print_contract_or_human(true, &contract, String::new)?;
            } else {
                print_human(|| dep_added(current_locale(), &parent_ref, &child_ref))?;
            }
        }
        DepCommand::Remove {
            parent_ref,
            child_ref,
        } => {
            let edge = dependency_edge(db_path, board, &parent_ref, &child_ref)?;
            remove_dependency(db_path, board, actor, &parent_ref, &child_ref)?;
            let dependencies = dependency_snapshot(db_path, board, &child_ref)?;
            let output = DependencyMutation { edge, dependencies };
            if json {
                let contract = kanban_contract::CliDependencyRemoveOutput::new(
                    cli_dependency_mutation(&output),
                );
                print_contract_or_human(true, &contract, String::new)?;
            } else {
                print_human(|| dep_removed(current_locale(), &parent_ref, &child_ref))?;
            }
        }
        DepCommand::List { task_ref } => {
            let snapshot = dependency_snapshot(db_path, board, &task_ref)?;
            let output =
                kanban_contract::CliDependencyListOutput::new(cli_dependency_snapshot(&snapshot));
            print_contract_or_human(json, &output, || {
                snapshot
                    .edges
                    .iter()
                    .map(|edge| format!("{} -> {}", edge.parent.task_ref, edge.child.task_ref))
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
        }
    }
    Ok(())
}
