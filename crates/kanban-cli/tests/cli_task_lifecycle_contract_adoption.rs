mod common;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_contract::{
    CliTaskArchiveOutput, CliTaskBlockOutput, CliTaskCompleteOutput, CliTaskDoneOutput,
    CliTaskHeartbeatOutput, CliTaskPromoteOutput, CliTaskReopenOutput, CliTaskReviewOutput,
    CliTaskUnblockOutput,
};
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
    serde_json::from_value::<T>(fixture(operation)?).context("valid CLI task lifecycle fixture")?;
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
    for (key, replacement) in [
        ("scheduled_at", 103),
        ("started_at", 104),
        ("completed_at", 105),
        ("archived_at", 106),
        ("claim_expires_at", 107),
        ("last_heartbeat_at", 108),
    ] {
        if let Some(current) = task.get_mut(key)
            && !current.is_null()
        {
            anyhow::ensure!(current.is_i64(), "invalid task.{key}");
            *current = json!(replacement);
        }
    }
    if let Some(current_run_id) = task.get_mut("current_run_id")
        && !current_run_id.is_null()
    {
        anyhow::ensure!(
            current_run_id
                .as_str()
                .is_some_and(|value| !value.is_empty()),
            "invalid task.current_run_id"
        );
        *current_run_id = json!("r_fixture");
    }
    Ok(())
}

fn create_task(temp: &TempDb, scheduled_at: Option<i64>) -> anyhow::Result<String> {
    kanban(&temp.path, &["init"])?.success()?;
    let scheduled_at_arg = scheduled_at.map(|value| value.to_string());
    let mut args = vec![
        "--json",
        "--actor",
        "fixture",
        "task",
        "create",
        "Lifecycle contract task",
        "--description",
        "lifecycle specification",
        "--assignee",
        "fixture-owner",
        "--priority",
        "2",
        "--max-retries",
        "4",
        "--metadata",
        "{\"cohort\":\"cli-lifecycle\"}",
    ];
    if let Some(value) = scheduled_at_arg.as_deref() {
        args.extend(["--scheduled-at", value]);
    }
    let created = kanban(&temp.path, &args)?.success_json()?;
    Ok(created["data"]["ref"]
        .as_str()
        .context("task ref")?
        .to_owned())
}

fn mark_plan_not_required(temp: &TempDb, task_ref: &str) -> anyhow::Result<()> {
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
            "contract fixture",
        ],
    )?
    .success()?;
    Ok(())
}

fn claim(temp: &TempDb, task_ref: &str) -> anyhow::Result<String> {
    let claim = kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture-worker",
            "task",
            "claim",
            task_ref,
            "--ttl-ms",
            "60000",
        ],
    )?
    .success_json()?;
    Ok(claim["data"]["claim_token"]
        .as_str()
        .context("claim token")?
        .to_owned())
}

fn output(temp: &TempDb, operation: &str, task_ref: &str, extra: &[&str]) -> anyhow::Result<Value> {
    let mut args = vec!["--json", "--actor", "fixture", "task", operation, task_ref];
    args.extend_from_slice(extra);
    kanban(&temp.path, &args)?.success_json()
}

fn verify<T: DeserializeOwned>(
    operation: &str,
    mut output: Value,
    expected_status: &str,
) -> anyhow::Result<()> {
    serde_json::from_value::<T>(output.clone())?;
    anyhow::ensure!(output["data"]["status"] == expected_status);
    normalize_task(&mut output["data"])?;
    assert_eq!(output, fixture(operation)?);
    Ok(())
}

#[test]
fn task_promote_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = TempDb::new("task_promote_output_fixture_is_produced_by_real_cli")?;
    let scheduled_at =
        i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())? + 250;
    let task_ref = create_task(&temp, Some(scheduled_at))?;
    mark_plan_not_required(&temp, &task_ref)?;
    std::thread::sleep(Duration::from_millis(300));
    verify::<CliTaskPromoteOutput>(
        "promote",
        output(&temp, "promote", &task_ref, &[])?,
        "ready",
    )
}

#[test]
fn task_reopen_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = TempDb::new("task_reopen_output_fixture_is_produced_by_real_cli")?;
    let task_ref = create_task(&temp, None)?;
    mark_plan_not_required(&temp, &task_ref)?;
    let token = claim(&temp, &task_ref)?;
    output(&temp, "done", &task_ref, &["--claim-token", &token])?;
    verify::<CliTaskReopenOutput>(
        "reopen",
        output(&temp, "reopen", &task_ref, &["--reason", "contract rerun"])?,
        "ready",
    )
}

