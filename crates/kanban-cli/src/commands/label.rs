use std::path::PathBuf;

use anyhow::Result;
use kanban_sqlite::{CreateLabel, add_task_label, create_label, list_labels, remove_task_label};

use crate::args::LabelCommand;
use crate::output::{label_line, print_or_json, print_task};

pub(crate) fn handle_label(
    command: LabelCommand,
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    json: bool,
) -> Result<()> {
    match command {
        LabelCommand::List => {
            let labels = list_labels(db_path, board)?;
            print_or_json(json, &labels, || {
                labels.iter().map(label_line).collect::<Vec<_>>().join("\n")
            })?;
        }
        LabelCommand::Create(args) => {
            let label = create_label(
                db_path,
                board,
                CreateLabel {
                    name: args.name,
                    color: args.color,
                },
            )?;
            print_or_json(json, &label, || label_line(&label))?;
        }
        LabelCommand::Add(args) => {
            let task = add_task_label(db_path, board, actor, &args.task_ref, &args.label)?;
            print_task(json, &task)?;
        }
        LabelCommand::Remove(args) => {
            let task = remove_task_label(db_path, board, actor, &args.task_ref, &args.label)?;
            print_task(json, &task)?;
        }
    }
    Ok(())
}
