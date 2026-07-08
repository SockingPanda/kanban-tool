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

#[test]
fn locale_does_not_change_runtime_json_error_machine_fields() -> anyhow::Result<()> {
    let temp = TempDb::new("locale_does_not_change_runtime_json_error_machine_fields")?;
    kanban(&temp.path, &["init"])?.success()?;

    let zh = kanban(
        &temp.path,
        &[
            "--locale",
            "zh-CN",
            "--json",
            "board",
            "show",
            "missing-board",
        ],
    )?;
    let en = kanban(
        &temp.path,
        &["--locale", "en", "--json", "board", "show", "missing-board"],
    )?;

    assert_eq!(zh.output.status.code(), Some(3));
    assert_eq!(en.output.status.code(), Some(3));
    assert!(zh.output.stderr.is_empty());
    assert!(en.output.stderr.is_empty());

    let zh: serde_json::Value = serde_json::from_slice(&zh.output.stdout)?;
    let en: serde_json::Value = serde_json::from_slice(&en.output.stdout)?;
    assert_eq!(zh["error"]["code"], "not_found");
    assert_eq!(en["error"]["code"], "not_found");
    assert_eq!(zh["error"]["exit_code"], 3);
    assert_eq!(en["error"]["exit_code"], 3);
    assert!(
        zh["error"]["message"]
            .as_str()
            .context("zh message")?
            .contains("未找到")
    );
    assert!(
        en["error"]["message"]
            .as_str()
            .context("en message")?
            .contains("not found")
    );
    Ok(())
}
