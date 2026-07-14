mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_contract::{CliCommentAddOutput, CliDependencyAddOutput, CliDependencyRemoveOutput};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn fixture(operation: &str) -> anyhow::Result<Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join(format!(
        "schemas/fixtures/cli/{operation}-output.v1.valid.json"
    ));
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

fn consume<T: DeserializeOwned>(operation: &str) -> anyhow::Result<()> {
    serde_json::from_value::<T>(fixture(operation)?)?;
    Ok(())
}

fn create_task(temp: &TempDb, title: &str) -> anyhow::Result<String> {
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
            "contract specification",
        ],
    )?
    .success_json()?;
    Ok(output["data"]["ref"]
        .as_str()
        .context("task ref")?
        .to_owned())
}

fn setup(name: &str) -> anyhow::Result<(TempDb, String, String)> {
    let temp = TempDb::new(name)?;
    kanban(&temp.path, &["init"])?.success()?;
    let parent = create_task(&temp, "Parent")?;
    let child = create_task(&temp, "Child")?;
    Ok((temp, parent, child))
}

fn task_identity(temp: &TempDb, task_ref: &str) -> anyhow::Result<(String, String)> {
    let output = kanban(
        &temp.path,
        &["--json", "--actor", "fixture", "task", "show", task_ref],
    )?
    .success_json()?;
    Ok((
        output["data"]["id"].as_str().context("task id")?.to_owned(),
        output["data"]["board_id"]
            .as_str()
            .context("task board id")?
            .to_owned(),
    ))
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
        (value.is_string() && current.as_str().is_some_and(|v| !v.is_empty()))
            || (value.is_i64() && current.is_i64()),
        "invalid dynamic field {key}"
    );
    *current = value;
    Ok(())
}

fn normalize_dependency_task(task: &mut Value) -> anyhow::Result<()> {
    let task = task.as_object_mut().context("dependency task")?;
    let task_ref = task["ref"].as_str().context("dependency task ref")?;
    let (id, title) = match task_ref {
        "default#1" => ("t_parent", "Parent"),
        "default#2" => ("t_child", "Child"),
        other => anyhow::bail!("unexpected task ref {other}"),
    };
    anyhow::ensure!(task["title"] == title);
    anyhow::ensure!(!task.contains_key("claim_token"), "claim_token leaked");
    replace_dynamic(task, "id", json!(id))?;
    replace_dynamic(task, "board_id", json!("b_fixture"))?;
    Ok(())
}