#[test]
fn task_heartbeat_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = TempDb::new("task_heartbeat_output_fixture_is_produced_by_real_cli")?;
    let task_ref = create_task(&temp, None)?;
    mark_plan_not_required(&temp, &task_ref)?;
    let token = claim(&temp, &task_ref)?;
    verify::<CliTaskHeartbeatOutput>(
        "heartbeat",
        output(
            &temp,
            "heartbeat",
            &task_ref,
            &["--claim-token", &token, "--ttl-ms", "60000"],
        )?,
        "running",
    )
}

fn produce_running_exit<T: DeserializeOwned>(
    test_name: &str,
    operation: &str,
    expected_status: &str,
    extra: &[&str],
) -> anyhow::Result<()> {
    let temp = TempDb::new(test_name)?;
    let task_ref = create_task(&temp, None)?;
    mark_plan_not_required(&temp, &task_ref)?;
    let token = claim(&temp, &task_ref)?;
    let mut operation_extra = vec!["--claim-token", token.as_str()];
    operation_extra.extend_from_slice(extra);
    verify::<T>(
        operation,
        output(&temp, operation, &task_ref, &operation_extra)?,
        expected_status,
    )
}

#[test]
fn task_done_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    produce_running_exit::<CliTaskDoneOutput>(
        "task_done_output_fixture_is_produced_by_real_cli",
        "done",
        "done",
        &[],
    )
}

#[test]
fn task_complete_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    produce_running_exit::<CliTaskCompleteOutput>(
        "task_complete_output_fixture_is_produced_by_real_cli",
        "complete",
        "done",
        &[],
    )
}

#[test]
fn task_review_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    produce_running_exit::<CliTaskReviewOutput>(
        "task_review_output_fixture_is_produced_by_real_cli",
        "review",
        "review",
        &[],
    )
}

#[test]
fn task_block_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    produce_running_exit::<CliTaskBlockOutput>(
        "task_block_output_fixture_is_produced_by_real_cli",
        "block",
        "blocked",
        &["contract blocked"],
    )
}

#[test]
fn task_unblock_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = TempDb::new("task_unblock_output_fixture_is_produced_by_real_cli")?;
    let task_ref = create_task(&temp, None)?;
    mark_plan_not_required(&temp, &task_ref)?;
    output(&temp, "block", &task_ref, &["contract blocked", "--force"])?;
    verify::<CliTaskUnblockOutput>(
        "unblock",
        output(&temp, "unblock", &task_ref, &[])?,
        "ready",
    )
}

#[test]
fn task_archive_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = TempDb::new("task_archive_output_fixture_is_produced_by_real_cli")?;
    let task_ref = create_task(&temp, None)?;
    verify::<CliTaskArchiveOutput>(
        "archive",
        output(&temp, "archive", &task_ref, &[])?,
        "archived",
    )
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
    task_promote_output_fixture_is_consumed_by_contract_root,
    CliTaskPromoteOutput,
    "promote"
);
consumer_test!(
    task_reopen_output_fixture_is_consumed_by_contract_root,
    CliTaskReopenOutput,
    "reopen"
);
consumer_test!(
    task_heartbeat_output_fixture_is_consumed_by_contract_root,
    CliTaskHeartbeatOutput,
    "heartbeat"
);
consumer_test!(
    task_done_output_fixture_is_consumed_by_contract_root,
    CliTaskDoneOutput,
    "done"
);
consumer_test!(
    task_complete_output_fixture_is_consumed_by_contract_root,
    CliTaskCompleteOutput,
    "complete"
);
consumer_test!(
    task_review_output_fixture_is_consumed_by_contract_root,
    CliTaskReviewOutput,
    "review"
);
consumer_test!(
    task_block_output_fixture_is_consumed_by_contract_root,
    CliTaskBlockOutput,
    "block"
);
consumer_test!(
    task_unblock_output_fixture_is_consumed_by_contract_root,
    CliTaskUnblockOutput,
    "unblock"
);
consumer_test!(
    task_archive_output_fixture_is_consumed_by_contract_root,
    CliTaskArchiveOutput,
    "archive"
);
