mod common;

use common::{TempDb, kanban, kanban_in_dir_str_envs, kanban_with_stdin};
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn codex_hook_install_status_and_uninstall_preserve_user_hooks() -> anyhow::Result<()> {
    let temp = TempDb::new("codex_hook_install_status_and_uninstall")?;
    let codex_home = temp.dir.join("codex-home");
    std::fs::create_dir_all(&codex_home)?;
    let hooks_path = codex_home.join("hooks.json");
    std::fs::write(
        &hooks_path,
        r#"{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "^Bash$",
        "hooks": [
          {"type":"command","command":"echo user hook"}
        ]
      }
    ]
  }
}
"#,
    )?;
    let codex_home = codex_home.to_string_lossy().into_owned();

    let installed = kanban_in_dir_str_envs(
        &temp.path,
        &[
            "--json",
            "hook",
            "codex",
            "install",
            "--handler-command",
            "kanban hook codex handle",
            "--record-signals",
        ],
        &temp.dir,
        &[("CODEX_HOME", codex_home.as_str())],
    )?
    .success_json()?;
    assert_eq!(installed["data"]["installed"], true);
    assert_eq!(installed["data"]["managed_hook_count"], 2);
    assert!(
        installed["data"]["handler_commands"][0]
            .as_str()
            .unwrap()
            .contains("kanban hook codex handle failure")
    );
    assert!(
        installed["data"]["handler_commands"][0]
            .as_str()
            .unwrap()
            .contains("--record-signals")
    );
    assert!(
        installed["data"]["handler_commands"][1]
            .as_str()
            .unwrap()
            .contains("kanban hook codex handle task-create")
    );
    assert!(
        !installed["data"]["handler_commands"][1]
            .as_str()
            .unwrap()
            .contains("--record-signals")
    );

    let reinstalled = kanban_in_dir_str_envs(
        &temp.path,
        &[
            "--json",
            "hook",
            "codex",
            "install",
            "--handler-command",
            "kanban hook codex handle",
            "--record-signals",
        ],
        &temp.dir,
        &[("CODEX_HOME", codex_home.as_str())],
    )?
    .success_json()?;
    assert_eq!(reinstalled["data"]["managed_hook_count"], 2);

    let status = kanban_in_dir_str_envs(
        &temp.path,
        &["--json", "hook", "codex", "status"],
        &temp.dir,
        &[("CODEX_HOME", codex_home.as_str())],
    )?
    .success_json()?;
    assert_eq!(status["data"]["installed"], true);
    assert_eq!(status["data"]["managed_hook_count"], 2);

    let config: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&hooks_path)?)?;
    let hooks = config["hooks"]["PostToolUse"].as_array().unwrap();
    let commands = hooks
        .iter()
        .flat_map(|group| group["hooks"].as_array().unwrap())
        .filter_map(|hook| hook["command"].as_str())
        .collect::<Vec<_>>();
    assert!(commands.contains(&"echo user hook"));
    assert_eq!(
        commands
            .iter()
            .filter(|command| command.contains("kanban-hook-codex"))
            .count(),
        2
    );
    assert!(!temp.dir.join(".codex/hooks.json").exists());

    let uninstalled = kanban_in_dir_str_envs(
        &temp.path,
        &["--json", "hook", "codex", "uninstall"],
        &temp.dir,
        &[("CODEX_HOME", codex_home.as_str())],
    )?
    .success_json()?;
    assert_eq!(uninstalled["data"]["removed_hook_count"], 2);

    let config: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&hooks_path)?)?;
    let commands = config["hooks"]["PostToolUse"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|group| group["hooks"].as_array().unwrap())
        .filter_map(|hook| hook["command"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(commands, vec!["echo user hook"]);
    Ok(())
}

#[test]
fn codex_hook_handle_ignores_non_kanban_bash() -> anyhow::Result<()> {
    let temp = TempDb::new("codex_hook_handle_ignores_non_kanban_bash")?;
    let payload = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "echo hello"},
        "tool_response": {"exit_code": 0, "stdout": "hello\n", "stderr": ""}
    });

    let stdout = kanban_with_stdin(
        &temp.path,
        &["hook", "codex", "handle", "failure"],
        &payload.to_string(),
    )?
    .success_stdout()?;
    assert_eq!(stdout, "");
    Ok(())
}

#[test]
fn codex_hook_handle_detects_kanban_after_shell_separator() -> anyhow::Result<()> {
    let temp = TempDb::new("codex_hook_handle_detects_kanban_after_shell_separator")?;
    let payload = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "cd /tmp && kanban task list --bad-flag"},
        "tool_response": {"exit_code": 2, "stdout": "", "stderr": "unexpected argument --bad-flag"}
    });

    let stdout = kanban_with_stdin(
        &temp.path,
        &["hook", "codex", "handle", "failure"],
        &payload.to_string(),
    )?
    .success_stdout()?;
    let response: serde_json::Value = serde_json::from_str(&stdout)?;
    let message = response["systemMessage"].as_str().unwrap();
    assert!(message.contains("cd /tmp && kanban task list --bad-flag"));
    Ok(())
}

