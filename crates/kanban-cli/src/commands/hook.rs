//! Codex hook adapter。
//!
//! Hook 管理只读写用户明确指定的 `CODEX_HOME/hooks.json` 和 XDG prompt 文件；处理器只
//! 解析 stdin/stdout 协议，不打开数据库。若将来需要记录 signal，应通过 localhost client
//! 增加独立 operation，不能在这里恢复直接 Turso 路径。

use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    io::{self, Read},
    path::{Path, PathBuf},
};

use clap::{Args, Subcommand};
use kanban_protocol::cli_operator::{
    CliHookCodexInstallOutput, CliHookCodexInstallResult, CliHookCodexStatusOutput,
    CliHookCodexStatusResult, CliHookCodexUninstallOutput, CliHookCodexUninstallResult,
    CliHookPromptBindings, CliHookPromptConfigStatus,
};
use serde_json::{Map, Value, json};

use crate::{config, error::CliFailure, output};

const MANAGED_MARKER: &str = "kanban-hook-codex";
const POST_TOOL_USE: &str = "PostToolUse";
const BASH_MATCHER: &str = "^Bash$";
const FAILURE_STATUS_MESSAGE: &str = "检查 kanban CLI 失败 trace";
const TASK_CREATE_STATUS_MESSAGE: &str = "检查 kanban task 创建 trace";
const DEFAULT_FAILURE_PROMPT_ALIAS: &str = "failure.zh-default";
const DEFAULT_TASK_CREATE_PROMPT_ALIAS: &str = "task_create.zh-default";
const DEFAULT_FAILURE_PROMPT_TEMPLATE: &str = "检测到 kanban CLI 命令失败。\n\n命令：{{command}}\n退出码：{{exit_code}}\n\n继续调整。修正后继续当前任务，并在确有必要时记录后续工作。";
const DEFAULT_TASK_CREATE_PROMPT_TEMPLATE: &str = "检测到 kanban task 创建成功。\n\n命令：{{command}}\n任务：{{task_ref}}\n\n请考虑为该 task 执行 label/signal follow-up；需要标签时先运行 `kanban label suggest {{task_ref}} --json`。不要自动写 label ontology，除非已有完整 suggestion snapshot 和明确 decision payload。";

