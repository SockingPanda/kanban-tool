mod common;

use common::{TempDb, kanban, kanban_with_stdin};
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn codex_hook_install_status_and_uninstall_preserve_user_hooks() -> anyhow::Result<()> {
    let temp = TempDb::new("codex_hook_install_status_and_uninstall")?;
    let codex_dir = temp.dir.join(".codex");
    std::fs::create_dir_all(&codex_dir)?;
    let hooks_path = codex_dir.join("hooks.json");
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

    let installed = kanban(
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
    )?
    .success_json()?;
    assert_eq!(installed["data"]["installed"], true);
    assert_eq!(installed["data"]["managed_hook_count"], 1);
    assert!(
        installed["data"]["handler_command"]
            .as_str()
            .unwrap()
            .contains("--record-signals")
    );

    let reinstalled = kanban(
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
    )?
    .success_json()?;
    assert_eq!(reinstalled["data"]["managed_hook_count"], 1);

    let status = kanban(&temp.path, &["--json", "hook", "codex", "status"])?.success_json()?;
    assert_eq!(status["data"]["installed"], true);
    assert_eq!(status["data"]["managed_hook_count"], 1);

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
        1
    );

    let uninstalled =
        kanban(&temp.path, &["--json", "hook", "codex", "uninstall"])?.success_json()?;
    assert_eq!(uninstalled["data"]["removed_hook_count"], 1);

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
        &["hook", "codex", "handle"],
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
        &["hook", "codex", "handle"],
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
        &["hook", "codex", "handle"],
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
        &["hook", "codex", "handle"],
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
