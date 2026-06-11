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
            .contains("frontier")
    }));

    let human = kanban(&temp.path, &["dag", "show"])?.success_stdout()?;
    assert!(human.contains("DAG default: nodes=3 edges=1 frontier=2"));
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
