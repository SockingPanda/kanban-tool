use std::path::PathBuf;

use anyhow::Result;
use kanban_sqlite::{
    DependencyMutation, add_dependency, dependency_edge, dependency_snapshot, remove_dependency,
};

use crate::args::DepCommand;
use crate::output::print_or_json;

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
            print_or_json(json, &output, || {
                format!("已添加依赖：{parent_ref} -> {child_ref}")
            })?;
        }
        DepCommand::Remove {
            parent_ref,
            child_ref,
        } => {
            let edge = dependency_edge(db_path, board, &parent_ref, &child_ref)?;
            remove_dependency(db_path, board, actor, &parent_ref, &child_ref)?;
            let dependencies = dependency_snapshot(db_path, board, &child_ref)?;
            let output = DependencyMutation { edge, dependencies };
            print_or_json(json, &output, || {
                format!("已移除依赖：{parent_ref} -> {child_ref}")
            })?;
        }
        DepCommand::List { task_ref } => {
            let snapshot = dependency_snapshot(db_path, board, &task_ref)?;
            print_or_json(json, &snapshot, || {
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
