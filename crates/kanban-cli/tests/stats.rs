mod common;

use anyhow::Context;
use common::{TempDb, kb};
use pretty_assertions::assert_eq;
#[test]
fn stats_command_reports_stale_claims_and_blocked_reasons() -> anyhow::Result<()> {
    let temp = TempDb::new("stats_command_reports_stale_claims_and_blocked_reasons")?;
    kb(&temp.path, &["init"])?.success()?;
    let stale = kb(
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
    kb(
        &temp.path,
        &["--json", "task", "claim", stale_id, "--ttl-ms", "1"],
    )?
    .success_json()?;
    let blocked = kb(
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
    kb(
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

    let stats = kb(&temp.path, &["--json", "stats"])?.success_json()?;

    assert_eq!(stats["data"]["stale_claims"][0]["task_id"], stale_id);
    assert_eq!(
        stats["data"]["blocked_reasons"][0]["reason"],
        "operator needed"
    );
    assert_eq!(stats["data"]["blocked_reasons"][0]["count"], 1);
    Ok(())
}