#[derive(Debug, Subcommand)]
pub(crate) enum HookCommand {
    /// 管理 Codex 生命周期 hooks。
    Codex {
        #[command(subcommand)]
        command: CodexHookCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum CodexHookCommand {
    /// 安装由 fingerprint 保护的 managed hooks。
    Install(CodexHookInstallArgs),
    /// 查看 managed hooks 和 prompt 配置状态。
    Status,
    /// 仅移除 fingerprint 匹配的 managed hooks。
    Uninstall,
    /// 处理 Codex stdin hook payload。
    Handle {
        #[command(subcommand)]
        command: CodexHookHandleCommand,
    },
}

#[derive(Debug, Args)]
pub(crate) struct CodexHookInstallArgs {
    #[arg(
        long,
        value_name = "command-prefix",
        default_value = "kanban hook codex handle"
    )]
    pub(crate) handler_command: String,
    #[arg(long, default_value_t = 30)]
    pub(crate) timeout: u64,
    #[arg(long)]
    pub(crate) record_signals: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum CodexHookHandleCommand {
    /// 处理失败的 kanban Bash command。
    Failure(CodexHookFailureHandleArgs),
    /// 处理成功的 kanban task create command。
    #[command(name = "task-create")]
    TaskCreate(CodexHookTaskCreateHandleArgs),
}

#[derive(Debug, Args)]
pub(crate) struct CodexHookFailureHandleArgs {
    #[arg(long, hide = true)]
    pub(crate) installed_by: Option<String>,
    #[arg(long)]
    pub(crate) record_signals: bool,
}

#[derive(Debug, Args)]
pub(crate) struct CodexHookTaskCreateHandleArgs {
    #[arg(long, hide = true)]
    pub(crate) installed_by: Option<String>,
}

pub(crate) fn run(command: &HookCommand, json_output: bool) -> Result<(), CliFailure> {
    match command {
        HookCommand::Codex { command } => match command {
            CodexHookCommand::Install(args) => install(args, json_output),
            CodexHookCommand::Status => status(json_output),
            CodexHookCommand::Uninstall => uninstall(json_output),
            CodexHookCommand::Handle { command } => match command {
                CodexHookHandleCommand::Failure(args) => {
                    let _ = (&args.installed_by, args.record_signals);
                    handle_payload(HandlerKind::Failure)
                }
                CodexHookHandleCommand::TaskCreate(args) => {
                    let _ = &args.installed_by;
                    handle_payload(HandlerKind::TaskCreate)
                }
            },
        },
    }
}

fn install(args: &CodexHookInstallArgs, json_output: bool) -> Result<(), CliFailure> {
    let path = hooks_config_path()?;
    let mut value = read_hooks_config(&path)?;
    remove_managed_hooks(&mut value)?;
    let managed = managed_hook_specs(&args.handler_command, args.record_signals);
    install_managed_hooks(&mut value, &managed, args.timeout)?;
    write_hooks_config(&path, &value)?;

    let prompt_path = config::prompt_config_path();
    let prompt_config_created = ensure_prompt_config_exists(&prompt_path)?;
    let status = inspect_hooks_config(&path, Some(&value))?;
    let result = CliHookCodexInstallResult {
        path,
        installed: status.installed,
        matcher: BASH_MATCHER.to_owned(),
        handler_commands: managed.into_iter().map(|hook| hook.command).collect(),
        managed_hook_count: status.managed_hook_count,
        prompt_config_created,
        prompt_config: inspect_prompt_config(&prompt_path),
    };
    let envelope = CliHookCodexInstallOutput::new(result);
    if json_output {
        output::print_json(&envelope);
    } else {
        println!(
            "已安装 {} 个 managed kanban Codex hooks：{}\nprompt 配置：{}",
            envelope.data.managed_hook_count,
            envelope.data.path.display(),
            envelope.data.prompt_config.path.display()
        );
    }
    Ok(())
}

fn status(json_output: bool) -> Result<(), CliFailure> {
    let path = hooks_config_path()?;
    let result = inspect_hooks_config(&path, None)?;
    let envelope = CliHookCodexStatusOutput::new(result);
    if json_output {
        output::print_json(&envelope);
    } else if envelope.data.installed {
        println!(
            "kanban Codex hook 已安装：{}（{} 个 managed hook）\nprompt 配置：{}",
            envelope.data.path.display(),
            envelope.data.managed_hook_count,
            envelope.data.prompt_config.path.display()
        );
    } else {
        println!(
            "kanban Codex hook 未安装：{}\nprompt 配置：{}",
            envelope.data.path.display(),
            envelope.data.prompt_config.path.display()
        );
    }
    Ok(())
}

fn uninstall(json_output: bool) -> Result<(), CliFailure> {
    let path = hooks_config_path()?;
    let removed = if path.exists() {
        let mut value = read_hooks_config(&path)?;
        let removed = remove_managed_hooks(&mut value)?;
        if removed > 0 {
            write_hooks_config(&path, &value)?;
        }
        removed
    } else {
        0
    };
    let envelope = CliHookCodexUninstallOutput::new(CliHookCodexUninstallResult {
        path,
        removed_hook_count: removed,
        installed: false,
    });
    if json_output {
        output::print_json(&envelope);
    } else {
        println!(
            "已移除 {} 个 managed kanban Codex hook{}：{}",
            removed,
            if removed == 1 { "" } else { "s" },
            envelope.data.path.display()
        );
    }
    Ok(())
}

fn handle_payload(kind: HandlerKind) -> Result<(), CliFailure> {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .map_err(|error| io_failure("读取 Codex hook payload 失败", error))?;
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
    match kind {
        HandlerKind::Failure if normalized.failed() => write_hook_response(&HookResponse {
            system_message: failure_system_message(&normalized),
        }),
        HandlerKind::TaskCreate
            if !normalized.failed() && is_task_create_command(&normalized.command) =>
        {
            write_hook_response(&HookResponse {
                system_message: task_create_system_message(&normalized),
            })
        }
        _ => Ok(()),
    }
}

fn hooks_config_path() -> Result<PathBuf, CliFailure> {
    if let Some(home) = env::var_os("CODEX_HOME") {
        return Ok(PathBuf::from(home).join("hooks.json"));
    }
    let home = env::var_os("HOME").ok_or_else(|| CliFailure {
        code: "invalid_input",
        message: "未设置 HOME；请设置 CODEX_HOME 以管理 Codex hook".to_owned(),
        exit_code: 2,
    })?;
    Ok(PathBuf::from(home).join(".codex/hooks.json"))
}

fn read_hooks_config(path: &Path) -> Result<Value, CliFailure> {
    if !path.exists() {
        return Ok(json!({ "hooks": {} }));
    }
    let text =
        fs::read_to_string(path).map_err(|error| io_failure("读取 hooks 配置失败", error))?;
    if text.trim().is_empty() {
        return Ok(json!({ "hooks": {} }));
    }
    let value: Value = serde_json::from_str(&text).map_err(|error| CliFailure {
        code: "invalid_input",
        message: format!("解析 {} 的 JSON 失败：{error}", path.display()),
        exit_code: 2,
    })?;
    if !value.is_object() {
        return Err(invalid(format!("{} 必须包含 JSON 对象", path.display())));
    }
    Ok(value)
}

fn write_hooks_config(path: &Path, value: &Value) -> Result<(), CliFailure> {
    let text = serde_json::to_vec_pretty(value).map_err(|error| CliFailure {
        code: "generic_error",
        message: error.to_string(),
        exit_code: 1,
    })?;
    let mut content = text;
    content.push(b'\n');
    config::atomic_write(path, &content)
        .map(|_| ())
        .map_err(|error| io_failure("写入 hooks 配置失败", error))
}

fn install_managed_hooks(
    value: &mut Value,
    managed: &[ManagedHookSpec],
    timeout: u64,
) -> Result<(), CliFailure> {
    let hooks = hooks_object_mut(value)?;
    let post = hooks
        .entry(POST_TOOL_USE)
        .or_insert_with(|| Value::Array(Vec::new()));
    let groups = post
        .as_array_mut()
        .ok_or_else(|| invalid("hooks.PostToolUse 必须是数组"))?;
    let hook_values = managed
        .iter()
        .map(|hook| {
            json!({
                "type": "command",
                "command": hook.command,
                "timeout": timeout,
                "statusMessage": hook.status_message,
                "kanbanManaged": MANAGED_MARKER,
                "kanbanFingerprint": hook.fingerprint,
            })
        })
        .collect::<Vec<_>>();
    groups.push(json!({ "matcher": BASH_MATCHER, "hooks": hook_values }));
    Ok(())
}

fn remove_managed_hooks(value: &mut Value) -> Result<usize, CliFailure> {
    let hooks = hooks_object_mut(value)?;
    let Some(groups) = hooks.get_mut(POST_TOOL_USE).and_then(Value::as_array_mut) else {
        return Ok(0);
    };
    let mut removed = 0;
    let mut remove_empty_groups = vec![false; groups.len()];
    for (index, group) in groups.iter_mut().enumerate() {
        let Some(group_hooks) = group
            .as_object_mut()
            .and_then(|group| group.get_mut("hooks"))
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        let before = group_hooks.len();
        group_hooks.retain(|hook| !is_managed_hook(hook));
        let removed_here = before.saturating_sub(group_hooks.len());
        removed += removed_here;
        remove_empty_groups[index] = removed_here > 0 && group_hooks.is_empty();
    }
    let mut index = 0;
    groups.retain(|_| {
        let remove = remove_empty_groups[index];
        index += 1;
        !remove
    });
    Ok(removed)
}

fn hooks_object_mut(value: &mut Value) -> Result<&mut Map<String, Value>, CliFailure> {
    let root = value
        .as_object_mut()
        .ok_or_else(|| invalid("hooks 配置根必须是 JSON 对象"))?;
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    hooks
        .as_object_mut()
        .ok_or_else(|| invalid("hooks 配置字段 `hooks` 必须是 JSON 对象"))
}

fn is_managed_hook(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let Some(command) = object.get("command").and_then(Value::as_str) else {
        return false;
    };
    object.get("type").and_then(Value::as_str) == Some("command")
        && object.get("kanbanManaged").and_then(Value::as_str) == Some(MANAGED_MARKER)
        && object.get("kanbanFingerprint").and_then(Value::as_str)
            == Some(hook_fingerprint(command).as_str())
}

fn inspect_hooks_config(
    path: &Path,
    provided: Option<&Value>,
) -> Result<CliHookCodexStatusResult, CliFailure> {
    let owned;
    let value = if let Some(value) = provided {
        Some(value)
    } else if path.exists() {
        owned = read_hooks_config(path)?;
        Some(&owned)
    } else {
        None
    };
    let mut commands = Vec::new();
    let mut groups_count = 0;
    if let Some(value) = value
        && let Some(groups) = value
            .get("hooks")
            .and_then(|hooks| hooks.get(POST_TOOL_USE))
            .and_then(Value::as_array)
    {
        groups_count = groups.len();
        for group in groups {
            if let Some(hooks) = group.get("hooks").and_then(Value::as_array) {
                for hook in hooks {
                    if is_managed_hook(hook)
                        && let Some(command) = hook.get("command").and_then(Value::as_str)
                    {
                        commands.push(command.to_owned());
                    }
                }
            }
        }
    }
    Ok(CliHookCodexStatusResult {
        path: path.to_path_buf(),
        installed: !commands.is_empty(),
        matcher: BASH_MATCHER.to_owned(),
        managed_hook_count: commands.len(),
        post_tool_use_group_count: groups_count,
        managed_commands: commands,
        prompt_config: inspect_prompt_config(&config::prompt_config_path()),
    })
}

fn managed_hook_specs(base: &str, record_signals: bool) -> Vec<ManagedHookSpec> {
    vec![
        ManagedHookSpec {
            command: managed_handler_command(base, "failure", record_signals),
            status_message: FAILURE_STATUS_MESSAGE,
            fingerprint: String::new(),
        },
        ManagedHookSpec {
            command: managed_handler_command(base, "task-create", false),
            status_message: TASK_CREATE_STATUS_MESSAGE,
            fingerprint: String::new(),
        },
    ]
    .into_iter()
    .map(|mut hook| {
        hook.fingerprint = hook_fingerprint(&hook.command);
        hook
    })
    .collect()
}

fn managed_handler_command(base: &str, handler: &str, record_signals: bool) -> String {
    let mut command = base.trim().to_owned();
    if command.is_empty() {
        command = "kanban hook codex handle".to_owned();
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

fn hook_fingerprint(command: &str) -> String {
    let mut hasher = DefaultHasher::new();
    MANAGED_MARKER.hash(&mut hasher);
    command.hash(&mut hasher);
    format!("v1-{:#x}", hasher.finish())
}

fn ensure_prompt_config_exists(path: &Path) -> Result<bool, CliFailure> {
    if path.exists() {
        return Ok(false);
    }
    let content =
        serde_json::to_vec_pretty(&default_prompt_config()).map_err(|error| CliFailure {
            code: "generic_error",
            message: error.to_string(),
            exit_code: 1,
        })?;
    let mut content = content;
    content.push(b'\n');
    config::atomic_write(path, &content)
        .map_err(|error| io_failure("写入 Codex prompt 配置失败", error))
}

fn default_prompt_config() -> Value {
    json!({
        "version": 1,
        "codex_hooks": {
            "bindings": {
                "failure": DEFAULT_FAILURE_PROMPT_ALIAS,
                "task_create": DEFAULT_TASK_CREATE_PROMPT_ALIAS
            },
            "prompts": {
                "failure.zh-default": DEFAULT_FAILURE_PROMPT_TEMPLATE,
                "task_create.zh-default": DEFAULT_TASK_CREATE_PROMPT_TEMPLATE
            }
        }
    })
}

fn inspect_prompt_config(path: &Path) -> CliHookPromptConfigStatus {
    if !path.exists() {
        return CliHookPromptConfigStatus {
            path: path.to_path_buf(),
            exists: false,
            valid: false,
            error: None,
            bindings: default_prompt_bindings(),
        };
    }
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            return CliHookPromptConfigStatus {
                path: path.to_path_buf(),
                exists: true,
                valid: false,
                error: Some(error.to_string()),
                bindings: default_prompt_bindings(),
            };
        }
    };
    let value: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(error) => {
            return CliHookPromptConfigStatus {
                path: path.to_path_buf(),
                exists: true,
                valid: false,
                error: Some(error.to_string()),
                bindings: default_prompt_bindings(),
            };
        }
    };
    match prompt_bindings_from_config(&value)
        .and_then(|bindings| validate_prompt_templates(&value, &bindings).map(|()| bindings))
    {
        Ok(bindings) => CliHookPromptConfigStatus {
            path: path.to_path_buf(),
            exists: true,
            valid: true,
            error: None,
            bindings,
        },
        Err(error) => CliHookPromptConfigStatus {
            path: path.to_path_buf(),
            exists: true,
            valid: false,
            error: Some(error),
            bindings: default_prompt_bindings(),
        },
    }
}

