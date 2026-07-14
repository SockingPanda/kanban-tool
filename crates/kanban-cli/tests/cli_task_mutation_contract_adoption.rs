mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_contract::{CliTaskCreateOutput, CliTaskUpdateOutput};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn fixture(path: &str) -> anyhow::Result<Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Ok(serde_json::from_str(&std::fs::read_to_string(
        root.join(path),
    )?)?)
}

fn consume<T: DeserializeOwned>(path: &str) -> anyhow::Result<()> {
    serde_json::from_value::<T>(fixture(path)?).context("valid CLI task mutation fixture")?;
    Ok(())
}

fn normalize_task(task: &mut Value) -> anyhow::Result<()> {
    let task = task.as_object_mut().context("task")?;
    anyhow::ensure!(!task.contains_key("claim_token"), "claim_token leaked");
    for (key, replacement) in [
        ("id", json!("t_fixture")),
        ("board_id", json!("b_fixture")),
        ("created_at", json!(101)),
        ("updated_at", json!(102)),
    ] {
        let current = task
            .get_mut(key)
            .with_context(|| format!("missing task.{key}"))?;
        anyhow::ensure!(
            (replacement.is_string() && current.as_str().is_some_and(|value| !value.is_empty()))
                || (replacement.is_i64() && current.is_i64()),
            "invalid task.{key}"
        );
        *current = replacement;
    }
    Ok(())
}

fn create(temp: &TempDb) -> anyhow::Result<Value> {
    kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture",
            "task",
            "create",
            "Created contract task",
            "--description",
            "initial description",
            "--priority",
            "2",
            "--max-retries",
            "3",
        ],
    )?
    .success_json()
}

#[test]
fn task_create_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = TempDb::new("task_create_output_fixture_is_produced_by_real_cli")?;
    kanban(&temp.path, &["init"])?.success()?;
    let mut output = create(&temp)?;
    serde_json::from_value::<CliTaskCreateOutput>(output.clone())?;
    normalize_task(&mut output["data"])?;
    assert_eq!(
        output,
        fixture("schemas/fixtures/cli/task-create-output.v1.valid.json")?
    );
    Ok(())
}

#[test]
fn task_create_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<CliTaskCreateOutput>("schemas/fixtures/cli/task-create-output.v1.valid.json")
}

#[test]
fn task_update_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = TempDb::new("task_update_output_fixture_is_produced_by_real_cli")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = create(&temp)?;
    let task_ref = created["data"]["ref"].as_str().context("task ref")?;
    let mut output = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture",
            "task",
            "update",
            task_ref,
            "--title",
            "Updated contract task",
            "--description",
            "updated description",
            "--priority",
            "1",
            "--max-retries",
            "4",
        ],
    )?
    .success_json()?;
    serde_json::from_value::<CliTaskUpdateOutput>(output.clone())?;
    normalize_task(&mut output["data"])?;
    assert_eq!(
        output,
        fixture("schemas/fixtures/cli/task-update-output.v1.valid.json")?
    );
    Ok(())
}

#[test]
fn task_update_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<CliTaskUpdateOutput>("schemas/fixtures/cli/task-update-output.v1.valid.json")
}
