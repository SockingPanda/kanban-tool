mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use pretty_assertions::assert_eq;

fn create_task(temp: &TempDb, title: &str) -> anyhow::Result<String> {
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "--board",
            "default",
            "task",
            "create",
            title,
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    created["data"]["id"]
        .as_str()
        .map(str::to_owned)
        .context("expected task id")
}

#[test]
fn comment_add_and_list_json_roundtrip() -> anyhow::Result<()> {
    let temp = TempDb::new("comment_add_and_list_json_roundtrip")?;
    kanban(&temp.path, &["--board", "default", "init"])?.success()?;
    let task_id = create_task(&temp, "comment json")?;

    let added = kanban(
        &temp.path,
        &[
            "--json",
            "--board",
            "default",
            "--actor",
            "alice",
            "comment",
            "add",
            &task_id,
            "hello from cli",
        ],
    )?
    .success_json()?;
    assert!(added["data"]["id"].as_str().unwrap_or("").starts_with("c_"));
    assert_eq!(added["data"]["task_id"], task_id);
    assert_eq!(added["data"]["body"], "hello from cli");
    assert_eq!(added["data"]["kind"], "text");
    assert_eq!(added["data"]["author"], "alice");
    assert_eq!(added["data"]["author_type"], "human");
    assert!(added["data"]["agent_type"].is_null());

    let listed = kanban(
        &temp.path,
        &["--json", "--board", "default", "comment", "list", &task_id],
    )?
    .success_json()?;
    assert_eq!(
        listed["data"].as_array().context("expected array")?.len(),
        1
    );
    assert_eq!(listed["data"][0], added["data"]);
    Ok(())
}

#[test]
fn comment_human_output_is_readable() -> anyhow::Result<()> {
    let temp = TempDb::new("comment_human_output_is_readable")?;
    kanban(&temp.path, &["--board", "default", "init"])?.success()?;
    let task_id = create_task(&temp, "comment human")?;

    let added = kanban(
        &temp.path,
        &[
            "--board",
            "default",
            "--actor",
            "bob",
            "comment",
            "add",
            &task_id,
            "human output body",
        ],
    )?
    .success_stdout()?;
    assert!(added.contains("c_"));
    assert!(added.contains(&task_id));
    assert!(added.contains("[text]"));
    assert!(added.contains("bob (human)"));
    assert!(added.contains("human output body"));

    let listed = kanban(
        &temp.path,
        &["--board", "default", "comment", "list", &task_id],
    )?
    .success_stdout()?;
    assert!(listed.contains("c_"));
    assert!(listed.contains("[text]"));
    assert!(listed.contains("bob (human)"));
    assert!(listed.contains("human output body"));
    Ok(())
}

#[test]
fn comment_kind_worker_infers_agent_author_type() -> anyhow::Result<()> {
    let temp = TempDb::new("comment_kind_worker_infers_agent_author_type")?;
    kanban(&temp.path, &["--board", "default", "init"])?.success()?;
    let task_id = create_task(&temp, "worker comment")?;

    let added = kanban(
        &temp.path,
        &[
            "--json",
            "--board",
            "default",
            "--actor",
            "worker-a",
            "comment",
            "add",
            &task_id,
            "worker output",
            "--kind",
            "worker",
        ],
    )?
    .success_json()?;
    assert_eq!(added["data"]["kind"], "worker");
    assert_eq!(added["data"]["author_type"], "agent");
    Ok(())
}

#[test]
fn comment_agent_type_requires_agent_author_type() -> anyhow::Result<()> {
    let temp = TempDb::new("comment_agent_type_requires_agent_author_type")?;
    kanban(&temp.path, &["--board", "default", "init"])?.success()?;
    let task_id = create_task(&temp, "agent comment")?;

    let added = kanban(
        &temp.path,
        &[
            "--json",
            "--board",
            "default",
            "--actor",
            "executor",
            "comment",
            "add",
            &task_id,
            "agent note",
            "--author-type",
            "agent",
            "--agent-type",
            "root",
        ],
    )?
    .success_json()?;
    assert_eq!(added["data"]["author"], "executor");
    assert_eq!(added["data"]["author_type"], "agent");
    assert_eq!(added["data"]["agent_type"], "root");

    kanban(
        &temp.path,
        &[
            "--board",
            "default",
            "comment",
            "add",
            &task_id,
            "bad agent type",
            "--agent-type",
            "root",
        ],
    )?
    .failure_containing("comment agent_type is only allowed when author_type is agent")?;
    Ok(())
}

#[test]
fn comment_add_rejects_invalid_input() -> anyhow::Result<()> {
    let temp = TempDb::new("comment_add_rejects_invalid_input")?;
    kanban(&temp.path, &["--board", "default", "init"])?.success()?;
    let task_id = create_task(&temp, "invalid comments")?;

    kanban(
        &temp.path,
        &["--board", "default", "comment", "add", &task_id, "   "],
    )?
    .failure_containing("comment body is required")?;
    kanban(
        &temp.path,
        &[
            "--board", "default", "comment", "add", &task_id, "bad kind", "--kind", "invalid",
        ],
    )?
    .failure_containing("invalid comment kind")?;
    kanban(
        &temp.path,
        &[
            "--board",
            "default",
            "comment",
            "add",
            &task_id,
            "bad author type",
            "--author-type",
            "robot",
        ],
    )?
    .failure_containing("invalid comment author_type")?;
    Ok(())
}

#[test]
fn comment_add_writes_task_comment_created_event() -> anyhow::Result<()> {
    let temp = TempDb::new("comment_add_writes_task_comment_created_event")?;
    kanban(&temp.path, &["--board", "default", "init"])?.success()?;
    let task_id = create_task(&temp, "comment event")?;

    kanban(
        &temp.path,
        &[
            "--board",
            "default",
            "comment",
            "add",
            &task_id,
            "event note",
        ],
    )?
    .success()?;

    let events = kanban(
        &temp.path,
        &["--json", "--board", "default", "events", &task_id],
    )?
    .success_json()?;
    let kinds = events["data"]
        .as_array()
        .context("expected event array")?
        .iter()
        .filter_map(|event| event["kind"].as_str())
        .collect::<Vec<_>>();
    assert!(kinds.contains(&"task.comment.created"));
    Ok(())
}
