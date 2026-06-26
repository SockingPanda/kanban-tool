mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use pretty_assertions::assert_eq;

fn mark_no_plan_required(db_path: &std::path::Path, task_id: &str) -> anyhow::Result<()> {
    kanban_sqlite::mark_execution_plan_not_required(
        db_path,
        "default",
        "cli-stats-test",
        task_id,
        "stats fixture does not need subtasks",
    )?;
    Ok(())
}

#[test]
fn stats_command_reports_stale_claims_and_blocked_reasons() -> anyhow::Result<()> {
    let temp = TempDb::new("stats_command_reports_stale_claims_and_blocked_reasons")?;
    kanban(&temp.path, &["init"])?.success()?;
    let stale = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "stale cli",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let stale_id = stale["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    mark_no_plan_required(&temp.path, stale_id)?;
    kanban(
        &temp.path,
        &["--json", "task", "claim", stale_id, "--ttl-ms", "1"],
    )?
    .success_json()?;
    let blocked = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "blocked cli",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let blocked_id = blocked["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "block",
            blocked_id,
            "operator needed",
            "--force",
        ],
    )?
    .success_json()?;
    std::thread::sleep(std::time::Duration::from_millis(5));

    let stats = kanban(&temp.path, &["--json", "stats"])?.success_json()?;

    assert_eq!(stats["data"]["unplanned_active_tasks"], 1);
    assert_eq!(
        stats["data"]["active_parents_with_incomplete_required_subtasks"],
        0
    );
    assert_eq!(stats["data"]["stale_claims"][0]["task_id"], stale_id);
    assert_eq!(
        stats["data"]["blocked_reasons"][0]["reason"],
        "operator needed"
    );
    assert_eq!(stats["data"]["blocked_reasons"][0]["count"], 1);
    Ok(())
}
