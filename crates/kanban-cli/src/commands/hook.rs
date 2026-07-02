use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use kanban_sqlite::{SignalRecordInput, record_signal};
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::{
    args::{
        CodexHookCommand, CodexHookHandleCommand, CodexHookInstallArgs, HookCommand,
    },
    commands::common::active_board,
    output::print_or_json,
};

const MANAGED_MARKER: &str = "kanban-hook-codex";
const POST_TOOL_USE: &str = "PostToolUse";
const BASH_MATCHER: &str = "^Bash$";
const FAILURE_STATUS_MESSAGE: &str = "Inspecting kanban CLI failure trace";
const TASK_CREATE_STATUS_MESSAGE: &str = "Inspecting kanban task create trace";

pub(crate) fn handle_hook(
    command: &HookCommand,
    db_path: &PathBuf,
    board: Option<&str>,
    actor: &str,
    json: bool,
) -> Result<()> {
    match command {
        HookCommand::Codex { command } => match command {
            CodexHookCommand::Install(args) => install_codex_hook(args, json),
            CodexHookCommand::Status => status_codex_hook(json),
            CodexHookCommand::Uninstall => uninstall_codex_hook(json),
            CodexHookCommand::Handle { command } => match command {
                CodexHookHandleCommand::Failure(args) => {
                    let _installed_by = args.installed_by.as_deref();
                    handle_codex_payload(
                        CodexHookPostHandler::Failure {
                            record_signals: args.record_signals,
                        },
                        db_path,
                        board,
                        actor,
                    )
                }
                CodexHookHandleCommand::TaskCreate(args) => {
                    let _installed_by = args.installed_by.as_deref();
                    handle_codex_payload(CodexHookPostHandler::TaskCreate, db_path, board, actor)
                }
            },
        },
    }
}

fn install_codex_hook(args: &CodexHookInstallArgs, json_output: bool) -> Result<()> {
    let path = hooks_config_path()?;
    let mut config = read_hooks_config(&path)?;
    remove_managed_hooks(&mut config)?;
    let managed_hooks = managed_hook_specs(&args.handler_command, args.record_signals);
    install_managed_hooks(&mut config, &managed_hooks, args.timeout)?;
    write_hooks_config(&path, &config)?;

    let status = inspect_hooks_config(&path, Some(&config))?;
    let output = CodexHookInstallOutput {
        path,
        installed: status.installed,
        matcher: BASH_MATCHER.to_owned(),
        handler_commands: managed_hooks
            .iter()
            .map(|hook| hook.command.clone())
            .collect(),
        managed_hook_count: status.managed_hook_count,
    };
    print_or_json(json_output, &output, || {
        format!(
            "Installed {} managed kanban Codex hooks at {}",
            output.managed_hook_count,
            output.path.display(),
        )
    })
}

fn status_codex_hook(json_output: bool) -> Result<()> {
    let path = hooks_config_path()?;
    let status = inspect_hooks_config(&path, None)?;
    print_or_json(json_output, &status, || {
        if status.installed {
            format!(
                "kanban Codex hook installed at {} ({} managed hook{})",
                status.path.display(),
                status.managed_hook_count,
                plural(status.managed_hook_count)
            )
        } else {
            format!(
                "kanban Codex hook not installed at {}",
                status.path.display()
            )
        }
    })
}

fn uninstall_codex_hook(json_output: bool) -> Result<()> {
    let path = hooks_config_path()?;
    let mut config = read_hooks_config(&path)?;
    let removed_hook_count = remove_managed_hooks(&mut config)?;
    write_hooks_config(&path, &config)?;
    let output = CodexHookUninstallOutput {
        path,
        removed_hook_count,
        installed: false,
    };
    print_or_json(json_output, &output, || {
        format!(
            "Removed {} managed kanban Codex hook{} from {}",
            output.removed_hook_count,
            plural(output.removed_hook_count),
            output.path.display()
        )
    })
}

