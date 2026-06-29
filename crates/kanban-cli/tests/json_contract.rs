mod common;

use anyhow::Context;
use common::{TempDb, kanban};

#[test]
fn board_current_and_step_remove_json_return_named_objects() -> anyhow::Result<()> {
    let temp = TempDb::new("board_current_and_step_remove_json_return_named_objects")?;
    kanban(&temp.path, &["init"])?.success()?;

    let current = kanban(&temp.path, &["--json", "board", "current"])?.success_json()?;
    assert_eq!(current["data"]["board"]["slug"], "default");
    assert!(
        current["data"]["board"]["id"]
            .as_str()
            .context("board id")?
            .starts_with("b_")
    );

    let task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "remove step target",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;
    let step = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "step",
            "add",
            task_id,
            "install fixture",
            "--body",
            "smoke evidence",
        ],
    )?
    .success_json()?;
    let step_id = step["data"]["id"].as_str().context("step id")?;

    let removed = kanban(
        &temp.path,
        &["--json", "task", "step", "remove", task_id, step_id],
    )?
    .success_json()?;
    assert_eq!(removed["data"]["removed"], true);
    assert_eq!(removed["data"]["step"]["id"], step_id);
    assert_eq!(removed["data"]["step"]["parent_task_id"], task_id);
    assert_eq!(removed["data"]["step"]["title"], "install fixture");
    assert!(removed["data"].get("task_ref").is_none());
    assert!(removed["data"].get("step_ref").is_none());
    Ok(())
}
