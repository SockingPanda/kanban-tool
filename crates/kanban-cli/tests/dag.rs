mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use pretty_assertions::assert_eq;

#[test]
fn dag_show_json_reports_raw_and_derived_snapshot() -> anyhow::Result<()> {
    let temp = TempDb::new("dag_show_json_reports_raw_and_derived_snapshot")?;
    kanban(&temp.path, &["init"])?.success()?;

    let parent = create_task(&temp, "parent", "ready")?;
    let child = create_task(&temp, "child", "ready")?;
    let solo = create_task(&temp, "solo", "ready")?;
    kanban(&temp.path, &["dep", "add", &parent, &child])?.success()?;

    let output = kanban(&temp.path, &["--json", "dag", "show"])?.success_json()?;
    let data = &output["data"];
    assert_eq!(data["board"]["slug"], "default");
    assert_eq!(data["snapshot"]["node_count"], 3);
    assert_eq!(data["snapshot"]["edge_count"], 1);
    assert_eq!(data["raw"]["edges"][0]["parent"], parent);
    assert_eq!(data["raw"]["edges"][0]["child"], child);

    let frontier = data["derived"]["frontier"]
        .as_array()
        .context("expected frontier array")?;
    let frontier_ids = frontier
        .iter()
        .map(|entry| entry["task_id"].as_str().context("task_id string"))
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert_eq!(frontier_ids, vec![parent.as_str(), solo.as_str()]);
    assert!(frontier.iter().all(|entry| {
        entry["why"]
            .as_str()
            .unwrap_or_default()
            .contains("前置依赖已完成或不存在")
    }));
    assert!(
        data["raw"]["nodes"][0]["why"]
            .as_str()
            .unwrap_or_default()
            .contains("当前状态为")
    );
    assert!(
        data["raw"]["edges"][0]["why"]
            .as_str()
            .unwrap_or_default()
            .contains("必须先完成")
    );

    let human = kanban(&temp.path, &["dag", "show"])?.success_stdout()?;
    assert!(human.contains("DAG default: nodes=3 edges=1 frontier=2"));
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

#[test]
fn dag_ancestors_json_and_markdown_include_target_last() -> anyhow::Result<()> {
    let temp = TempDb::new("dag_ancestors_json_and_markdown_include_target_last")?;
    kanban(&temp.path, &["init"])?.success()?;

    let root = create_task(&temp, "root", "ready")?;
    let middle = create_task(&temp, "middle", "ready")?;
    let target = create_task(&temp, "target", "ready")?;
    kanban(&temp.path, &["dep", "add", &root, &middle])?.success()?;
    kanban(&temp.path, &["dep", "add", &middle, &target])?.success()?;

    let output = kanban(&temp.path, &["--json", "dag", "ancestors", &target])?.success_json()?;
    let data = &output["data"];
    assert_eq!(data["target"]["id"], target);
    assert_eq!(data["nodes"].as_array().context("nodes array")?.len(), 3);
    assert_eq!(data["edges"].as_array().context("edges array")?.len(), 2);
    assert_eq!(
        data["ordered_refs"]
            .as_array()
            .context("ordered refs array")?
            .last()
            .and_then(|value| value.as_str()),
        data["target"]["ref"].as_str()
    );
    assert!(data["generated_at"].as_i64().is_some());

    let human = kanban(&temp.path, &["dag", "ancestors", &target])?.success_stdout()?;
    assert!(human.contains("# Ancestors"));
    assert!(human.contains("## Ordered Tasks"));
    assert!(human.contains("- [3]"));
    assert!(human.contains("## Dependency Edges"));
    Ok(())
}

#[test]
fn dag_ancestors_missing_task_reports_clear_error() -> anyhow::Result<()> {
    let temp = TempDb::new("dag_ancestors_missing_task_reports_clear_error")?;
    kanban(&temp.path, &["init"])?.success()?;

    kanban(&temp.path, &["dag", "ancestors", "default#404"])?
        .failure_containing("not found: task default#404")?;
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
