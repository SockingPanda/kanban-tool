mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use pretty_assertions::assert_eq;
#[test]
fn task_list_supports_search_assignee_sort_limit_and_offset() -> anyhow::Result<()> {
    let temp = TempDb::new("task_list_supports_search_assignee_sort_limit_and_offset")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(
        &temp.path,
        &[
            "task",
            "create",
            "Alpha search match",
            "--description",
            "ready spec search-term",
            "--assignee",
            "worker-a",
            "--priority",
            "1",
        ],
    )?
    .success()?;
    kanban(
        &temp.path,
        &[
            "task",
            "create",
            "Beta search match",
            "--description",
            "ready spec search-term",
            "--assignee",
            "worker-a",
            "--priority",
            "3",
        ],
    )?
    .success()?;
    kanban(
        &temp.path,
        &[
            "task",
            "create",
            "Gamma search match",
            "--description",
            "ready spec search-term",
            "--assignee",
            "worker-b",
            "--priority",
            "2",
        ],
    )?
    .success()?;

    let tasks = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "list",
            "--search",
            "search-term",
            "--assignee",
            "worker-a",
            "--sort",
            "priority_desc",
            "--limit",
            "1",
            "--offset",
            "1",
        ],
    )?
    .success_json()?;

    let data = tasks["data"].as_array().context("expected JSON array")?;
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["title"], "Alpha search match");
    Ok(())
}

#[test]
fn task_list_supports_expanded_table_sort_fields() -> anyhow::Result<()> {
    let temp = TempDb::new("task_list_supports_expanded_table_sort_fields")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(
        &temp.path,
        &[
            "task",
            "create",
            "Alpha table sort",
            "--description",
            "ready spec",
            "--assignee",
            "worker-a",
        ],
    )?
    .success()?;
    kanban(
        &temp.path,
        &[
            "task",
            "create",
            "Beta table sort",
            "--description",
            "ready spec",
            "--assignee",
            "worker-b",
        ],
    )?
    .success()?;

    let title_desc = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "list",
            "--search",
            "table sort",
            "--sort",
            "title_desc",
        ],
    )?
    .success_json()?;
    let title_data = title_desc["data"]
        .as_array()
        .context("expected JSON array")?;
    assert_eq!(title_data[0]["title"], "Beta table sort");

    let api_style_desc = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "list",
            "--search",
            "table sort",
            "--sort=-assignee",
        ],
    )?
    .success_json()?;
    let assignee_data = api_style_desc["data"]
        .as_array()
        .context("expected JSON array")?;
    assert_eq!(assignee_data[0]["assignee"], "worker-b");

    Ok(())
}

#[test]
fn task_list_command_rejects_unbounded_limit() -> anyhow::Result<()> {
    let temp = TempDb::new("task_list_command_rejects_unbounded_limit")?;
    kanban(&temp.path, &["init"])?.success()?;

    kanban(
        &temp.path,
        &["task", "list", "--limit", &usize::MAX.to_string()],
    )?
    .failure_containing("limit must be <= 1000")?;
    Ok(())
}
