mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_contract::CliTaskStepListOutput;
use serde_json::{Value, json};

fn fixture() -> anyhow::Result<Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Ok(serde_json::from_str(&std::fs::read_to_string(root.join(
        "schemas/fixtures/cli/task-step-list-output.v1.valid.json",
    ))?)?)
}

fn setup(name: &str) -> anyhow::Result<(TempDb, String)> {
    let temp = TempDb::new(name)?;
    kanban(&temp.path, &["init"])?.success()?;
    let create = |title: &str| -> anyhow::Result<String> {
        let output = kanban(
            &temp.path,
            &["--json", "--actor", "fixture", "task", "create", title],
        )?
        .success_json()?;
        Ok(output["data"]["ref"]
            .as_str()
            .context("task ref")?
            .to_owned())
    };
    let parent_ref = create("Plan owner")?;
    let linked_ref = create("Linked implementation")?;
    kanban(
        &temp.path,
        &[
            "--actor",
            "fixture",
            "task",
            "step",
            "add",
            &parent_ref,
            "Implement linked work",
            "--body",
            "Keep the linked task visible",
            "--link-task",
            &linked_ref,
        ],
    )?
    .success()?;
    Ok((temp, parent_ref))
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

#[test]
fn task_step_list_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let (temp, task_ref) = setup("task_step_list_output_fixture_is_produced_by_real_cli")?;
    let mut output =
        kanban(&temp.path, &["--json", "task", "step", "list", &task_ref])?.success_json()?;
    serde_json::from_value::<CliTaskStepListOutput>(output.clone())?;
    let data = output["data"].as_object_mut().context("data")?;
    replace_dynamic(data, "task_id", json!("t_parent"))?;
    let plan = data["execution_plan"]
        .as_object_mut()
        .context("execution plan")?;
    replace_dynamic(plan, "board_id", json!("b_fixture"))?;
    replace_dynamic(plan, "task_id", json!("t_parent"))?;
    replace_dynamic(plan, "updated_at", json!(101))?;
    let steps = data["steps"].as_array_mut().context("steps")?;
    anyhow::ensure!(steps.len() == 1, "expected one step");
    let step = steps[0].as_object_mut().context("step")?;
    replace_dynamic(step, "id", json!("s_fixture"))?;
    replace_dynamic(step, "parent_task_id", json!("t_parent"))?;
    replace_dynamic(step, "created_at", json!(102))?;
    replace_dynamic(step, "updated_at", json!(103))?;
    let linked = step["linked_task"].as_object_mut().context("linked task")?;
    anyhow::ensure!(!linked.contains_key("claim_token"), "claim_token leaked");
    replace_dynamic(linked, "id", json!("t_linked"))?;
    replace_dynamic(linked, "board_id", json!("b_fixture"))?;
    replace_dynamic(linked, "created_at", json!(104))?;
    replace_dynamic(linked, "updated_at", json!(105))?;
    assert_eq!(output, fixture()?);
    Ok(())
}

#[test]
fn task_step_list_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    serde_json::from_value::<CliTaskStepListOutput>(fixture()?)?;
    Ok(())
}
