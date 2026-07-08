use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{Context, Result};
use kanban_sqlite::{
    SignalLifecycle, SignalListOptions, SignalRecord, SignalRecordInput, SignalReviewInput,
    SignalStatus, get_signal, list_signals, record_signal, review_signals, update_signal_status,
};

use crate::commands::common::{read_text_input, resolve_required_text_input};
use crate::{args::SignalCommand, output::print_or_json};

pub(crate) fn handle_signal(
    command: SignalCommand,
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    json: bool,
) -> Result<()> {
    match command {
        SignalCommand::Record(args) => {
            let input = read_record_input(&args.input)?;
            let result = record_signal(db_path, board, actor, input)?;
            print_or_json(json, &result, || signal_line(&result.signal))?;
        }
        SignalCommand::List(args) => {
            let signals = list_signals(
                db_path,
                board,
                list_options(
                    args.status,
                    args.kind,
                    args.task,
                    args.include_all,
                    args.limit,
                )?,
            )?;
            print_or_json(json, &signals, || signal_lines(&signals))?;
        }
        SignalCommand::Show { signal_id } => {
            let signal = get_signal(db_path, board, &signal_id)?;
            print_or_json(json, &signal, || signal_line(&signal))?;
        }
        SignalCommand::Review(args) => {
            let signals = review_signals(
                db_path,
                board,
                list_options(args.status, args.kind, args.task, false, args.limit)?,
            )?;
            print_or_json(json, &signals, || signal_lines(&signals))?;
        }
        SignalCommand::Confirm(args) => lifecycle(
            db_path,
            board,
            actor,
            json,
            LifecycleCommandInput {
                lifecycle: SignalLifecycle::Confirm,
                signal_ids: args.signal_ids,
                replacement_signal_id: None,
                reason: resolve_required_text_input(
                    args.reason,
                    args.reason_file,
                    "--reason",
                    "--reason-file",
                    "reason",
                )?,
            },
        )?,
        SignalCommand::Reject(args) => lifecycle(
            db_path,
            board,
            actor,
            json,
            LifecycleCommandInput {
                lifecycle: SignalLifecycle::Reject,
                signal_ids: args.signal_ids,
                replacement_signal_id: None,
                reason: resolve_required_text_input(
                    args.reason,
                    args.reason_file,
                    "--reason",
                    "--reason-file",
                    "reason",
                )?,
            },
        )?,
        SignalCommand::Resolve(args) => lifecycle(
            db_path,
            board,
            actor,
            json,
            LifecycleCommandInput {
                lifecycle: SignalLifecycle::Resolve,
                signal_ids: args.signal_ids,
                replacement_signal_id: None,
                reason: resolve_required_text_input(
                    args.reason,
                    args.reason_file,
                    "--reason",
                    "--reason-file",
                    "reason",
                )?,
            },
        )?,
        SignalCommand::Supersede(args) => lifecycle(
            db_path,
            board,
            actor,
            json,
            LifecycleCommandInput {
                lifecycle: SignalLifecycle::Supersede,
                signal_ids: args.signal_ids,
                replacement_signal_id: Some(args.by),
                reason: resolve_required_text_input(
                    args.reason,
                    args.reason_file,
                    "--reason",
                    "--reason-file",
                    "reason",
                )?,
            },
        )?,
    }
    Ok(())
}

fn read_record_input(path: &Path) -> Result<SignalRecordInput> {
    let content = read_text_input(path).context("failed to read signal input")?;
    serde_json::from_str(&content).context("failed to parse signal input JSON")
}

fn list_options(
    status: Vec<String>,
    kind: Vec<String>,
    task: Option<String>,
    include_all: bool,
    limit: usize,
) -> Result<SignalListOptions> {
    Ok(SignalListOptions {
        statuses: status
            .into_iter()
            .map(|value| SignalStatus::from_str(&value))
            .collect::<kanban_core::Result<Vec<_>>>()?,
        kinds: kind,
        task_ref: task,
        include_all,
        limit,
    })
}

struct LifecycleCommandInput {
    lifecycle: SignalLifecycle,
    signal_ids: Vec<String>,
    replacement_signal_id: Option<String>,
    reason: String,
}

fn lifecycle(
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    json: bool,
    input: LifecycleCommandInput,
) -> Result<()> {
    let signals = update_signal_status(
        db_path,
        board,
        actor,
        SignalReviewInput {
            signal_ids: input.signal_ids,
            lifecycle: input.lifecycle,
            replacement_signal_id: input.replacement_signal_id,
            reason: input.reason,
        },
    )?;
    print_or_json(json, &signals, || signal_lines(&signals))
}

fn signal_lines(signals: &[SignalRecord]) -> String {
    signals
        .iter()
        .map(signal_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn signal_line(signal: &SignalRecord) -> String {
    format!(
        "{} [{}] kind={} severity={} title={} task={}",
        signal.id,
        signal.status,
        signal.kind,
        signal.severity,
        signal.title,
        signal
            .observation
            .task_ref_snapshot
            .as_deref()
            .unwrap_or("-")
    )
}