fn handle_codex_payload(
    handler: CodexHookPostHandler,
    db_path: &PathBuf,
    board_arg: Option<&str>,
    actor: &str,
) -> Result<()> {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .context("failed to read Codex hook payload from stdin")?;
    let Some(payload) = parse_hook_payload(&raw) else {
        return Ok(());
    };
    let event = string_field(
        &payload,
        &["hook_event_name", "hookEventName", "event", "name"],
    );
    if event.as_deref().is_some_and(|value| value != POST_TOOL_USE) {
        return Ok(());
    }
    let normalized = normalize_post_tool_use(&payload);
    if !normalized.is_bash || !is_kanban_command(&normalized.command) {
        return Ok(());
    }

    match handler {
        CodexHookPostHandler::Failure { record_signals } => {
            if !normalized.failed() {
                return Ok(());
            }
            let record_result = if record_signals {
                Some(record_failure_signal(
                    db_path,
                    board_arg,
                    actor,
                    &payload,
                    &normalized,
                ))
            } else {
                None
            };
            write_hook_response(&HookResponse {
                system_message: failure_system_message(&normalized, record_result.as_ref()),
            })?;
        }
        CodexHookPostHandler::TaskCreate => {
            if normalized.failed() || !is_task_create_command(&normalized.command) {
                return Ok(());
            }
            write_hook_response(&HookResponse {
                system_message: task_create_system_message(&normalized),
            })?;
        }
    }
    Ok(())
}

fn hooks_config_path() -> Result<PathBuf> {
    if let Some(home) = env::var_os("CODEX_HOME") {
        return Ok(PathBuf::from(home).join("hooks.json"));
    }
    let home = env::var_os("HOME").context("HOME is not set; set CODEX_HOME")?;
    Ok(PathBuf::from(home).join(".codex/hooks.json"))
}

fn read_hooks_config(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(json!({ "hooks": {} }));
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(json!({ "hooks": {} }));
    }
    let value: Value = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {} as JSON", path.display()))?;
    if !value.is_object() {
        bail!("{} must contain a JSON object", path.display());
    }
    Ok(value)
}

fn write_hooks_config(path: &Path, config: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let content = serde_json::to_string_pretty(config)?;
    fs::write(path, format!("{content}\n"))
        .with_context(|| format!("failed to write {}", path.display()))
}

fn hooks_object_mut(config: &mut Value) -> Result<&mut Map<String, Value>> {
    let root = config
        .as_object_mut()
        .context("hooks config root must be a JSON object")?;
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    hooks
        .as_object_mut()
        .context("hooks config field `hooks` must be a JSON object")
}

fn install_managed_hooks(
    config: &mut Value,
    managed_hooks: &[ManagedHookSpec],
    timeout: u64,
) -> Result<()> {
    let hooks = hooks_object_mut(config)?;
    let post_tool_use = hooks
        .entry(POST_TOOL_USE)
        .or_insert_with(|| Value::Array(Vec::new()));
    let groups = post_tool_use
        .as_array_mut()
        .context("hooks.PostToolUse must be an array")?;
    let hooks = managed_hooks
        .iter()
        .map(|hook| {
            json!({
                "type": "command",
                "command": hook.command.clone(),
                "timeout": timeout,
                "statusMessage": hook.status_message
            })
        })
        .collect::<Vec<_>>();
    groups.push(json!({
        "matcher": BASH_MATCHER,
        "hooks": hooks
    }));
    Ok(())
}

fn remove_managed_hooks(config: &mut Value) -> Result<usize> {
    let hooks = hooks_object_mut(config)?;
    let Some(groups) = hooks.get_mut(POST_TOOL_USE).and_then(Value::as_array_mut) else {
        return Ok(0);
    };
    let mut removed = 0;
    for group in groups.iter_mut() {
        let Some(group_hooks) = group
            .as_object_mut()
            .and_then(|group| group.get_mut("hooks"))
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        let before = group_hooks.len();
        group_hooks.retain(|hook| !is_managed_hook(hook));
        removed += before.saturating_sub(group_hooks.len());
    }
    groups.retain(|group| {
        match group
            .as_object()
            .and_then(|group| group.get("hooks"))
            .and_then(Value::as_array)
        {
            Some(hooks) => !hooks.is_empty(),
            None => true,
        }
    });
    Ok(removed)
}

fn is_managed_hook(hook: &Value) -> bool {
    let Some(object) = hook.as_object() else {
        return false;
    };
    let Some(command) = object.get("command").and_then(Value::as_str) else {
        return false;
    };
    object.get("type").and_then(Value::as_str) == Some("command")
        && command.contains(MANAGED_MARKER)
}

