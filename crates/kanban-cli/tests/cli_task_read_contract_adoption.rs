mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_contract::{CliTaskListOutput, CliTaskShowOutput};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn fixture(path: &str) -> anyhow::Result<Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Ok(serde_json::from_str(&std::fs::read_to_string(
        root.join(path),
    )?)?)
}

fn setup(name: &str) -> anyhow::Result<(TempDb, String)> {
    let temp = TempDb::new(name)?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(
        &temp.path,
        &[
            "--actor", "fixture", "label", "create", "core", "--color", "#123456",
        ],
    )?
    .success()?;
    let output = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture",
            "task",
            "create",
            "Fixture task",
            "--description",
            "ready spec",
            "--priority",
            "1",
            "--max-retries",
            "2",
            "--label",
            "core",
        ],
    )?
    .success_json()?;
    let task_ref = output["data"]["ref"]
        .as_str()
        .context("created task ref")?
        .to_owned();
    Ok((temp, task_ref))
}

fn normalize_task(task: &mut Value) -> anyhow::Result<()> {
    let object = task.as_object_mut().context("task object")?;
    anyhow::ensure!(
        !object.contains_key("claim_token"),
        "task leaked claim_token"
    );
    for (key, replacement) in [
        ("id", json!("t_fixture")),
        ("board_id", json!("b_fixture")),
        ("created_at", json!(101)),
        ("updated_at", json!(102)),
    ] {
        let value = object.get_mut(key).with_context(|| format!("task.{key}"))?;
        match key {
            "id" | "board_id" => anyhow::ensure!(
                value.as_str().is_some_and(|value| !value.is_empty()),
                "task.{key} must be a non-empty string"
            ),
            _ => anyhow::ensure!(value.is_i64(), "task.{key} must be an integer"),
        }
        *value = replacement;
    }
    let labels = object
        .get_mut("labels")
        .and_then(Value::as_array_mut)
        .context("task.labels")?;
    anyhow::ensure!(labels.len() == 1, "expected one task label");
    let label = labels[0].as_object_mut().context("task.labels[0]")?;
    anyhow::ensure!(label.get("name") == Some(&json!("core")), "label name");
    anyhow::ensure!(label.get("color") == Some(&json!("#123456")), "label color");
    for (key, replacement) in [
        ("id", json!("l_fixture")),
        ("board_id", json!("b_fixture")),
        ("created_at", json!(91)),
        ("updated_at", json!(92)),
    ] {
        let value = label
            .get_mut(key)
            .with_context(|| format!("task.labels[0].{key}"))?;
        match key {
            "id" | "board_id" => anyhow::ensure!(
                value.as_str().is_some_and(|value| !value.is_empty()),
                "task.labels[0].{key} must be a non-empty string"
            ),
            _ => anyhow::ensure!(value.is_i64(), "task.labels[0].{key} must be an integer"),
        }
        *value = replacement;
    }
    Ok(())
}

fn consume<T: DeserializeOwned>(path: &str) -> anyhow::Result<()> {
    serde_json::from_value::<T>(fixture(path)?).context("valid CLI output fixture")?;
    Ok(())
}

#[test]
fn task_list_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let (temp, _) = setup("task_list_output_fixture_is_produced_by_real_cli")?;
    let mut output =
        kanban(&temp.path, &["--json", "task", "list", "--limit", "1"])?.success_json()?;
    serde_json::from_value::<CliTaskListOutput>(output.clone())
        .context("real task list output must satisfy its contract root")?;
    let tasks = output["data"].as_array_mut().context("task list")?;
    anyhow::ensure!(tasks.len() == 1, "expected one task");
    normalize_task(&mut tasks[0])?;
    assert_eq!(
        output,
        fixture("schemas/fixtures/cli/task-list-output.v1.valid.json")?
    );
    Ok(())
}

#[test]
fn task_list_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<CliTaskListOutput>("schemas/fixtures/cli/task-list-output.v1.valid.json")
}

#[test]
fn task_show_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let (temp, task_ref) = setup("task_show_output_fixture_is_produced_by_real_cli")?;
    let mut output = kanban(
        &temp.path,
        &["--json", "task", "show", &task_ref, "--details"],
    )?
    .success_json()?;
    serde_json::from_value::<CliTaskShowOutput>(output.clone())
        .context("real task show output must satisfy its contract root")?;
    normalize_task(&mut output["data"])?;
    anyhow::ensure!(
        output["meta"]["details"]["ontology_summary"].is_null(),
        "details ontology summary must be explicit null"
    );
    assert_eq!(
        output,
        fixture("schemas/fixtures/cli/task-show-output.v1.valid.json")?
    );
    Ok(())
}

#[test]
fn task_show_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<CliTaskShowOutput>("schemas/fixtures/cli/task-show-output.v1.valid.json")
}