fn prompt_bindings_from_config(value: &Value) -> Result<CliHookPromptBindings, String> {
    if value.get("version").and_then(Value::as_i64) != Some(1) {
        return Err("codex hook prompt 配置版本必须为 1".to_owned());
    }
    let hooks = value
        .get("codex_hooks")
        .and_then(Value::as_object)
        .ok_or_else(|| "codex_hooks 必须是对象".to_owned())?;
    let alias = |key: &str, default: &str| {
        let alias = hooks
            .get("bindings")
            .and_then(|bindings| bindings.get(key))
            .and_then(Value::as_str)
            .unwrap_or(default);
        if alias.trim().is_empty() {
            Err(format!("codex_hooks.bindings.{key} 不能为空"))
        } else {
            Ok(alias.to_owned())
        }
    };
    Ok(CliHookPromptBindings {
        failure: alias("failure", DEFAULT_FAILURE_PROMPT_ALIAS)?,
        task_create: alias("task_create", DEFAULT_TASK_CREATE_PROMPT_ALIAS)?,
    })
}

fn validate_prompt_templates(
    value: &Value,
    bindings: &CliHookPromptBindings,
) -> Result<(), String> {
    for alias in [&bindings.failure, &bindings.task_create] {
        let template = value
            .get("codex_hooks")
            .and_then(|hooks| hooks.get("prompts"))
            .and_then(|prompts| prompts.get(alias))
            .and_then(Value::as_str)
            .ok_or_else(|| format!("codex_hooks.prompts.{alias} 必须是字符串"))?;
        if template.trim().is_empty() {
            return Err(format!("codex_hooks.prompts.{alias} 不能为空"));
        }
    }
    Ok(())
}

