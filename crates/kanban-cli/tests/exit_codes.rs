mod common;

use std::path::Path;

use anyhow::Context;
use assert_cmd::Command;
use kanban_sqlite::maintenance_lock_path;
use serde_json::Value;

use common::{TempDb, kanban, kanban_in_dir};

#[test]
fn usage_errors_exit_2_before_runtime_json_envelope() -> anyhow::Result<()> {
    let output = Command::cargo_bin("kanban")?
        .args(["--json", "completions", "invalid-shell"])
        .output()?;

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid value 'invalid-shell'"), "{stderr}");
    Ok(())
}

#[test]
fn runtime_json_errors_include_stable_code_message_and_exit_code() -> anyhow::Result<()> {
    let temp = TempDb::new("runtime_json_errors_include_stable_code_message_and_exit_code")?;
    kanban(&temp.path, &["init"])?.success()?;

    let result = kanban(&temp.path, &["--json", "board", "show", "missing-board"])?;
    assert_exit_json(result.output, 3, "not_found")?;

    Ok(())
}

#[test]
fn validation_errors_exit_2() -> anyhow::Result<()> {
    let temp = TempDb::new("validation_errors_exit_2")?;
    kanban(&temp.path, &["init"])?.success()?;

    let result = kanban(&temp.path, &["--json", "task", "list", "--limit", "1001"])?;
    assert_exit_json(result.output, 2, "invalid_input")?;
    Ok(())
}

#[test]
fn command_layer_validation_errors_exit_2() -> anyhow::Result<()> {
    let temp = TempDb::new("command_layer_validation_errors_exit_2")?;
    kanban(&temp.path, &["init"])?.success()?;

    let unsupported_locale = kanban(&temp.path, &["--json", "--locale", "fr-FR", "task", "list"])?;
    assert_exit_json(unsupported_locale.output, 2, "invalid_input")?;

    let serve_non_loopback = kanban(
        &temp.path,
        &["--json", "serve", "--host", "0.0.0.0", "--port", "8721"],
    )?;
    assert_exit_json(serve_non_loopback.output, 2, "invalid_input")?;

    let description_path = temp.dir.join("description.md");
    std::fs::write(&description_path, "from file")?;
    let mutually_exclusive = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "bad description input",
            "--description",
            "inline",
            "--description-file",
            description_path.to_str().context("description path")?,
        ],
    )?;
    assert_exit_json(mutually_exclusive.output, 2, "invalid_input")?;

    let worker_profile = temp.dir.join("bad-workers.toml");
    std::fs::write(&worker_profile, "[workers.default]\nnot-a-key-value-line\n")?;
    let invalid_worker_profile = kanban(
        &temp.path,
        &[
            "--json",
            "dispatch",
            "--profile-config",
            worker_profile.to_str().context("worker profile path")?,
            "--max-iterations",
            "1",
        ],
    )?;
    assert_exit_json(invalid_worker_profile.output, 2, "invalid_input")?;

    let unsupported_vector_provider = kanban(
        &temp.path,
        &[
            "--json",
            "vector",
            "configure",
            "--provider",
            "not-ollama",
            "--skip-check",
        ],
    )?;
    assert_exit_json(unsupported_vector_provider.output, 2, "invalid_input")?;
    Ok(())
}

#[test]
fn state_transition_errors_exit_4() -> anyhow::Result<()> {
    let temp = TempDb::new("state_transition_errors_exit_4")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task = kanban(&temp.path, &["--json", "task", "create", "missing spec"])?.success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;

    let result = kanban(&temp.path, &["--json", "task", "claim", task_id])?;
    assert_exit_json(result.output, 4, "invalid_transition")?;
    Ok(())
}

#[test]
fn claim_conflicts_exit_5() -> anyhow::Result<()> {
    let temp = TempDb::new("claim_conflicts_exit_5")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "claim conflict task",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;
    mark_plan_not_required(&temp.path, task_id)?;
    kanban(&temp.path, &["--json", "task", "claim", task_id])?.success_json()?;

    let result = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "heartbeat",
            task_id,
            "--claim-token",
            "claim_wrong",
        ],
    )?;
    assert_exit_json(result.output, 5, "claim_conflict")?;
    Ok(())
}

#[test]
fn dependency_blocked_errors_exit_6() -> anyhow::Result<()> {
    let temp = TempDb::new("dependency_blocked_errors_exit_6")?;
    kanban(&temp.path, &["init"])?.success()?;
    let parent = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "parent task",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let child = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "child task",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let parent_id = parent["data"]["id"].as_str().context("parent id")?;
    let child_id = child["data"]["id"].as_str().context("child id")?;
    mark_plan_not_required(&temp.path, child_id)?;
    kanban(&temp.path, &["--json", "dep", "add", parent_id, child_id])?.success_json()?;

    let result = kanban(&temp.path, &["--json", "task", "promote", child_id])?;
    assert_exit_json(result.output, 6, "dependency_blocked")?;
    Ok(())
}

#[test]
fn sqlite_locked_errors_exit_7() -> anyhow::Result<()> {
    let temp = TempDb::new("sqlite_locked_errors_exit_7")?;
    kanban(&temp.path, &["init"])?.success()?;
    let lock_path = maintenance_lock_path(&temp.path);
    std::fs::write(&lock_path, format!("pid={}", std::process::id()))?;

    let result = kanban_in_dir(Path::new("kb.db"), &["--json", "doctor"], &temp.dir)?;
    assert_exit_json(result.output, 7, "sqlite_busy")?;
    std::fs::remove_file(lock_path)?;
    Ok(())
}

fn mark_plan_not_required(db_path: &Path, task_id: &str) -> anyhow::Result<()> {
    kanban(
        db_path,
        &[
            "--json",
            "task",
            "step",
            "not-required",
            task_id,
            "--reason",
            "exit code test fixture",
        ],
    )?
    .success_json()?;
    Ok(())
}

fn assert_exit_json(
    output: std::process::Output,
    expected_exit: i32,
    expected_code: &str,
) -> anyhow::Result<Value> {
    assert_eq!(output.status.code(), Some(expected_exit));
    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.is_empty(),
        "runtime --json errors should not write stderr, got: {stderr}"
    );
    let json: Value = serde_json::from_str(&stdout).with_context(|| stdout.clone())?;
    assert_eq!(json["error"]["code"], expected_code);
    assert_eq!(json["error"]["exit_code"], expected_exit);
    assert!(
        json["error"]["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "{json}"
    );
    Ok(json)
}