#[test]
fn codex_hook_handle_reports_failed_kanban_command() -> anyhow::Result<()> {
    let temp = TempDb::new("codex_hook_handle_reports_failed_kanban_command")?;
    let payload = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_use_id": "tool_1",
        "tool_input": {"command": "kanban task create --bad-flag"},
        "tool_response": {"exit_code": 2, "stdout": "", "stderr": "unexpected argument --bad-flag"}
    });

    let stdout = kanban_with_stdin(
        &temp.path,
        &["hook", "codex", "handle", "failure"],
        &payload.to_string(),
    )?
    .success_stdout()?;
    let response: serde_json::Value = serde_json::from_str(&stdout)?;
    let message = response["systemMessage"].as_str().unwrap();
    assert!(message.contains("kanban CLI command failed"));
    assert!(message.contains("spawn a native debugger agent"));
    assert!(message.contains("unexpected argument --bad-flag"));
    Ok(())
}

#[test]
fn codex_hook_failure_handler_ignores_successful_task_create() -> anyhow::Result<()> {
    let temp = TempDb::new("codex_hook_failure_handler_ignores_successful_task_create")?;
    let payload = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "kanban --json task create 'new work'"},
        "tool_response": {
            "exit_code": 0,
            "stdout": r#"{"data":{"ref":"default#42"}}"#,
            "stderr": ""
        }
    });

    let stdout = kanban_with_stdin(
        &temp.path,
        &["hook", "codex", "handle", "failure"],
        &payload.to_string(),
    )?
    .success_stdout()?;
    assert_eq!(stdout, "");
    Ok(())
}

#[test]
fn codex_hook_handle_records_signal_when_enabled() -> anyhow::Result<()> {
    let temp = TempDb::new("codex_hook_handle_records_signal_when_enabled")?;
    kanban(&temp.path, &["--board", "default", "init"])?.success()?;
    let payload = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_use_id": "tool_2",
        "tool_input": {"command": "kanban task list --bad-flag"},
        "tool_response": {"exit_code": 2, "stdout": "", "stderr": "unexpected argument --bad-flag"}
    });

    let stdout = kanban_with_stdin(
        &temp.path,
        &[
            "--board",
            "default",
            "hook",
            "codex",
            "handle",
            "failure",
            "--record-signals",
        ],
        &payload.to_string(),
    )?
    .success_stdout()?;
    let response: serde_json::Value = serde_json::from_str(&stdout)?;
    assert!(
        response["systemMessage"]
            .as_str()
            .unwrap()
            .contains("Recorded generic signal")
    );

    let signals = kanban(
        &temp.path,
        &["--json", "--board", "default", "signal", "list"],
    )?
    .success_json()?;
    assert_eq!(signals["data"].as_array().unwrap().len(), 1);
    assert_eq!(signals["data"][0]["kind"], "agent_cli_failure");
    assert_eq!(
        signals["data"][0]["observation"]["source"],
        "kanban-hook-codex"
    );
    Ok(())
}

#[test]
fn codex_hook_task_create_handler_ignores_failed_task_create() -> anyhow::Result<()> {
    let temp = TempDb::new("codex_hook_task_create_handler_ignores_failed_task_create")?;
    let payload = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "kanban task create --bad-flag"},
        "tool_response": {"exit_code": 2, "stdout": "", "stderr": "unexpected argument --bad-flag"}
    });

    let stdout = kanban_with_stdin(
        &temp.path,
        &["hook", "codex", "handle", "task-create"],
        &payload.to_string(),
    )?
    .success_stdout()?;
    assert_eq!(stdout, "");
    Ok(())
}

#[test]
fn codex_hook_handle_advises_after_task_create() -> anyhow::Result<()> {
    let temp = TempDb::new("codex_hook_handle_advises_after_task_create")?;
    let payload = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "kanban --json --board default task create 'new work'"},
        "tool_response": {
            "exit_code": 0,
            "stdout": r#"{"data":{"ref":"default#42","id":"t_42"}}"#,
            "stderr": ""
        }
    });

    let stdout = kanban_with_stdin(
        &temp.path,
        &["hook", "codex", "handle", "task-create"],
        &payload.to_string(),
    )?
    .success_stdout()?;
    let response: serde_json::Value = serde_json::from_str(&stdout)?;
    let message = response["systemMessage"].as_str().unwrap();
    assert!(message.contains("kanban task create succeeded for `default#42`"));
    assert!(message.contains("kanban label suggest <task_ref> --json"));
    assert!(message.contains("Do not write label ontology automatically"));
    Ok(())
}