fn default_prompt_bindings() -> CliHookPromptBindings {
    CliHookPromptBindings {
        failure: DEFAULT_FAILURE_PROMPT_ALIAS.to_owned(),
        task_create: DEFAULT_TASK_CREATE_PROMPT_ALIAS.to_owned(),
    }
}

fn parse_hook_payload(raw: &str) -> Option<Value> {
    let raw = raw.trim();
    (!raw.is_empty())
        .then(|| serde_json::from_str(raw).ok())
        .flatten()
}

fn normalize_post_tool_use(payload: &Value) -> NormalizedPostToolUse {
    let tool_name = string_field(payload, &["tool_name", "toolName"]);
    let command = command_from_payload(payload).unwrap_or_default();
    let response = response_from_payload(payload);
    NormalizedPostToolUse {
        is_bash: tool_name.as_deref() == Some("Bash"),
        command,
        exit_code: integer_field(&response, &["exit_code", "exitCode", "status"]),
        success: bool_field(&response, &["success", "ok"]),
        stdout: string_field(&response, &["stdout", "output"]).unwrap_or_default(),
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
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str).map(str::to_owned))
}

fn integer_field(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|raw| {
            raw.as_i64()
                .or_else(|| raw.as_str().and_then(|text| text.parse::<i64>().ok()))
        })
    })
}

