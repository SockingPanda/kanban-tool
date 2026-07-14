mod common;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_contract::{CliTaskClaimOutput, CliTaskReclaimOutput, CliTaskStartOutput};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn fixture(operation: &str) -> anyhow::Result<Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join(format!(
        "schemas/fixtures/cli/task-{operation}-output.v1.valid.json"
    ));
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

fn consume<T: DeserializeOwned>(operation: &str) -> anyhow::Result<()> {
    serde_json::from_value::<T>(fixture(operation)?).context("valid CLI task claim fixture")?;
    Ok(())
}

fn create_ready_task(temp: &TempDb) -> anyhow::Result<String> {
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture",
            "task",
            "create",
            "Claim contract task",
            "--description",
            "claim specification",
            "--assignee",
            "fixture-owner",
            "--priority",
            "2",
            "--max-retries",
            "4",
            "--metadata",
            "{\"cohort\":\"cli-claim\"}",
        ],
    )?
    .success_json()?;
    let task_ref = created["data"]["ref"]
        .as_str()
        .context("task ref")?
        .to_owned();
    kanban(
        &temp.path,
        &[
            "--actor",
            "fixture",
            "task",
            "step",
            "not-required",
            &task_ref,
            "--reason",
            "contract fixture",
        ],
    )?
    .success()?;
    Ok(task_ref)
}

fn claim_output(temp: &TempDb, operation: &str, ttl_ms: &str) -> anyhow::Result<Value> {
    let task_ref = create_ready_task(temp)?;
    kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture-worker",
            "task",
            operation,
            &task_ref,
            "--ttl-ms",
            ttl_ms,
        ],
    )?
    .success_json()
}

fn normalize_claim(output: &mut Value) -> anyhow::Result<()> {
    let data = output["data"].as_object_mut().context("claim data")?;
    let token = data.get_mut("claim_token").context("claim token")?;
    anyhow::ensure!(token.as_str().is_some_and(|value| !value.is_empty()));
    *token = json!("claim_fixture");
    let claim_expires_at = data
        .get_mut("claim_expires_at")
        .context("claim expires at")?;
    anyhow::ensure!(claim_expires_at.is_i64());
    *claim_expires_at = json!(107);

    let task = data
        .get_mut("task")
        .and_then(Value::as_object_mut)
        .context("claim task")?;
    anyhow::ensure!(
        !task.contains_key("claim_token"),
        "nested task token leaked"
    );
    for (key, replacement) in [
        ("id", json!("t_fixture")),
        ("board_id", json!("b_fixture")),
        ("created_at", json!(101)),
        ("updated_at", json!(102)),
        ("started_at", json!(104)),
        ("claim_expires_at", json!(107)),
        ("last_heartbeat_at", json!(108)),
        ("current_run_id", json!("r_fixture")),
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

    let run = data
        .get_mut("run")
        .and_then(Value::as_object_mut)
        .context("claim run")?;
    anyhow::ensure!(!run.contains_key("claim_token"), "run token leaked");
    anyhow::ensure!(!run.contains_key("log_path"), "run log_path leaked");
    for (key, replacement) in [
        ("id", json!("r_fixture")),
        ("task_id", json!("t_fixture")),
        ("started_at", json!(104)),
    ] {
        let current = run
            .get_mut(key)
            .with_context(|| format!("missing run.{key}"))?;
        anyhow::ensure!(
            (replacement.is_string() && current.as_str().is_some_and(|value| !value.is_empty()))
                || (replacement.is_i64() && current.is_i64()),
            "invalid run.{key}"
        );
        *current = replacement;
    }
    Ok(())
}

fn verify_claim<T: DeserializeOwned>(operation: &str, mut output: Value) -> anyhow::Result<()> {
    serde_json::from_value::<T>(output.clone())?;
    anyhow::ensure!(output["data"]["task"]["status"] == "running");
    anyhow::ensure!(output["data"]["run"]["status"] == "running");
    anyhow::ensure!(
        output["data"]["run"]["id"] == output["data"]["task"]["current_run_id"],
        "run/task identity drift"
    );
    anyhow::ensure!(
        output["data"]["run"]["task_id"] == output["data"]["task"]["id"],
        "run.task_id/task.id identity drift"
    );
    anyhow::ensure!(
        output["data"]["claim_expires_at"] == output["data"]["task"]["claim_expires_at"],
        "claim/task lease expiry drift"
    );
    normalize_claim(&mut output)?;
    assert_eq!(output, fixture(operation)?);
    Ok(())
}

#[test]
fn task_claim_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = TempDb::new("task_claim_output_fixture_is_produced_by_real_cli")?;
    verify_claim::<CliTaskClaimOutput>("claim", claim_output(&temp, "claim", "60000")?)
}

#[test]
fn task_start_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = TempDb::new("task_start_output_fixture_is_produced_by_real_cli")?;
    verify_claim::<CliTaskStartOutput>("start", claim_output(&temp, "start", "60000")?)
}

#[test]
fn task_reclaim_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = TempDb::new("task_reclaim_output_fixture_is_produced_by_real_cli")?;
    let claim = claim_output(&temp, "claim", "1")?;
    let expires_at = claim["data"]["claim_expires_at"]
        .as_i64()
        .context("claim expires at")?;
    let wait_started = Instant::now();
    loop {
        let now = i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
        if now > expires_at {
            break;
        }
        anyhow::ensure!(
            wait_started.elapsed() <= Duration::from_secs(2),
            "claim did not expire within monotonic timeout"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let output = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture",
            "task",
            "reclaim",
            "--expired",
        ],
    )?
    .success_json()?;
    serde_json::from_value::<CliTaskReclaimOutput>(output.clone())?;
    anyhow::ensure!(output["data"]["reclaimed"] == 1);
    assert_eq!(output, fixture("reclaim")?);
    Ok(())
}

#[test]
fn task_claim_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<CliTaskClaimOutput>("claim")
}

#[test]
fn task_start_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<CliTaskStartOutput>("start")
}

#[test]
fn task_reclaim_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<CliTaskReclaimOutput>("reclaim")
}
