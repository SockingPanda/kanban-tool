mod common;

use anyhow::Context;
use common::{TempDb, kanban};

#[test]
fn dag_command_is_removed_from_help_and_rejected() -> anyhow::Result<()> {
    let temp = TempDb::new("dag_command_is_removed_from_help_and_rejected")?;

    let stdout = kanban(&temp.path, &["--help"])?.success_stdout()?;
    assert!(
        !stdout
            .lines()
            .any(|line| line.trim_start().starts_with("dag")),
        "{stdout}"
    );

    kanban(&temp.path, &["dag", "show"])?.failure_containing_any(&[
        "unrecognized subcommand",
        "unexpected argument",
        "invalid subcommand",
    ])?;
    Ok(())
}

#[test]
fn dep_add_remove_human_output_is_chinese() -> anyhow::Result<()> {
    let temp = TempDb::new("dep_add_remove_human_output_is_chinese")?;
    kanban(&temp.path, &["init"])?.success()?;

    let parent = create_task(&temp, "parent", "ready")?;
    let child = create_task(&temp, "child", "ready")?;

    let added = kanban(&temp.path, &["dep", "add", &parent, &child])?.success_stdout()?;
    assert!(added.contains("已添加依赖"));
    assert!(added.contains(&parent));
    assert!(added.contains(&child));

    let removed = kanban(&temp.path, &["dep", "remove", &parent, &child])?.success_stdout()?;
    assert!(removed.contains("已移除依赖"));
    assert!(removed.contains(&parent));
    assert!(removed.contains(&child));
    Ok(())
}

fn create_task(temp: &TempDb, title: &str, status: &str) -> anyhow::Result<String> {
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            title,
            "--description",
            "ready spec",
            "--status",
            status,
        ],
    )?
    .success_json()?;
    Ok(created["data"]["id"]
        .as_str()
        .context("expected task id")?
        .to_owned())
}