fn bool_field(value: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|raw| {
            raw.as_bool().or_else(|| match raw.as_str() {
                Some("true") => Some(true),
                Some("false") => Some(false),
                _ => None,
            })
        })
    })
}

fn is_kanban_command(command: &str) -> bool {
    kanban_token_index(&command_tokens(command)).is_some()
}

fn is_task_create_command(command: &str) -> bool {
    let tokens = command_tokens(command);
    let Some(index) = kanban_token_index(&tokens) else {
        return false;
    };
    tokens[index + 1..]
        .windows(2)
        .any(|window| window[0] == "task" && window[1] == "create")
}

fn kanban_token_index(tokens: &[String]) -> Option<usize> {
    let mut command_start = true;
    for (index, token) in tokens.iter().enumerate() {
        if matches!(token.as_str(), "&&" | "||" | ";" | "|") {
            command_start = true;
            continue;
        }
        if !command_start {
            continue;
        }
        if token.is_empty() || (token.contains('=') && !token.starts_with('-')) {
            continue;
        }
        if matches!(token.as_str(), "env" | "sudo" | "command") {
            continue;
        }
        if token == "kanban" {
            return Some(index);
        }
        command_start = false;
    }
    None
}

fn command_tokens(command: &str) -> Vec<String> {
    command
        .replace("&&", " && ")
        .replace("||", " || ")
        .replace(';', " ; ")
        .replace('|', " | ")
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

fn failure_system_message(normalized: &NormalizedPostToolUse) -> String {
    let command = truncate(&normalized.command, 240);
    let exit_code = normalized
        .exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "未知".to_owned());
    render_hook_prompt(
        PromptKind::Failure,
        &[("command", &command), ("exit_code", &exit_code)],
    )
}

