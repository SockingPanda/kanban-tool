mod args;
mod commands;
mod output;

use anyhow::Error;
use kanban_core::KanbanError;
use serde::Serialize;

fn main() {
    let cli = commands::app::parse_cli();
    let wants_json = cli.json;
    if let Err(error) = commands::app::run(cli) {
        let report = CliErrorReport::from_error(&error);
        if wants_json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({ "error": report }))
                    .unwrap_or_else(|_| "{\"error\":{\"code\":\"generic_error\",\"message\":\"failed to render error\",\"exit_code\":1}}".to_owned())
            );
        } else {
            eprintln!("Error: {}", report.human_message);
        }
        std::process::exit(report.exit_code);
    }
}

#[derive(Debug, Serialize)]
struct CliErrorReport {
    code: &'static str,
    message: String,
    exit_code: i32,
    #[serde(skip)]
    human_message: String,
}

impl CliErrorReport {
    fn from_error(error: &Error) -> Self {
        if let Some(kanban_error) = error
            .chain()
            .find_map(|cause| cause.downcast_ref::<KanbanError>())
        {
            let locale = kanban_core::current_locale();
            let message = kanban_core::i18n::render_error(locale, kanban_error);
            let (code, exit_code) = classify_kanban_error(kanban_error);
            let human_message = render_human_error(
                locale,
                &message,
                recovery_hint_for_kanban_error(locale, kanban_error),
            );
            return Self {
                code,
                message,
                exit_code,
                human_message,
            };
        }

        let message = format!("{error:?}");
        let (code, exit_code) = classify_error_message(&message).unwrap_or(("generic_error", 1));
        let human_message = message.clone();
        Self {
            code,
            message,
            exit_code,
            human_message,
        }
    }
}

fn classify_kanban_error(error: &KanbanError) -> (&'static str, i32) {
    match error {
        KanbanError::InvalidInput(message) | KanbanError::InvalidStatus(message) => {
            classify_error_message(message).unwrap_or(("invalid_input", 2))
        }
        KanbanError::NotFound(_) => ("not_found", 3),
        KanbanError::InvalidTransition(message)
            if message.contains("claim conflict") || message.contains("matching running claim") =>
        {
            ("claim_conflict", 5)
        }
        KanbanError::InvalidTransition(message) if message.contains("dependency blocked") => {
            ("dependency_blocked", 6)
        }
        KanbanError::InvalidTransition(_)
        | KanbanError::ExecutionPlanRequired(_)
        | KanbanError::StepsIncomplete(_) => ("invalid_transition", 4),
        KanbanError::Conflict(_) => ("claim_conflict", 5),
        KanbanError::Storage(message) => {
            classify_error_message(message).unwrap_or(("storage_error", 1))
        }
    }
}

fn classify_error_message(message: &str) -> Option<(&'static str, i32)> {
    let normalized = message.to_ascii_lowercase();
    if normalized.contains("database is locked")
        || normalized.contains("database is busy")
        || normalized.contains("sqlite busy")
        || normalized.contains("sqlite locked")
    {
        return Some(("sqlite_busy", 7));
    }
    if normalized.contains("integrity_check")
        || normalized.contains("integrity check failed")
        || normalized.contains("failed doctor checks")
    {
        return Some(("integrity_check_failed", 8));
    }
    // Only classify external or storage-layer text that cannot carry a structured
    // KanbanError variant through the command boundary. Business/config validation
    // must return KanbanError::InvalidInput at the command layer.
    None
}

fn render_human_error(
    locale: kanban_core::Locale,
    message: &str,
    hint: Option<&'static str>,
) -> String {
    let Some(hint) = hint else {
        return message.to_owned();
    };
    match locale {
        kanban_core::Locale::ZhCn => format!("{message}\n恢复建议：{hint}"),
        kanban_core::Locale::En => format!("{message}\nRecovery: {hint}"),
    }
}

fn recovery_hint_for_kanban_error(
    locale: kanban_core::Locale,
    error: &KanbanError,
) -> Option<&'static str> {
    match error {
        KanbanError::InvalidStatus(_) => Some(status_hint(locale)),
        KanbanError::InvalidInput(detail) => invalid_input_hint(locale, detail),
        KanbanError::NotFound(detail) => not_found_hint(locale, detail),
        _ => None,
    }
}

fn invalid_input_hint(locale: kanban_core::Locale, detail: &str) -> Option<&'static str> {
    if detail.starts_with("unsupported task list sort: ")
        || detail.starts_with("unsupported sort: ")
    {
        return Some(match locale {
            kanban_core::Locale::ZhCn => {
                "允许的 task list sort：seq, -seq, title, -title, status, -status, position, -position, priority, -priority, assignee, -assignee, scheduled_at, -scheduled_at, created_at, -created_at, updated_at, -updated_at, due_at, -due_at。"
            }
            kanban_core::Locale::En => {
                "Allowed task list sort values: seq, -seq, title, -title, status, -status, position, -position, priority, -priority, assignee, -assignee, scheduled_at, -scheduled_at, created_at, -created_at, updated_at, -updated_at, due_at, -due_at."
            }
        });
    }
    if detail.starts_with("unsupported export format: ") {
        return Some(match locale {
            kanban_core::Locale::ZhCn => "允许的 export formats：jsonl。",
            kanban_core::Locale::En => "Allowed export formats: jsonl.",
        });
    }
    if detail.starts_with("unsupported locale: ") {
        return Some(match locale {
            kanban_core::Locale::ZhCn => "允许的 locale：auto, zh-CN, en。",
            kanban_core::Locale::En => "Allowed locales: auto, zh-CN, en.",
        });
    }
    if detail.starts_with("invalid priority filter: ") {
        return Some(match locale {
            kanban_core::Locale::ZhCn => "允许的 priority：0, 1, 2, 3。",
            kanban_core::Locale::En => "Allowed priorities: 0, 1, 2, 3.",
        });
    }
    None
}

fn not_found_hint(locale: kanban_core::Locale, detail: &str) -> Option<&'static str> {
    let normalized = detail.to_ascii_lowercase();
    if normalized.contains("board") {
        return Some(match locale {
            kanban_core::Locale::ZhCn => {
                "运行 `kanban board list` 查看可用 board，或运行 `kanban board current` 查看当前 board。"
            }
            kanban_core::Locale::En => {
                "Run `kanban board list` to see available boards, or `kanban board current` to show the active board."
            }
        });
    }
    if normalized.contains("task") || normalized.contains("ref") {
        return Some(match locale {
            kanban_core::Locale::ZhCn => {
                "运行 `kanban task list` 查找任务 ref，或运行 `kanban task show <task-ref>` 查看任务详情。"
            }
            kanban_core::Locale::En => {
                "Run `kanban task list` to find task refs, or `kanban task show <task-ref>` to inspect a task."
            }
        });
    }
    None
}

fn status_hint(locale: kanban_core::Locale) -> &'static str {
    match locale {
        kanban_core::Locale::ZhCn => {
            "允许的 statuses：triage, todo, scheduled, ready, running, blocked, review, done, archived。"
        }
        kanban_core::Locale::En => {
            "Allowed statuses: triage, todo, scheduled, ready, running, blocked, review, done, archived."
        }
    }
}