fn inspect_hooks_config(path: &Path, config: Option<&Value>) -> Result<CodexHookStatusOutput> {
    let owned;
    let config = if let Some(config) = config {
        Some(config)
    } else if path.exists() {
        owned = read_hooks_config(path)?;
        Some(&owned)
    } else {
        None
    };
    let mut managed_commands = Vec::new();
    let mut group_count = 0;
    if let Some(config) = config
        && let Some(groups) = config
            .get("hooks")
            .and_then(|hooks| hooks.get(POST_TOOL_USE))
            .and_then(Value::as_array)
    {
        group_count = groups.len();
        for group in groups {
            let Some(hooks) = group
                .as_object()
                .and_then(|group| group.get("hooks"))
                .and_then(Value::as_array)
            else {
                continue;
            };
            for hook in hooks {
                if is_managed_hook(hook)
                    && let Some(command) = hook.get("command").and_then(Value::as_str)
                {
                    managed_commands.push(command.to_owned());
                }
            }
        }
    }

    Ok(CodexHookStatusOutput {
        path: path.to_path_buf(),
        installed: !managed_commands.is_empty(),
        matcher: BASH_MATCHER.to_owned(),
        managed_hook_count: managed_commands.len(),
        post_tool_use_group_count: group_count,
        managed_commands,
    })
}

fn managed_hook_specs(base: &str, record_signals: bool) -> Vec<ManagedHookSpec> {
    vec![
        ManagedHookSpec {
            command: managed_handler_command(base, "failure", record_signals),
            status_message: FAILURE_STATUS_MESSAGE,
        },
        ManagedHookSpec {
            command: managed_handler_command(base, "task-create", false),
            status_message: TASK_CREATE_STATUS_MESSAGE,
        },
    ]
}

fn managed_handler_command(base: &str, handler: &str, record_signals: bool) -> String {
    let mut command = base.trim().to_owned();
    if command.is_empty() {
        command.push_str("kanban hook codex handle");
    }
    command.push(' ');
    command.push_str(handler);
    if !command.contains("--installed-by") {
        command.push_str(" --installed-by ");
        command.push_str(MANAGED_MARKER);
    }
    if record_signals && !command.contains("--record-signals") {
        command.push_str(" --record-signals");
    }
    command
}

fn parse_hook_payload(raw: &str) -> Option<Value> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

fn normalize_post_tool_use(payload: &Value) -> NormalizedPostToolUse {
    let tool_name = string_field(payload, &["tool_name", "toolName"]);
    let command = command_from_payload(payload).unwrap_or_default();
    let response = response_from_payload(payload);
    NormalizedPostToolUse {
        tool_name: tool_name.clone().unwrap_or_default(),
        is_bash: tool_name.as_deref() == Some("Bash"),
        tool_use_id: string_field(payload, &["tool_use_id", "toolUseId"]),
        command,
        exit_code: integer_field(&response, &["exit_code", "exitCode", "status"]),
        success: bool_field(&response, &["success", "ok"]),
        stdout: string_field(&response, &["stdout", "output"]).unwrap_or_default(),
        stderr: string_field(&response, &["stderr", "error"]).unwrap_or_default(),
    }
}

fn command_from_payload(payload: &Value) -> Option<String> {
    let input = object_or_json_string(
        payload
            .get("tool_input")
            .or_else(|| payload.get("toolInput")),
    )?;
    string_field(&input, &["command"])
}

fn response_from_payload(payload: &Value) -> Value {
    object_or_json_string(
        payload
            .get("tool_response")
            .or_else(|| payload.get("toolResponse")),
    )
    .unwrap_or_else(|| Value::Object(Map::new()))
}

fn object_or_json_string(value: Option<&Value>) -> Option<Value> {
    match value? {
        Value::Object(_) => value.cloned(),
        Value::String(raw) => serde_json::from_str(raw)
            .ok()
            .or_else(|| Some(json!({ "stdout": raw }))),
        _ => None,
    }
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(raw) = value.get(*key).and_then(Value::as_str) {
            return Some(raw.to_owned());
        }
    }
    None
}