fn task_create_system_message(normalized: &NormalizedPostToolUse) -> String {
    let task_ref =
        task_ref_from_create_stdout(&normalized.stdout).unwrap_or_else(|| "<task_ref>".to_owned());
    let command = truncate(&normalized.command, 240);
    render_hook_prompt(
        PromptKind::TaskCreate,
        &[("command", &command), ("task_ref", &task_ref)],
    )
}

fn render_hook_prompt(kind: PromptKind, variables: &[(&str, &str)]) -> String {
    let (template, warning) = load_hook_prompt_template(kind);
    let mut rendered = template;
    for (name, value) in variables {
        rendered = rendered.replace(&format!("{{{{{name}}}}}"), value);
    }
    if let Some(warning) = warning {
        rendered.push_str(&format!(
            "\n\n提示：Codex hook prompt 配置不可用，已使用内置默认提示。原因：{}。",
            truncate(&warning, 240)
        ));
    }
    rendered
}

fn load_hook_prompt_template(kind: PromptKind) -> (String, Option<String>) {
    let path = config::prompt_config_path();
    if !path.exists() {
        return (kind.default_template().to_owned(), None);
    }
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => return (kind.default_template().to_owned(), Some(error.to_string())),
    };
    let value: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(error) => return (kind.default_template().to_owned(), Some(error.to_string())),
    };
    let binding = prompt_bindings_from_config(&value).and_then(|bindings| {
        let alias = match kind {
            PromptKind::Failure => bindings.failure,
            PromptKind::TaskCreate => bindings.task_create,
        };
        value
            .get("codex_hooks")
            .and_then(|hooks| hooks.get("prompts"))
            .and_then(|prompts| prompts.get(&alias))
            .and_then(Value::as_str)
            .filter(|template| !template.trim().is_empty())
            .map(str::to_owned)
            .ok_or_else(|| format!("codex_hooks.prompts.{alias} 必须是字符串"))
    });
    match binding {
        Ok(template) => (template, None),
        Err(error) => (kind.default_template().to_owned(), Some(error)),
    }
}

