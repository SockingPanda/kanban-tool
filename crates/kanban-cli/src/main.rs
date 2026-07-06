mod args;
mod commands;
mod output;

use anyhow::Error;
use kanban_core::KanbanError;
use serde::Serialize;

fn main() {
    let wants_json = std::env::args_os().any(|arg| arg == "--json");
    if let Err(error) = commands::app::run() {
        let report = CliErrorReport::from_error(&error);
        if wants_json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({ "error": report }))
                    .unwrap_or_else(|_| "{\"error\":{\"code\":\"generic_error\",\"message\":\"failed to render error\",\"exit_code\":1}}".to_owned())
            );
        } else {
            eprintln!("Error: {}", report.message);
        }
        std::process::exit(report.exit_code);
    }
}

#[derive(Debug, Serialize)]
struct CliErrorReport {
    code: &'static str,
    message: String,
    exit_code: i32,
}

impl CliErrorReport {
    fn from_error(error: &Error) -> Self {
        if let Some(kanban_error) = error
            .chain()
            .find_map(|cause| cause.downcast_ref::<KanbanError>())
        {
            let message =
                kanban_core::i18n::render_error(kanban_core::current_locale(), kanban_error);
            let (code, exit_code) = classify_kanban_error(kanban_error);
            return Self {
                code,
                message,
                exit_code,
            };
        }

        let message = format!("{error:?}");
        let (code, exit_code) = classify_error_message(&message).unwrap_or(("generic_error", 1));
        Self {
            code,
            message,
            exit_code,
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
    if normalized.contains("unsupported locale")
        || normalized.contains("only supports loopback hosts")
        || normalized.contains("mutually exclusive")
        || normalized.contains("requires either")
        || normalized.contains("outside allowed run log roots")
        || normalized.contains("must be >=")
        || normalized.contains("must be <=")
        || normalized.contains("must be positive")
        || normalized.contains("must be a positive")
        || normalized.contains("must be one of")
        || normalized.contains("does not exist")
        || normalized.contains("already exists")
        || normalized.contains("requires --replace")
        || normalized.contains("is not a file")
        || normalized.contains("unsupported task list sort")
    {
        return Some(("invalid_input", 2));
    }
    None
}
