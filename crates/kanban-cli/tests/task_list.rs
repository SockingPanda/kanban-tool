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
fn task_list_filters_by_repeatable_labels() -> anyhow::Result<()> {
    let temp = TempDb::new("task_list_filters_by_repeatable_labels")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(&temp.path, &["label", "create", "backend"])?.success()?;
    kanban(&temp.path, &["label", "create", "api"])?.success()?;
    kanban(
        &temp.path,
        &[
            "task",
            "create",
            "backend only",
            "--description",
            "ready spec",
            "--label",
            "backend",
        ],
    )?
    .success()?;
    kanban(
        &temp.path,
        &[
            "task",
            "create",
            "backend api",
            "--description",
            "ready spec",
            "--label",
            "backend",
            "--label",
            "api",
        ],
    )?
    .success()?;

    let tasks = kanban(
        &temp.path,
        &[
            "--json", "task", "list", "--label", "backend", "--label", "api",
        ],
    )?
    .success_json()?;
    let data = tasks["data"].as_array().context("expected JSON array")?;
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["title"], "backend api");
    assert_eq!(data[0]["labels"][0]["name"], "api");
    assert_eq!(data[0]["labels"][1]["name"], "backend");
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
fn task_list_search_matches_task_refs_exactly() -> anyhow::Result<()> {
    let temp = TempDb::new("task_list_search_matches_task_refs_exactly")?;
    kanban(&temp.path, &["init"])?.success()?;
    let first = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "first cli list task",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    kanban(
        &temp.path,
        &[
            "task",
            "create",
            "title mentions 1 but numeric list search is exact",
            "--description",
            "ready spec",
        ],
    )?
    .success()?;
    let first_id = first["data"]["id"].as_str().context("task id")?;

    for query in ["1", "#1", "default#1", first_id] {
        let json =
            kanban(&temp.path, &["--json", "task", "list", "--search", query])?.success_json()?;
        let tasks = json["data"].as_array().context("expected JSON array")?;
        assert_eq!(tasks.len(), 1, "{query}: {json}");
        assert_eq!(tasks[0]["id"], first_id, "{query}: {json}");
    }

    let json = kanban(
        &temp.path,
        &["--json", "task", "list", "--search", "other#1"],
    )?
    .success_json()?;
    assert!(
        json["data"]
            .as_array()
            .context("expected JSON array")?
            .is_empty()
    );
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
