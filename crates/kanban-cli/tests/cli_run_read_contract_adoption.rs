mod common;

use std::path::Path;

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_contract::{CliRunLogsOutput, CliRunShowOutput, CliRunsOutput};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

struct Setup {
    temp: TempDb,
    task_ref: String,
    run_id: String,
}

fn fixture(path: &str) -> anyhow::Result<Value> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Ok(serde_json::from_str(&std::fs::read_to_string(
        root.join(path),
    )?)?)
}

fn consume<T: DeserializeOwned>(path: &str) -> anyhow::Result<()> {
    serde_json::from_value::<T>(fixture(path)?).context("valid CLI run output fixture")?;
    Ok(())
}

fn setup(name: &str) -> anyhow::Result<Setup> {
    let temp = TempDb::new(name)?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture",
            "task",
            "create",
            "Run contract fixture",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"].as_str().context("task id")?;
    let task_ref = created["data"]["ref"]
        .as_str()
        .context("task ref")?
        .to_owned();
    kanban_sqlite::api::mark_execution_plan_not_required(
        &temp.path,
        "default",
        "fixture",
        task_id,
        "run fixture has no plan",
    )?;
    let logs = temp.dir.join("logs");
    let dispatch = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture",
            "dispatch",
            "--once",
            "--command",
            "printf '0123456789abcdef\\n'",
            "--log-dir",
            logs.to_str().context("log dir")?,
        ],
    )?
    .success_json()?;
    let run_id = dispatch["data"]["run_id"]
        .as_str()
        .context("run id")?
        .to_owned();
    Ok(Setup {
        temp,
        task_ref,
        run_id,
    })
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

fn normalize_run(run: &mut Value) -> anyhow::Result<()> {
    let run = run.as_object_mut().context("run")?;
    anyhow::ensure!(!run.contains_key("claim_token"), "claim_token leaked");
    anyhow::ensure!(!run.contains_key("log_path"), "log_path leaked");
    replace_dynamic(run, "id", json!("r_fixture"))?;
    replace_dynamic(run, "task_id", json!("t_fixture"))?;
    replace_dynamic(run, "started_at", json!(101))?;
    replace_dynamic(run, "finished_at", json!(102))
}

#[test]
fn runs_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let setup = setup("runs_output_fixture_is_produced_by_real_cli")?;
    let mut output =
        kanban(&setup.temp.path, &["--json", "runs", &setup.task_ref])?.success_json()?;
    serde_json::from_value::<CliRunsOutput>(output.clone())?;
    let runs = output["data"].as_array_mut().context("runs")?;
    anyhow::ensure!(runs.len() == 1, "expected one run");
    normalize_run(&mut runs[0])?;
    assert_eq!(
        output,
        fixture("schemas/fixtures/cli/runs-output.v1.valid.json")?
    );
    Ok(())
}

#[test]
fn runs_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<CliRunsOutput>("schemas/fixtures/cli/runs-output.v1.valid.json")
}

#[test]
fn run_show_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let setup = setup("run_show_output_fixture_is_produced_by_real_cli")?;
    let mut output =
        kanban(&setup.temp.path, &["--json", "run", "show", &setup.run_id])?.success_json()?;
    serde_json::from_value::<CliRunShowOutput>(output.clone())?;
    normalize_run(&mut output["data"])?;
    assert_eq!(
        output,
        fixture("schemas/fixtures/cli/run-show-output.v1.valid.json")?
    );
    Ok(())
}

#[test]
fn run_show_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<CliRunShowOutput>("schemas/fixtures/cli/run-show-output.v1.valid.json")
}

#[test]
fn run_logs_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let setup = setup("run_logs_output_fixture_is_produced_by_real_cli")?;
    let mut output = kanban(
        &setup.temp.path,
        &["--json", "run", "logs", &setup.run_id, "--tail-bytes", "6"],
    )?
    .success_json()?;
    serde_json::from_value::<CliRunLogsOutput>(output.clone())?;
    let log = output["data"].as_object_mut().context("run log")?;
    replace_dynamic(log, "run_id", json!("r_fixture"))?;
    assert_eq!(
        output,
        fixture("schemas/fixtures/cli/run-logs-output.v1.valid.json")?
    );
    Ok(())
}

#[test]
fn run_logs_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<CliRunLogsOutput>("schemas/fixtures/cli/run-logs-output.v1.valid.json")
}