fn integer_field(value: &Value, keys: &[&str]) -> Option<i64> {
    for key in keys {
        let Some(raw) = value.get(*key) else {
            continue;
        };
        if let Some(value) = raw.as_i64() {
            return Some(value);
        }
        if let Some(value) = raw.as_str().and_then(|value| value.parse::<i64>().ok()) {
            return Some(value);
        }
    }
    None
}

fn bool_field(value: &Value, keys: &[&str]) -> Option<bool> {
    for key in keys {
        let Some(raw) = value.get(*key) else {
            continue;
        };
        if let Some(value) = raw.as_bool() {
            return Some(value);
        }
        if let Some(value) = raw.as_str().and_then(|value| match value {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }) {
            return Some(value);
        }
    }
    None
}

fn is_kanban_command(command: &str) -> bool {
    let tokens = command_tokens(command);
    kanban_token_index(&tokens).is_some()
}

fn is_task_create_command(command: &str) -> bool {
    let tokens = command_tokens(command);
    let Some(kanban_index) = kanban_token_index(&tokens) else {
        return false;
    };
    tokens[kanban_index + 1..]
        .windows(2)
        .any(|window| window[0] == "task" && window[1] == "create")
}

fn kanban_token_index(tokens: &[String]) -> Option<usize> {
    let mut at_simple_command_start = true;
    for (index, token) in tokens.iter().enumerate() {
        if is_shell_separator(token) {
            at_simple_command_start = true;
            continue;
        }
        if !at_simple_command_start {
            continue;
        }
        if token.is_empty() || token.contains('=') && !token.starts_with('-') {
            continue;
        }
        if matches!(token.as_str(), "env" | "sudo" | "command") {
            continue;
        }
        if token == "kanban" {
            return Some(index);
        }
        at_simple_command_start = false;
    }
    None
}

fn is_shell_separator(token: &str) -> bool {
    matches!(token, "&&" | "||" | ";" | "|")
}

fn command_tokens(command: &str) -> Vec<String> {
    let normalized = command
        .replace("&&", " && ")
        .replace("||", " || ")
        .replace(';', " ; ")
        .replace('|', " | ");
    normalized
        .split_whitespace()
        .map(|token| {
            token
                .trim_matches(|value| matches!(value, '\'' | '"' | '(' | ')'))
                .rsplit('/')
                .next()
                .unwrap_or(token)
                .to_owned()
        })
        .collect()
}

fn record_failure_signal(
    db_path: &PathBuf,
    board_arg: Option<&str>,
    actor: &str,
    payload: &Value,
    normalized: &NormalizedPostToolUse,
) -> RecordSignalResult {
    let board = match active_board(board_arg) {
        Ok(board) => board,
        Err(error) => {
            return RecordSignalResult::Failed(format!("failed to resolve active board: {error}"));
        }
    };
    let input = SignalRecordInput {
        kind: "agent_cli_failure".to_owned(),
        title: "kanban CLI command failed".to_owned(),
        summary: format!(
            "Codex PostToolUse observed a failing kanban command{}.",
            normalized
                .exit_code
                .map(|code| format!(" (exit {code})"))
                .unwrap_or_default()
        ),
        severity: Some("medium".to_owned()),
        task_ref: None,
        task_id: None,
        run_id: None,
        comment_id: None,
        actor: Some(actor.to_owned()),
        agent_type: Some("codex".to_owned()),
        dedupe_key: Some(dedupe_key(&normalized.command, normalized.exit_code)),
        source: Some("kanban-hook-codex".to_owned()),
        evidence: Some(json!({
            "hook_event_name": string_field(payload, &["hook_event_name", "hookEventName", "event", "name"]),
            "tool_name": normalized.tool_name,
            "tool_use_id": normalized.tool_use_id,
            "command": normalized.command,
            "exit_code": normalized.exit_code,
            "stderr": truncate(&normalized.stderr, 4000),
            "stdout": truncate(&normalized.stdout, 4000),
        })),
        comment: None,
    };
    match record_signal(db_path, &board, actor, input) {
        Ok(result) => RecordSignalResult::Recorded(result.signal.id),
        Err(error) => RecordSignalResult::Failed(error.to_string()),
    }
}

