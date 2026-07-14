mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_contract::{
    CliTaskStepAddOutput, CliTaskStepDoneOutput, CliTaskStepNotRequiredOutput,
    CliTaskStepRemoveOutput, CliTaskStepReopenOutput, CliTaskStepSkipOutput,
    CliTaskStepUpdateOutput,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn fixture(operation: &str) -> anyhow::Result<Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join(format!(
        "schemas/fixtures/cli/task-step-{operation}-output.v1.valid.json"
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
    let parent = create_task(&temp, "Plan owner")?;
    let linked = create_task(&temp, "Linked implementation")?;
    Ok((temp, parent, linked))
}

fn claim_task(temp: &TempDb, task_ref: &str) -> anyhow::Result<()> {
    kanban(
        &temp.path,
        &[
            "--actor",
            "fixture",
            "task",
            "step",
            "not-required",
            task_ref,
            "--reason",
            "linked execution",
        ],
    )?
    .success()?;
    kanban(
        &temp.path,
        &[
            "--actor",
            "fixture-claimer",
            "task",
            "claim",
            task_ref,
            "--ttl-ms",
            "60000",
        ],
    )?
    .success()?;
    Ok(())
}

fn add_step(
    temp: &TempDb,
    parent: &str,
    linked: Option<&str>,
    json_output: bool,
) -> anyhow::Result<Value> {
    let mut args = vec![
        "--actor",
        "fixture",
        "task",
        "step",
        "add",
        parent,
        "Implement contract",
        "--body",
        "Preserve the public step wire",
        "--position",
        "2048",
        "--optional",
    ];
    if json_output {
        args.insert(0, "--json");
    }
    if let Some(linked) = linked {
        args.extend(["--link-task", linked]);
    }
    let result = kanban(&temp.path, &args)?;
    if json_output {
        result.success_json()
    } else {
        result.success()?;
        Ok(Value::Null)
    }
}

fn step_id(output: &Value) -> anyhow::Result<String> {
    Ok(output["data"]["id"].as_str().context("step id")?.to_owned())
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

fn normalize_step(step: &mut Value) -> anyhow::Result<()> {
    let step = step.as_object_mut().context("step")?;
    replace_dynamic(step, "id", json!("s_fixture"))?;
    replace_dynamic(step, "parent_task_id", json!("t_parent"))?;
    replace_dynamic(step, "created_at", json!(101))?;
    replace_dynamic(step, "updated_at", json!(102))?;
    if !step["resolved_at"].is_null() {
        replace_dynamic(step, "resolved_at", json!(103))?;
    }
    if let Some(linked) = step["linked_task"].as_object_mut() {
        anyhow::ensure!(!linked.contains_key("claim_token"), "claim_token leaked");
        replace_dynamic(linked, "id", json!("t_linked"))?;
        replace_dynamic(linked, "board_id", json!("b_fixture"))?;
        replace_dynamic(linked, "created_at", json!(104))?;
        replace_dynamic(linked, "updated_at", json!(105))?;
        for (key, replacement) in [
            ("started_at", json!(106)),
            ("claim_expires_at", json!(107)),
            ("last_heartbeat_at", json!(108)),
            ("current_run_id", json!("r_fixture")),
        ] {
            if !linked[key].is_null() {
                replace_dynamic(linked, key, replacement)?;
            }
        }
    }
    Ok(())
}

fn normalize_plan(plan: &mut Value) -> anyhow::Result<()> {
    let plan = plan.as_object_mut().context("execution plan")?;
    replace_dynamic(plan, "board_id", json!("b_fixture"))?;
    replace_dynamic(plan, "task_id", json!("t_parent"))?;
    replace_dynamic(plan, "updated_at", json!(101))?;
    Ok(())
}

fn verify_step<T: DeserializeOwned>(
    operation: &str,
    mut output: Value,
    expected_status: &str,
) -> anyhow::Result<()> {
    serde_json::from_value::<T>(output.clone())?;
    anyhow::ensure!(output["data"]["status"] == expected_status);
    normalize_step(&mut output["data"])?;
    assert_eq!(output, fixture(operation)?);
    Ok(())
}

#[test]
fn task_step_add_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let (temp, parent, linked) = setup("task_step_add_output_fixture_is_produced_by_real_cli")?;
    claim_task(&temp, &linked)?;
    verify_step::<CliTaskStepAddOutput>(
        "add",
        add_step(&temp, &parent, Some(&linked), true)?,
        "todo",
    )
}

#[test]
fn task_step_update_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let (temp, parent, _) = setup("task_step_update_output_fixture_is_produced_by_real_cli")?;
    let created = add_step(&temp, &parent, None, true)?;
    let id = step_id(&created)?;
    let output = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture-editor",
            "task",
            "step",
            "update",
            &parent,
            &id,
            "--title",
            "Updated contract",
            "--body",
            "Updated public wire",
            "--position",
            "3072",
            "--required",
        ],
    )?
    .success_json()?;
    verify_step::<CliTaskStepUpdateOutput>("update", output, "todo")
}