fn task_ref_from_create_stdout(stdout: &str) -> Option<String> {
    let value: Value = serde_json::from_str(stdout.trim()).ok()?;
    value
        .pointer("/data/ref")
        .and_then(Value::as_str)
        .or_else(|| value.pointer("/data/task/ref").and_then(Value::as_str))
        .map(str::to_owned)
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut truncated = String::new();
    for _ in 0..max_chars {
        let Some(character) = chars.next() else {
            return truncated;
        };
        truncated.push(character);
    }
    if chars.next().is_some() {
        truncated.push_str("...");
    }
    truncated
}

fn write_hook_response(response: &HookResponse) -> Result<(), CliFailure> {
    output::print_json(response);
    Ok(())
}

fn invalid(message: impl Into<String>) -> CliFailure {
    CliFailure {
        code: "invalid_input",
        message: message.into(),
        exit_code: 2,
    }
}

fn io_failure(context: &str, error: io::Error) -> CliFailure {
    CliFailure {
        code: "generic_error",
        message: format!("{context}: {error}"),
        exit_code: 1,
    }
}

#[derive(Debug)]
struct ManagedHookSpec {
    command: String,
    status_message: &'static str,
    fingerprint: String,
}

#[derive(Debug, Clone, Copy)]
enum HandlerKind {
    Failure,
    TaskCreate,
}

#[derive(Debug, Clone, Copy)]
enum PromptKind {
    Failure,
    TaskCreate,
}

impl PromptKind {
    fn default_template(self) -> &'static str {
        match self {
            Self::Failure => DEFAULT_FAILURE_PROMPT_TEMPLATE,
            Self::TaskCreate => DEFAULT_TASK_CREATE_PROMPT_TEMPLATE,
        }
    }
}

#[derive(Debug)]
struct NormalizedPostToolUse {
    is_bash: bool,
    command: String,
    exit_code: Option<i64>,
    success: Option<bool>,
    stdout: String,
}

impl NormalizedPostToolUse {
    fn failed(&self) -> bool {
        self.exit_code.is_some_and(|code| code != 0) || self.success == Some(false)
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct HookResponse {
    system_message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uninstall_matches_only_our_fingerprint() {
        let managed = managed_hook_specs("kanban hook codex handle", true);
        let owned = json!({
            "type": "command",
            "command": managed[0].command.clone(),
            "kanbanManaged": MANAGED_MARKER,
            "kanbanFingerprint": managed[0].fingerprint.clone(),
        });
        let mut tampered = owned.clone();
        tampered["kanbanFingerprint"] = json!("v1-tampered");
        assert!(is_managed_hook(&owned));
        assert!(!is_managed_hook(&tampered));

        let mut config = json!({
            "hooks": {"PostToolUse": [{"matcher": BASH_MATCHER, "hooks": [owned, tampered]}]}
        });
        assert_eq!(remove_managed_hooks(&mut config).unwrap(), 1);
        let remaining = config["hooks"][POST_TOOL_USE][0]["hooks"]
            .as_array()
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0]["kanbanFingerprint"], "v1-tampered");
    }

    #[test]
    fn command_detection_handles_shell_boundaries_without_matching_arguments() {
        assert!(is_kanban_command("cd /tmp && kanban task list"));
        assert!(is_kanban_command(
            "env FOO=bar sudo /usr/bin/kanban task list"
        ));
        assert!(is_task_create_command("kanban --json task create work"));
        assert!(!is_kanban_command("echo kanban task list"));
        assert!(!is_task_create_command("kanban task list"));
    }

    #[test]
    fn truncation_only_adds_marker_when_input_exceeds_limit() {
        assert_eq!(truncate("abc", 3), "abc");
        assert_eq!(truncate("abcd", 3), "abc...");
    }
}
