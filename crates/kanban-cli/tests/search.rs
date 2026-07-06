mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_core::TaskStatus;
use kanban_sqlite::{CreateTask, connect_file, create_task, create_task_with_labels};
use pretty_assertions::assert_eq;

fn mark_no_plan_required(db_path: &std::path::Path, task_id: &str) -> anyhow::Result<()> {
    kanban_sqlite::mark_execution_plan_not_required(
        db_path,
        "default",
        "cli-search-test",
        task_id,
        "search fixture does not need steps",
    )?;
    Ok(())
}

#[test]
fn search_command_outputs_json_and_human_hits() -> anyhow::Result<()> {
    let temp = TempDb::new("search_command_outputs_json_and_human_hits")?;
    kanban(&temp.path, &["init"])?.success()?;
    let alpha = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "Alpha search surface",
            "--description",
            "ready spec unique-needle",
            "--assignee",
            "worker-a",
        ],
    )?
    .success_json()?;
    mark_no_plan_required(
        &temp.path,
        alpha["data"]["id"]
            .as_str()
            .context("expected JSON string")?,
    )?;
    let beta = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "Beta search surface",
            "--description",
            "ready spec unique-needle",
            "--assignee",
            "worker-b",
        ],
    )?
    .success_json()?;
    mark_no_plan_required(
        &temp.path,
        beta["data"]["id"]
            .as_str()
            .context("expected JSON string")?,
    )?;

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
    assert_eq!(json["meta"]["backend"], "sqlite");
    assert!(json["data"].get("meta").is_none(), "{json}");
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
fn search_command_filters_by_label() -> anyhow::Result<()> {
    let temp = TempDb::new("search_command_filters_by_label")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(&temp.path, &["label", "create", "backend"])?.success()?;
    kanban(&temp.path, &["label", "create", "frontend"])?.success()?;
    kanban(
        &temp.path,
        &[
            "task",
            "create",
            "Backend search label",
            "--description",
            "ready spec shared-label-needle",
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
            "Frontend search label",
            "--description",
            "ready spec shared-label-needle",
            "--label",
            "frontend",
        ],
    )?
    .success()?;

    let json = kanban(
        &temp.path,
        &[
            "--json",
            "search",
            "shared-label-needle",
            "--label",
            "backend",
        ],
    )?
    .success_json()?;
    let hits = json["data"]["hits"]
        .as_array()
        .context("expected JSON array")?;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["task"]["title"], "Backend search label");
    assert_eq!(hits[0]["task"]["labels"][0]["name"], "backend");
    Ok(())
}

#[test]
fn search_command_filters_labels_before_search_pagination() -> anyhow::Result<()> {
    let temp = TempDb::new("search_command_filters_labels_before_search_pagination")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban_sqlite::create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".into(),
            color: None,
        },
    )?;
    let labeled = create_task_with_labels(
        &temp.path,
        "default",
        "seed",
        CreateTask {
            title: "deep labeled cli search match".into(),
            description: Some("ready spec deep-label-needle".into()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 3,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
        &["backend".into()],
    )?;
    for index in 0..3 {
        create_task(
            &temp.path,
            "default",
            "seed",
            CreateTask {
                title: format!("unlabeled cli search match {index}"),
                description: Some("ready spec deep-label-needle".into()),
                status: Some(TaskStatus::Ready),
                assignee: None,
                priority: 3,
                scheduled_at: None,
                due_at: None,
                max_retries: None,
                metadata_json: "{}".into(),
            },
        )?;
    }
    connect_file(&temp.path)?.execute(
        "UPDATE tasks SET updated_at=seq WHERE board_id=(SELECT id FROM boards WHERE slug='default')",
        [],
    )?;

    let json = kanban(
        &temp.path,
        &[
            "--json",
            "search",
            "deep-label-needle",
            "--label",
            "backend",
            "--limit",
            "1",
        ],
    )?
    .success_json()?;
    let hits = json["data"]["hits"]
        .as_array()
        .context("expected JSON array")?;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["task"]["id"], labeled.id);
    assert_eq!(hits[0]["task"]["labels"][0]["name"], "backend");
    Ok(())
}

#[test]
fn search_command_rejects_unbounded_limit() -> anyhow::Result<()> {
    let temp = TempDb::new("search_command_rejects_unbounded_limit")?;
    kanban(&temp.path, &["init"])?.success()?;

    kanban(
        &temp.path,
        &[
            "--locale",
            "en",
            "search",
            "needle",
            "--limit",
            &usize::MAX.to_string(),
        ],
    )?
    .failure_containing("limit must be <= 1000")?;
    Ok(())
}

#[test]
fn search_command_matches_task_refs_exactly() -> anyhow::Result<()> {
    let temp = TempDb::new("search_command_matches_task_refs_exactly")?;
    kanban(&temp.path, &["init"])?.success()?;
    let first = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "first cli task",
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
            "title mentions 1 but numeric search is exact",
            "--description",
            "ready spec",
        ],
    )?
    .success()?;
    let first_id = first["data"]["id"].as_str().context("task id")?;

    for query in ["1", "#1", "default#1", first_id] {
        let json = kanban(&temp.path, &["--json", "search", query])?.success_json()?;
        let hits = json["data"]["hits"]
            .as_array()
            .context("expected JSON array")?;
        assert_eq!(hits.len(), 1, "{query}: {json}");
        assert_eq!(hits[0]["task"]["id"], first_id, "{query}: {json}");
    }

    let json = kanban(&temp.path, &["--json", "search", "other#1"])?.success_json()?;
    assert!(
        json["data"]["hits"]
            .as_array()
            .context("expected JSON array")?
            .is_empty()
    );
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
