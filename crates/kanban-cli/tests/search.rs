mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use pretty_assertions::assert_eq;
#[test]
fn search_command_outputs_json_and_human_hits() -> anyhow::Result<()> {
    let temp = TempDb::new("search_command_outputs_json_and_human_hits")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(
        &temp.path,
        &[
            "task",
            "create",
            "Alpha search surface",
            "--description",
            "ready spec unique-needle",
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
            "Beta search surface",
            "--description",
            "ready spec unique-needle",
            "--assignee",
            "worker-b",
        ],
    )?
    .success()?;

    let json = kanban(
        &temp.path,
        &[
            "--json",
            "search",
            "unique-needle",
            "--assignee",
            "worker-a",
            "--limit",
            "5",
        ],
    )?
    .success_json()?;
    assert_eq!(json["data"]["meta"]["backend"], "sqlite");
    let hits = json["data"]["hits"]
        .as_array()
        .context("expected JSON array")?;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["task"]["title"], "Alpha search surface");
    assert!(hits[0]["score"].as_f64().context("expected JSON f64")? > 0.0);
    assert!(
        hits[0]["snippet"]
            .as_str()
            .context("expected JSON string")?
            .contains("unique-needle")
    );

    let human = kanban(
        &temp.path,
        &["search", "unique-needle", "--assignee", "worker-a"],
    )?;
    assert!(human.output.status.success());
    let stdout = String::from_utf8_lossy(&human.output.stdout);
    assert!(stdout.contains("#1"), "{stdout}");
    assert!(stdout.contains("[ready]"), "{stdout}");
    assert!(stdout.contains("score="), "{stdout}");
    assert!(stdout.contains("Alpha search surface"), "{stdout}");
    assert!(stdout.contains("unique-needle"), "{stdout}");
    Ok(())
}

#[test]
fn search_command_rejects_unbounded_limit() -> anyhow::Result<()> {
    let temp = TempDb::new("search_command_rejects_unbounded_limit")?;
    kanban(&temp.path, &["init"])?.success()?;

    kanban(
        &temp.path,
        &["search", "needle", "--limit", &usize::MAX.to_string()],
    )?
    .failure_containing("limit must be <= 1000")?;
    Ok(())
}

#[test]
fn search_command_treats_like_wildcards_and_escape_characters_as_literal_text() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("search_command_treats_like_wildcards_and_escape_characters_as_literal_text")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(
        &temp.path,
        &[
            "task",
            "create",
            "literal percent % cli",
            "--description",
            "ready spec",
        ],
    )?
    .success()?;
    kanban(
        &temp.path,
        &[
            "task",
            "create",
            "literal backslash \\ cli",
            "--description",
            "ready spec",
        ],
    )?
    .success()?;
    kanban(
        &temp.path,
        &[
            "task",
            "create",
            "plain cli control",
            "--description",
            "ready spec",
        ],
    )?
    .success()?;

    let json = kanban(&temp.path, &["--json", "search", "%"])?.success_json()?;
    let hits = json["data"]["hits"]
        .as_array()
        .context("expected JSON array")?;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["task"]["title"], "literal percent % cli");

    let json = kanban(&temp.path, &["--json", "search", "\\"])?.success_json()?;
    let hits = json["data"]["hits"]
        .as_array()
        .context("expected JSON array")?;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["task"]["title"], "literal backslash \\ cli");
    Ok(())
}
