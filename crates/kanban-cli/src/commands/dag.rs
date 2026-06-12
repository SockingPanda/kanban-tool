use std::path::PathBuf;

use anyhow::Result;
use kanban_sqlite::{DagAncestors, dag_ancestors, dag_snapshot};

use crate::{args::DagCommand, output::print_or_json};

pub(crate) fn handle_dag(
    command: DagCommand,
    db_path: &PathBuf,
    board: &str,
    json: bool,
) -> Result<()> {
    match command {
        DagCommand::Show => {
            let snapshot = dag_snapshot(db_path, board)?;
            print_or_json(json, &snapshot, || {
                format!(
                    "DAG {}: nodes={} edges={} frontier={}",
                    snapshot.board.slug,
                    snapshot.snapshot.node_count,
                    snapshot.snapshot.edge_count,
                    snapshot.derived.frontier.len()
                )
            })?;
        }
        DagCommand::Ancestors { task_ref } => {
            let ancestors = dag_ancestors(db_path, board, &task_ref)?;
            print_or_json(json, &ancestors, || ancestors_markdown(&ancestors))?;
        }
    }
    Ok(())
}

fn ancestors_markdown(ancestors: &DagAncestors) -> String {
    let mut lines = vec![
        format!("# Ancestors for {}", ancestors.target.task_ref),
        String::new(),
        format!(
            "Target: {} `{}` [{}] {}",
            ancestors.target.task_ref,
            ancestors.target.id,
            ancestors.target.status.as_str(),
            ancestors.target.title
        ),
        format!("Generated at: {}", ancestors.generated_at),
        String::new(),
        "## Ordered Tasks".to_owned(),
    ];
    for (index, node) in ancestors.nodes.iter().enumerate() {
        lines.push(format!(
            "- [{}] {} `{}` [{}] {}",
            index + 1,
            node.task_ref,
            node.id,
            node.status.as_str(),
            node.title
        ));
    }
    lines.extend([String::new(), "## Dependency Edges".to_owned()]);
    if ancestors.edges.is_empty() {
        lines.push("- none".to_owned());
    } else {
        for edge in &ancestors.edges {
            lines.push(format!(
                "- `{}` -> `{}`: {}",
                edge.parent, edge.child, edge.why
            ));
        }
    }
    lines.join("\n")
}
