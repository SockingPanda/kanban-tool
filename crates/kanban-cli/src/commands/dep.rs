use std::path::PathBuf;

use anyhow::Result;
use kanban_sqlite::{add_dependency, list_dependencies, remove_dependency};

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
            print_or_json(
                json,
                &serde_json::json!({"parent": parent_ref, "child": child_ref}),
                || format!("已添加依赖：{parent_ref} -> {child_ref}"),
            )?;
        }
        DepCommand::Remove {
            parent_ref,
            child_ref,
        } => {
            remove_dependency(db_path, board, actor, &parent_ref, &child_ref)?;
            print_or_json(
                json,
                &serde_json::json!({"parent": parent_ref, "child": child_ref}),
                || format!("已移除依赖：{parent_ref} -> {child_ref}"),
            )?;
        }
        DepCommand::List { task_ref } => {
            let deps = list_dependencies(db_path, board, &task_ref)?;
            print_or_json(json, &deps, || {
                deps.iter()
                    .map(|(p, c)| format!("{} -> {}", p, c))
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
        }
    }
    Ok(())
}