fn normalize_dependency_tree(value: &mut Value) -> anyhow::Result<()> {
    match value {
        Value::Object(object) if object.contains_key("ref") => normalize_dependency_task(value),
        Value::Object(object) => {
            for child in object.values_mut() {
                normalize_dependency_tree(child)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for child in values {
                normalize_dependency_tree(child)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn verify_dependency<T: DeserializeOwned>(
    operation: &str,
    mut output: Value,
    removed: bool,
    parent_identity: (&str, &str),
    child_identity: (&str, &str),
) -> anyhow::Result<()> {
    serde_json::from_value::<T>(output.clone())?;
    anyhow::ensure!(output["data"]["edge"]["parent"]["ref"] == "default#1");
    anyhow::ensure!(output["data"]["edge"]["child"]["ref"] == "default#2");
    anyhow::ensure!(output["data"]["dependencies"]["task"]["ref"] == "default#2");
    anyhow::ensure!(output["data"]["edge"]["parent"]["id"] == parent_identity.0);
    anyhow::ensure!(output["data"]["edge"]["parent"]["board_id"] == parent_identity.1);
    anyhow::ensure!(output["data"]["edge"]["child"]["id"] == child_identity.0);
    anyhow::ensure!(output["data"]["edge"]["child"]["board_id"] == child_identity.1);
    anyhow::ensure!(output["data"]["dependencies"]["task"]["id"] == child_identity.0);
    anyhow::ensure!(output["data"]["dependencies"]["task"]["board_id"] == child_identity.1);
    if removed {
        anyhow::ensure!(
            output["data"]["dependencies"]["parents"]
                .as_array()
                .is_some_and(Vec::is_empty)
        );
        anyhow::ensure!(
            output["data"]["dependencies"]["edges"]
                .as_array()
                .is_some_and(Vec::is_empty)
        );
    } else {
        anyhow::ensure!(output["data"]["dependencies"]["parents"][0]["ref"] == "default#1");
        anyhow::ensure!(output["data"]["dependencies"]["parents"][0]["id"] == parent_identity.0);
        anyhow::ensure!(
            output["data"]["dependencies"]["parents"][0]["board_id"] == parent_identity.1
        );
        anyhow::ensure!(output["data"]["dependencies"]["edges"][0] == output["data"]["edge"]);
    }
    normalize_dependency_tree(&mut output)?;
    assert_eq!(output, fixture(operation)?);
    Ok(())
}

#[test]
fn comment_add_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let (temp, _, child) = setup("comment_add_output_fixture_is_produced_by_real_cli")?;
    let child_identity = task_identity(&temp, &child)?;
    let mut output = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture-agent",
            "comment",
            "add",
            &child,
            "line one\nline two",
            "--kind",
            "note",
            "--author-type",
            "agent",
            "--agent-type",
            "executor",
            "--metadata-json",
            r#"{"source":"fixture"}"#,
        ],
    )?
    .success_json()?;
    serde_json::from_value::<CliCommentAddOutput>(output.clone())?;
    anyhow::ensure!(output["data"]["author"] == "fixture-agent");
    anyhow::ensure!(output["data"]["body"] == "line one\nline two");
    anyhow::ensure!(output["data"]["task_id"] == child_identity.0);
    anyhow::ensure!(output["data"]["board_id"] == child_identity.1);
    let comment = output["data"].as_object_mut().context("comment")?;
    replace_dynamic(comment, "id", json!("c_fixture"))?;
    replace_dynamic(comment, "board_id", json!("b_fixture"))?;
    replace_dynamic(comment, "task_id", json!("t_child"))?;
    replace_dynamic(comment, "created_at", json!(101))?;
    assert_eq!(output, fixture("comment-add")?);
    Ok(())
}

#[test]
fn dependency_add_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let (temp, parent, child) = setup("dependency_add_output_fixture_is_produced_by_real_cli")?;
    let parent_identity = task_identity(&temp, &parent)?;
    let child_identity = task_identity(&temp, &child)?;
    let output = kanban(
        &temp.path,
        &[
            "--json", "--actor", "fixture", "dep", "add", &parent, &child,
        ],
    )?
    .success_json()?;
    verify_dependency::<CliDependencyAddOutput>(
        "dep-add",
        output,
        false,
        (&parent_identity.0, &parent_identity.1),
        (&child_identity.0, &child_identity.1),
    )
}

#[test]
fn dependency_remove_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let (temp, parent, child) = setup("dependency_remove_output_fixture_is_produced_by_real_cli")?;
    let parent_identity = task_identity(&temp, &parent)?;
    let child_identity = task_identity(&temp, &child)?;
    kanban(
        &temp.path,
        &["--actor", "fixture", "dep", "add", &parent, &child],
    )?
    .success()?;
    let output = kanban(
        &temp.path,
        &[
            "--json", "--actor", "fixture", "dep", "remove", &parent, &child,
        ],
    )?
    .success_json()?;
    verify_dependency::<CliDependencyRemoveOutput>(
        "dep-remove",
        output,
        true,
        (&parent_identity.0, &parent_identity.1),
        (&child_identity.0, &child_identity.1),
    )
}

#[test]
fn comment_add_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliCommentAddOutput>("comment-add")
}

#[test]
fn dependency_add_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliDependencyAddOutput>("dep-add")
}

#[test]
fn dependency_remove_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliDependencyRemoveOutput>("dep-remove")
}
