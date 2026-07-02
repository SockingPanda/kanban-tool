mod common;

use common::{TempDb, kanban, kanban_in_dir_str_envs, kanban_with_stdin};
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn codex_hook_install_status_and_uninstall_preserve_user_hooks() -> anyhow::Result<()> {
    let temp = TempDb::new("codex_hook_install_status_and_uninstall")?;
    let codex_home = temp.dir.join("codex-home");
    let prompt_config_path = temp
        .dir
        .join(".xdg-config")
        .join("kanban")
        .join("codex-hooks.json");
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
    assert_eq!(installed["data"]["prompt_config_created"], true);
    assert_eq!(
        installed["data"]["prompt_config"]["path"],
        prompt_config_path.to_string_lossy().as_ref()
    );
    assert_eq!(installed["data"]["prompt_config"]["exists"], true);
    assert_eq!(installed["data"]["prompt_config"]["valid"], true);
    assert_eq!(
        installed["data"]["prompt_config"]["bindings"]["failure"],
        "failure.zh-default"
    );
    assert_eq!(
        installed["data"]["prompt_config"]["bindings"]["task_create"],
        "task_create.zh-default"
    );
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
    assert_eq!(reinstalled["data"]["prompt_config_created"], false);
    assert!(prompt_config_path.exists());
    assert!(!temp.dir.join(".xdg-config").join("kb").exists());
    let prompt_config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&prompt_config_path)?)?;
    assert_eq!(
        prompt_config["codex_hooks"]["prompts"]["failure.zh-default"],
        "检测到 kanban CLI 命令失败。\n\n命令：{{command}}\n退出码：{{exit_code}}\n\n继续调整。调整成功后，视情况 spawn fork_turns=3 的 kanban-signal-recorder native agent。"
    );

    let status = kanban_in_dir_str_envs(
        &temp.path,
        &["--json", "hook", "codex", "status"],
        &temp.dir,
        &[("CODEX_HOME", codex_home.as_str())],
    )?
    .success_json()?;
    assert_eq!(status["data"]["installed"], true);
    assert_eq!(status["data"]["managed_hook_count"], 2);
    assert_eq!(status["data"]["prompt_config"]["valid"], true);

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
    assert!(message.contains("检测到 kanban CLI 命令失败"));
    assert!(message.contains("命令：kanban task create --bad-flag"));
    assert!(message.contains("退出码：2"));
    assert!(message.contains("spawn fork_turns=3 的 kanban-signal-recorder native agent"));
    assert!(!message.contains("unexpected argument --bad-flag"));
    Ok(())
}

#[test]
fn codex_hook_handle_uses_prompt_config_for_failure() -> anyhow::Result<()> {
    let temp = TempDb::new("codex_hook_handle_uses_prompt_config_for_failure")?;
    let prompt_dir = temp.dir.join(".xdg-config").join("kanban");
    std::fs::create_dir_all(&prompt_dir)?;
    std::fs::write(
        prompt_dir.join("codex-hooks.json"),
        r#"{
  "version": 1,
  "codex_hooks": {
    "bindings": {
      "failure": "failure.custom"
    },
    "prompts": {
      "failure.custom": "失败：{{command}} / {{exit_code}}"
    }
  }
}
"#,
    )?;
    let payload = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "kanban task list --bad-flag"},
        "tool_response": {"exit_code": 2, "stdout": "", "stderr": "unexpected argument --bad-flag"}
    });

    let stdout = kanban_with_stdin(
        &temp.path,
        &["hook", "codex", "handle", "failure"],
        &payload.to_string(),
    )?
    .success_stdout()?;
    let response: serde_json::Value = serde_json::from_str(&stdout)?;
    assert_eq!(
        response["systemMessage"],
        "失败：kanban task list --bad-flag / 2"
    );
    Ok(())
}

#[test]
fn codex_hook_handle_falls_back_when_prompt_config_is_invalid() -> anyhow::Result<()> {
    let temp = TempDb::new("codex_hook_handle_falls_back_when_prompt_config_is_invalid")?;
    let prompt_dir = temp.dir.join(".xdg-config").join("kanban");
    std::fs::create_dir_all(&prompt_dir)?;
    std::fs::write(prompt_dir.join("codex-hooks.json"), "{not json")?;
    let payload = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "kanban task list --bad-flag"},
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
    assert!(message.contains("检测到 kanban CLI 命令失败"));
    assert!(message.contains("提示：Codex hook prompt 配置不可用"));
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
            .contains("已记录 generic signal")
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
    assert!(message.contains("检测到 kanban task 创建成功"));
    assert!(message.contains("命令：kanban --json --board default task create 'new work'"));
    assert!(message.contains("任务：default#42"));
    assert!(message.contains("kanban label suggest default#42 --json"));
    assert!(message.contains("不要自动写 label ontology"));
    Ok(())
}

#[test]
fn codex_hook_handle_uses_prompt_config_for_task_create() -> anyhow::Result<()> {
    let temp = TempDb::new("codex_hook_handle_uses_prompt_config_for_task_create")?;
    let prompt_dir = temp.dir.join(".xdg-config").join("kanban");
    std::fs::create_dir_all(&prompt_dir)?;
    std::fs::write(
        prompt_dir.join("codex-hooks.json"),
        r#"{
  "version": 1,
  "codex_hooks": {
    "bindings": {
      "task_create": "task.custom"
    },
    "prompts": {
      "task.custom": "创建：{{task_ref}} via {{command}}"
    }
  }
}
"#,
    )?;
    let payload = json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "kanban --json task create 'new work'"},
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
    assert_eq!(
        response["systemMessage"],
        "创建：default#42 via kanban --json task create 'new work'"
    );
    Ok(())
}
