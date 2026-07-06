mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use pretty_assertions::assert_eq;
use std::path::Path;

fn mark_no_plan_required(db_path: &Path, task_id: &str) -> anyhow::Result<()> {
    kanban_sqlite::mark_execution_plan_not_required(
        db_path,
        "default",
        "cli-dispatch-test",
        task_id,
        "dispatch test task does not need steps",
    )?;
    Ok(())
}
#[test]
fn dispatch_profile_routes_assignees() -> anyhow::Result<()> {
    let temp =
        TempDb::new("dispatch_loop_uses_worker_profile_config_and_respects_assignee_routing")?;
    kanban(&temp.path, &["init"])?.success()?;
    let backend = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "backend work",
            "--description",
            "ready spec",
            "--assignee",
            "backend",
        ],
    )?
    .success_json()?;
    let frontend = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "frontend work",
            "--description",
            "ready spec",
            "--assignee",
            "frontend",
        ],
    )?
    .success_json()?;
    mark_no_plan_required(
        &temp.path,
        backend["data"]["id"]
            .as_str()
            .context("expected JSON string")?,
    )?;
    mark_no_plan_required(
        &temp.path,
        frontend["data"]["id"]
            .as_str()
            .context("expected JSON string")?,
    )?;

    let config = temp.dir.join("workers.toml");
    let logs = temp.dir.join("logs");
    std::fs::write(
        &config,
        format!(
            "[workers.backend]\ncommand = \"sh -c 'true'\"\nclaim_ttl_ms = 60000\nheartbeat_interval_ms = 10\non_success = \"done\"\non_failure = \"blocked\"\nlog_dir = \"{}\"\n",
            logs.display()
        ),
    )?;

    let result = kanban(
        &temp.path,
        &[
            "--json",
            "dispatch",
            "--worker-profile",
            "backend",
            "--profile-config",
            config.to_str().context("expected UTF-8 path")?,
            "--max-iterations",
            "1",
        ],
    )?
    .success_json()?;

    assert_eq!(result["data"]["iterations"], 1);
    assert_eq!(result["data"]["claimed"], 1);
    let backend_task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "show",
            backend["data"]["id"]
                .as_str()
                .context("expected JSON string")?,
        ],
    )?
    .success_json()?;
    let frontend_task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "show",
            frontend["data"]["id"]
                .as_str()
                .context("expected JSON string")?,
        ],
    )?
    .success_json()?;
    assert_eq!(backend_task["data"]["status"], "done");
    assert_eq!(frontend_task["data"]["status"], "ready");
    Ok(())
}

#[test]
fn dispatch_rejects_untrusted_log_dir() -> anyhow::Result<()> {
    let temp = TempDb::new("dispatch_rejects_profile_log_dir_outside_trusted_roots")?;
    kanban(&temp.path, &["init"])?.success()?;
    let task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "unsafe log root",
            "--description",
            "ready spec",
            "--assignee",
            "backend",
        ],
    )?
    .success_json()?;
    mark_no_plan_required(
        &temp.path,
        task["data"]["id"]
            .as_str()
            .context("expected JSON string")?,
    )?;

    let config = temp.dir.join("workers.toml");
    let untrusted_logs = temp.dir.join("custom-logs");
    std::fs::write(
        &config,
        format!(
            "[workers.backend]\ncommand = \"sh -c 'true'\"\nclaim_ttl_ms = 60000\nheartbeat_interval_ms = 10\non_success = \"done\"\non_failure = \"blocked\"\nlog_dir = \"{}\"\n",
            untrusted_logs.display()
        ),
    )?;

    kanban(
        &temp.path,
        &[
            "--json",
            "dispatch",
            "--worker-profile",
            "backend",
            "--profile-config",
            config.to_str().context("expected UTF-8 path")?,
            "--max-iterations",
            "1",
        ],
    )?
    .json_failure_containing("outside allowed run log roots")?;

    let task = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "show",
            task["data"]["id"]
                .as_str()
                .context("expected JSON string")?,
        ],
    )?
    .success_json()?;
    assert_eq!(task["data"]["status"], "ready");
    Ok(())
}

#[test]
fn retry_policy_and_run_logs_support_recovery() -> anyhow::Result<()> {
    let temp = TempDb::new("retry_policy_and_run_log_commands_support_operator_recovery")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "retry policy",
            "--description",
            "ready spec",
            "--max-retries",
            "2",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    assert_eq!(created["data"]["max_retries"], 2);
    mark_no_plan_required(&temp.path, task_id)?;

    let updated = kanban(
        &temp.path,
        &["--json", "task", "update", task_id, "--clear-max-retries"],
    )?
    .success_json()?;
    assert!(updated["data"]["max_retries"].is_null());

    let reset = kanban(
        &temp.path,
        &["--json", "task", "update", task_id, "--max-retries", "1"],
    )?
    .success_json()?;
    assert_eq!(reset["data"]["max_retries"], 1);

    let logs = temp.dir.join("logs");
    let dispatch = kanban(
        &temp.path,
        &[
            "--json",
            "dispatch",
            "--once",
            "--command",
            "printf 'operator log\\n'",
            "--log-dir",
            logs.to_str().context("expected UTF-8 path")?,
        ],
    )?
    .success_json()?;
    let run_id = dispatch["data"]["run_id"]
        .as_str()
        .context("expected JSON string")?;

    let run = kanban(&temp.path, &["--json", "run", "show", run_id])?.success_json()?;
    assert_eq!(run["data"]["id"], run_id);
    assert!(run["data"].get("claim_token").is_some());

    let log = kanban(&temp.path, &["--json", "run", "logs", run_id])?.success_json()?;
    assert_eq!(log["data"]["run_id"], run_id);
    assert_eq!(log["data"]["content"], "operator log\n");
    assert_eq!(log["data"]["truncated"], false);
    Ok(())
}

#[test]
fn run_logs_reject_suspicious_paths() -> anyhow::Result<()> {
    let temp = TempDb::new("run_log_command_rejects_suspicious_log_paths")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "suspicious log",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    mark_no_plan_required(
        &temp.path,
        created["data"]["id"]
            .as_str()
            .context("expected JSON string")?,
    )?;
    let dispatch = kanban(
        &temp.path,
        &[
            "--json",
            "dispatch",
            "--once",
            "--command",
            "printf 'operator log\\n'",
            "--log-dir",
            temp.dir
                .join("logs")
                .to_str()
                .context("expected UTF-8 path")?,
        ],
    )?
    .success_json()?;
    let run_id = dispatch["data"]["run_id"]
        .as_str()
        .context("expected JSON string")?;
    kanban_sqlite::connect_file(&temp.path)?.execute(
        "UPDATE task_runs SET log_path=?1 WHERE id=?2",
        ("/etc/passwd", run_id),
    )?;

    kanban(&temp.path, &["run", "logs", run_id])?.failure_containing("suspicious run log path")?;
    Ok(())
}