#[test]
fn task_step_done_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let (temp, parent, _) = setup("task_step_done_output_fixture_is_produced_by_real_cli")?;
    let created = add_step(&temp, &parent, None, true)?;
    let id = step_id(&created)?;
    let output = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture-finisher",
            "task",
            "step",
            "done",
            &parent,
            &id,
            "--note",
            "contract complete",
        ],
    )?
    .success_json()?;
    verify_step::<CliTaskStepDoneOutput>("done", output, "done")
}

#[test]
fn task_step_skip_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let (temp, parent, _) = setup("task_step_skip_output_fixture_is_produced_by_real_cli")?;
    let created = add_step(&temp, &parent, None, true)?;
    let id = step_id(&created)?;
    let output = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture-skipper",
            "task",
            "step",
            "skip",
            &parent,
            &id,
            "--reason",
            "not needed",
        ],
    )?
    .success_json()?;
    verify_step::<CliTaskStepSkipOutput>("skip", output, "skipped")
}

#[test]
fn task_step_reopen_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let (temp, parent, _) = setup("task_step_reopen_output_fixture_is_produced_by_real_cli")?;
    let created = add_step(&temp, &parent, None, true)?;
    let id = step_id(&created)?;
    kanban(
        &temp.path,
        &[
            "--actor",
            "fixture",
            "task",
            "step",
            "done",
            &parent,
            &id,
            "--note",
            "first pass",
        ],
    )?
    .success()?;
    let output = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture-reopener",
            "task",
            "step",
            "reopen",
            &parent,
            &id,
            "--reason",
            "needs revision",
        ],
    )?
    .success_json()?;
    verify_step::<CliTaskStepReopenOutput>("reopen", output, "todo")
}

#[test]
fn task_step_remove_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let (temp, parent, linked) = setup("task_step_remove_output_fixture_is_produced_by_real_cli")?;
    let created = add_step(&temp, &parent, Some(&linked), true)?;
    let id = step_id(&created)?;
    let mut output = kanban(
        &temp.path,
        &[
            "--json", "--actor", "fixture", "task", "step", "remove", &parent, &id,
        ],
    )?
    .success_json()?;
    serde_json::from_value::<CliTaskStepRemoveOutput>(output.clone())?;
    anyhow::ensure!(output["data"]["removed"] == true);
    let listed =
        kanban(&temp.path, &["--json", "task", "step", "list", &parent])?.success_json()?;
    anyhow::ensure!(
        listed["data"]["steps"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "removed step relation remained visible"
    );
    normalize_step(&mut output["data"]["step"])?;
    assert_eq!(output, fixture("remove")?);
    Ok(())
}

#[test]
fn task_step_not_required_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = TempDb::new("task_step_not_required_output_fixture_is_produced_by_real_cli")?;
    kanban(&temp.path, &["init"])?.success()?;
    let parent = create_task(&temp, "Plan-free task")?;
    let mut output = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture-planner",
            "task",
            "step",
            "not-required",
            &parent,
            "--reason",
            "single action",
        ],
    )?
    .success_json()?;
    serde_json::from_value::<CliTaskStepNotRequiredOutput>(output.clone())?;
    anyhow::ensure!(output["data"]["state"] == "not_required");
    normalize_plan(&mut output["data"])?;
    assert_eq!(output, fixture("not-required")?);
    Ok(())
}

macro_rules! consumer_test {
    ($name:ident, $root:ty, $operation:literal) => {
        #[test]
        fn $name() -> anyhow::Result<()> {
            consume::<$root>($operation)
        }
    };
}

consumer_test!(
    task_step_add_output_fixture_is_consumed_by_contract_root,
    CliTaskStepAddOutput,
    "add"
);
consumer_test!(
    task_step_update_output_fixture_is_consumed_by_contract_root,
    CliTaskStepUpdateOutput,
    "update"
);
consumer_test!(
    task_step_done_output_fixture_is_consumed_by_contract_root,
    CliTaskStepDoneOutput,
    "done"
);
consumer_test!(
    task_step_skip_output_fixture_is_consumed_by_contract_root,
    CliTaskStepSkipOutput,
    "skip"
);
consumer_test!(
    task_step_reopen_output_fixture_is_consumed_by_contract_root,
    CliTaskStepReopenOutput,
    "reopen"
);
consumer_test!(
    task_step_remove_output_fixture_is_consumed_by_contract_root,
    CliTaskStepRemoveOutput,
    "remove"
);
consumer_test!(
    task_step_not_required_output_fixture_is_consumed_by_contract_root,
    CliTaskStepNotRequiredOutput,
    "not-required"
);
