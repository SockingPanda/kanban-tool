mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use pretty_assertions::assert_eq;

#[test]
fn task_show_defaults_to_one_line_summary() -> anyhow::Result<()> {
    let temp = TempDb::new("task_show_defaults_to_one_line_summary")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "show summary title",
            "--description",
            "line one\nline two",
            "--assignee",
            "operator",
            "--priority",
            "7",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;

    let stdout = kanban(&temp.path, &["task", "show", task_id])?.success_stdout()?;

    assert_eq!(
        stdout,
        format!("default#1 {task_id} [ready] show summary title\n")
    );
    assert!(!stdout.contains("line one"), "{stdout}");
    assert_eq!(stdout.lines().count(), 1);
    Ok(())
}

#[test]
fn task_show_details_prints_full_readable_record() -> anyhow::Result<()> {
    let temp = TempDb::new("task_show_details_prints_full_readable_record")?;
    kanban(&temp.path, &["init"])?.success()?;
    let description = "first detail line\nsecond detail line";
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "detailed task title",
            "--description",
            description,
            "--assignee",
            "executor",
            "--priority",
            "42",
            "--scheduled-at",
            "1767225600000",
            "--due-at",
            "1767312000000",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;

    let stdout = kanban(&temp.path, &["task", "show", task_id, "--details"])?.success_stdout()?;

    assert!(stdout.contains("ref: default#1"), "{stdout}");
    assert!(stdout.contains(&format!("id: {task_id}")), "{stdout}");
    assert!(stdout.contains("status: ready"), "{stdout}");
    assert!(stdout.contains("title: detailed task title"), "{stdout}");
    assert!(stdout.contains("assignee: executor"), "{stdout}");
    assert!(stdout.contains("priority: 42"), "{stdout}");
    assert!(stdout.contains("scheduled_at: 1767225600000"), "{stdout}");
    assert!(stdout.contains("due_at: 1767312000000"), "{stdout}");
    assert!(stdout.contains("created_at: "), "{stdout}");
    assert!(stdout.contains("updated_at: "), "{stdout}");
    assert!(
        stdout.contains("description:\n  first detail line\n  second detail line"),
        "{stdout}"
    );
    Ok(())
}

#[test]
fn task_show_details_does_not_change_json_output() -> anyhow::Result<()> {
    let temp = TempDb::new("task_show_details_does_not_change_json_output")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "json stable title",
            "--description",
            "json stable spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;

    let default_json = kanban(&temp.path, &["--json", "task", "show", task_id])?.success_json()?;
    let details_json = kanban(
        &temp.path,
        &["--json", "task", "show", task_id, "--details"],
    )?
    .success_json()?;

    assert_eq!(details_json, default_json);
    assert_eq!(details_json["data"]["title"], "json stable title");
    assert_eq!(details_json["data"]["description"], "json stable spec");
    Ok(())
}

#[test]
fn task_update_sets_and_clears_schedule_fields() -> anyhow::Result<()> {
    let temp = TempDb::new("task_update_sets_and_clears_scheduled_at_and_due_at")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "cli update dates",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;

    let updated = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "update",
            task_id,
            "--scheduled-at",
            "1767225600000",
            "--due-at",
            "1767312000000",
        ],
    )?
    .success_json()?;
    assert_eq!(updated["data"]["scheduled_at"], 1_767_225_600_000_i64);
    assert_eq!(updated["data"]["due_at"], 1_767_312_000_000_i64);

    let cleared = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "update",
            task_id,
            "--clear-scheduled-at",
            "--clear-due-at",
        ],
    )?
    .success_json()?;
    assert!(cleared["data"]["scheduled_at"].is_null());
    assert!(cleared["data"]["due_at"].is_null());
    Ok(())
}

#[test]
fn task_complete_alias_finishes_running_task() -> anyhow::Result<()> {
    let temp = TempDb::new("task_complete_alias_finishes_like_done")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "cli complete alias",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    let claim = kanban(&temp.path, &["--json", "task", "claim", task_id])?.success_json()?;
    let token = claim["data"]["claim_token"]
        .as_str()
        .context("expected JSON string")?;

    let completed = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "complete",
            task_id,
            "--claim-token",
            token,
        ],
    )?
    .success_json()?;
    assert_eq!(completed["data"]["status"], "done");
    Ok(())
}

#[test]
fn task_reclaim_expired_alias_matches_default_reclaim() -> anyhow::Result<()> {
    let bare = TempDb::new("task_reclaim_expired_alias_matches_bare_reclaim_bare")?;
    let explicit = TempDb::new("task_reclaim_expired_alias_matches_bare_reclaim_explicit")?;

    for temp in [&bare, &explicit] {
        kanban(&temp.path, &["init"])?.success()?;
        let created = kanban(
            &temp.path,
            &[
                "--json",
                "task",
                "create",
                "cli reclaim alias",
                "--description",
                "ready spec",
            ],
        )?
        .success_json()?;
        let task_id = created["data"]["id"]
            .as_str()
            .context("expected JSON string")?;
        kanban(
            &temp.path,
            &["--json", "task", "claim", task_id, "--ttl-ms", "1"],
        )?
        .success_json()?;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    let bare_result = kanban(&bare.path, &["--json", "task", "reclaim"])?.success_json()?;
    let explicit_result =
        kanban(&explicit.path, &["--json", "task", "reclaim", "--expired"])?.success_json()?;

    assert_eq!(bare_result, explicit_result);
    assert_eq!(explicit_result["data"]["reclaimed"], 1);
    Ok(())
}