fn failure_system_message(
    normalized: &NormalizedPostToolUse,
    record_result: Option<&RecordSignalResult>,
) -> String {
    let exit = normalized
        .exit_code
        .map(|code| format!("exit {code}"))
        .unwrap_or_else(|| "non-zero result".to_owned());
    let mut lines = vec![format!(
        "kanban CLI command failed ({exit}): `{}`.",
        truncate(&normalized.command, 240)
    )];
    if !normalized.stderr.trim().is_empty() {
        lines.push(format!(
            "stderr: {}",
            truncate(normalized.stderr.trim(), 900)
        ));
    } else if !normalized.stdout.trim().is_empty() {
        lines.push(format!(
            "stdout: {}",
            truncate(normalized.stdout.trim(), 900)
        ));
    }
    match record_result {
        Some(RecordSignalResult::Recorded(signal_id)) => {
            lines.push(format!("Recorded generic signal `{signal_id}` for this trace."));
        }
        Some(RecordSignalResult::Failed(error)) => {
            lines.push(format!(
                "Tried to record a generic signal but it failed: {}.",
                truncate(error, 240)
            ));
        }
        None => lines.push(
            "If this affects task state, record durable trace with `kanban signal record --input - --json`; if root cause is unclear, ask Codex to spawn a native debugger agent for this failure."
                .to_owned(),
        ),
    }
    lines.join("\n")
}

fn task_create_system_message(normalized: &NormalizedPostToolUse) -> String {
    let task_ref = task_ref_from_create_stdout(&normalized.stdout)
        .map(|value| format!(" for `{value}`"))
        .unwrap_or_default();
    format!(
        "kanban task create succeeded{task_ref}. Consider a label/signal follow-up before execution: run `kanban label suggest <task_ref> --json` when labels matter, and record durable disagreement or CLI trace with `kanban signal record --input - --json`. Do not write label ontology automatically unless you have a complete suggestion snapshot and explicit decision payload."
    )
}

fn task_ref_from_create_stdout(stdout: &str) -> Option<String> {
    let value: Value = serde_json::from_str(stdout.trim()).ok()?;
    value
        .pointer("/data/ref")
        .and_then(Value::as_str)
        .or_else(|| value.pointer("/data/task/ref").and_then(Value::as_str))
        .map(str::to_owned)
}

fn dedupe_key(command: &str, exit_code: Option<i64>) -> String {
    let mut hasher = DefaultHasher::new();
    command.hash(&mut hasher);
    exit_code.hash(&mut hasher);
    format!("kanban-hook-codex:{:x}", hasher.finish())
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut truncated = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        truncated.push_str("...");
    }
    truncated
}

fn write_hook_response(response: &HookResponse) -> Result<()> {
    println!("{}", serde_json::to_string(response)?);
    Ok(())
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[derive(Debug, Serialize)]
struct CodexHookInstallOutput {
    path: PathBuf,
    installed: bool,
    matcher: String,
    handler_commands: Vec<String>,
    managed_hook_count: usize,
}

#[derive(Debug, Serialize)]
struct CodexHookStatusOutput {
    path: PathBuf,
    installed: bool,
    matcher: String,
    managed_hook_count: usize,
    post_tool_use_group_count: usize,
    managed_commands: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CodexHookUninstallOutput {
    path: PathBuf,
    removed_hook_count: usize,
    installed: bool,
}

#[derive(Debug)]
struct ManagedHookSpec {
    command: String,
    status_message: &'static str,
}

#[derive(Debug, Clone, Copy)]
enum CodexHookPostHandler {
    Failure { record_signals: bool },
    TaskCreate,
}

#[derive(Debug)]
struct NormalizedPostToolUse {
    tool_name: String,
    is_bash: bool,
    tool_use_id: Option<String>,
    command: String,
    exit_code: Option<i64>,
    success: Option<bool>,
    stdout: String,
    stderr: String,
}

impl NormalizedPostToolUse {
    fn failed(&self) -> bool {
        self.exit_code.is_some_and(|code| code != 0) || self.success == Some(false)
    }
}

#[derive(Debug)]
enum RecordSignalResult {
    Recorded(String),
    Failed(String),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HookResponse {
    system_message: String,
}
