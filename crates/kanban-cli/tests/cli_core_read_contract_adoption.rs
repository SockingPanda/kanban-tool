mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_contract::{CliCommentListOutput, CliDependencyListOutput, CliEventsOutput};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

struct Setup {
    temp: TempDb,
    child_ref: String,
}

fn fixture(path: &str) -> anyhow::Result<Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Ok(serde_json::from_str(&std::fs::read_to_string(
        root.join(path),
    )?)?)
}

fn setup(name: &str) -> anyhow::Result<Setup> {
    let temp = TempDb::new(name)?;
    kanban(&temp.path, &["init"])?.success()?;
    let create = |title: &str| -> anyhow::Result<String> {
        let output = kanban(
            &temp.path,
            &[
                "--json",
                "--actor",
                "fixture",
                "task",
                "create",
                title,
                "--description",
                "ready spec",
            ],
        )?
        .success_json()?;
        Ok(output["data"]["ref"]
            .as_str()
            .context("task ref")?
            .to_owned())
    };
    let parent_ref = create("Parent")?;
    let child_ref = create("Child")?;
    kanban(
        &temp.path,
        &[
            "--actor",
            "fixture",
            "comment",
            "add",
            &child_ref,
            "Fixture comment",
            "--kind",
            "note",
        ],
    )?
    .success()?;
    kanban(
        &temp.path,
        &["--actor", "fixture", "dep", "add", &parent_ref, &child_ref],
    )?
    .success()?;
    Ok(Setup { temp, child_ref })
}

fn consume<T: DeserializeOwned>(path: &str) -> anyhow::Result<()> {
    serde_json::from_value::<T>(fixture(path)?).context("valid CLI output fixture")?;
    Ok(())
}

fn replace_dynamic(
    object: &mut serde_json::Map<String, Value>,
    key: &str,
    value: Value,
) -> anyhow::Result<()> {
    let current = object
        .get_mut(key)
        .with_context(|| format!("missing {key}"))?;
    anyhow::ensure!(
        (value.is_string() && current.as_str().is_some_and(|value| !value.is_empty()))
            || (value.is_i64() && current.is_i64()),
        "invalid dynamic field {key}"
    );
    *current = value;
    Ok(())
}

fn normalize_dependency_task(task: &mut Value, id: &str) -> anyhow::Result<()> {
    let task = task.as_object_mut().context("dependency task")?;
    replace_dynamic(task, "id", json!(id))?;
    replace_dynamic(task, "board_id", json!("b_fixture"))
}

#[test]
fn comment_list_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let setup = setup("comment_list_output_fixture_is_produced_by_real_cli")?;
    let mut output = kanban(
        &setup.temp.path,
        &["--json", "comment", "list", &setup.child_ref],
    )?
    .success_json()?;
    serde_json::from_value::<CliCommentListOutput>(output.clone())?;
    let comments = output["data"].as_array_mut().context("comments")?;
    anyhow::ensure!(comments.len() == 1, "expected one comment");
    let comment = comments[0].as_object_mut().context("comment")?;
    replace_dynamic(comment, "id", json!("c_fixture"))?;
    replace_dynamic(comment, "board_id", json!("b_fixture"))?;
    replace_dynamic(comment, "task_id", json!("t_child"))?;
    replace_dynamic(comment, "created_at", json!(103))?;
    assert_eq!(
        output,
        fixture("schemas/fixtures/cli/comment-list-output.v1.valid.json")?
    );
    Ok(())
}

#[test]
fn comment_list_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<CliCommentListOutput>("schemas/fixtures/cli/comment-list-output.v1.valid.json")
}

#[test]
fn dependency_list_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let setup = setup("dependency_list_output_fixture_is_produced_by_real_cli")?;
    let mut output = kanban(
        &setup.temp.path,
        &["--json", "dep", "list", &setup.child_ref],
    )?
    .success_json()?;
    serde_json::from_value::<CliDependencyListOutput>(output.clone())?;
    normalize_dependency_task(&mut output["data"]["task"], "t_child")?;
    normalize_dependency_task(&mut output["data"]["parents"][0], "t_parent")?;
    normalize_dependency_task(&mut output["data"]["edges"][0]["parent"], "t_parent")?;
    normalize_dependency_task(&mut output["data"]["edges"][0]["child"], "t_child")?;
    assert_eq!(
        output,
        fixture("schemas/fixtures/cli/dep-list-output.v1.valid.json")?
    );
    Ok(())
}

#[test]
fn dependency_list_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<CliDependencyListOutput>("schemas/fixtures/cli/dep-list-output.v1.valid.json")
}

#[test]
fn events_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let setup = setup("events_output_fixture_is_produced_by_real_cli")?;
    let mut output =
        kanban(&setup.temp.path, &["--json", "events", &setup.child_ref])?.success_json()?;
    serde_json::from_value::<CliEventsOutput>(output.clone())?;
    let events = output["data"].as_array_mut().context("events")?;
    anyhow::ensure!(events.len() == 3, "expected three task events");
    for (index, event) in events.iter_mut().enumerate() {
        let event = event.as_object_mut().context("event")?;
        replace_dynamic(event, "id", json!(301 + index as i64))?;
        replace_dynamic(event, "event_id", json!(format!("e_fixture_{}", index + 1)))?;
        replace_dynamic(event, "task_id", json!("t_child"))?;
        replace_dynamic(event, "created_at", json!(201 + index as i64))?;
        let mut payload = event.get("payload").cloned().context("event payload")?;
        match event.get("kind").and_then(Value::as_str) {
            Some("task.comment.created") => {
                let payload_object = payload.as_object().context("comment event payload")?;
                anyhow::ensure!(payload_object.len() == 4, "comment payload keys");
                anyhow::ensure!(
                    payload_object.get("agent_type") == Some(&Value::Null),
                    "comment agent_type"
                );
                anyhow::ensure!(payload["author_type"] == "user", "comment author_type");
                anyhow::ensure!(payload["kind"] == "note", "comment kind");
                anyhow::ensure!(
                    payload["comment_id"]
                        .as_str()
                        .is_some_and(|value| !value.is_empty()),
                    "comment id"
                );
                payload["comment_id"] = json!("c_fixture");
            }
            Some("dependency.added") => {
                let payload_object = payload.as_object().context("dependency event payload")?;
                anyhow::ensure!(payload_object.len() == 1, "dependency payload keys");
                anyhow::ensure!(
                    payload["parent_task_id"]
                        .as_str()
                        .is_some_and(|value| !value.is_empty()),
                    "parent task id"
                );
                payload["parent_task_id"] = json!("t_parent");
            }
            Some("task.created") => {
                anyhow::ensure!(payload == json!({"status": "todo"}), "task created payload");
            }
            kind => anyhow::bail!("unexpected event kind {kind:?}"),
        }
        event.insert("payload".to_owned(), payload);
    }
    assert_eq!(
        output,
        fixture("schemas/fixtures/cli/events-output.v1.valid.json")?
    );
    Ok(())
}

#[test]
fn events_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<CliEventsOutput>("schemas/fixtures/cli/events-output.v1.valid.json")
}
