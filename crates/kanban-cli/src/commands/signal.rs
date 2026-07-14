use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{Context, Result};
use kanban_contract::cli_operator::{
    CliSignal, CliSignalConfirmOutput, CliSignalListOutput, CliSignalObservation,
    CliSignalRecordOutput, CliSignalRecordResult, CliSignalRejectOutput, CliSignalResolveOutput,
    CliSignalReviewOutput, CliSignalShowOutput, CliSignalStatus, CliSignalSupersedeOutput,
};
use kanban_contract::structured_metadata::SignalRecordMetadataInput;
use kanban_core::KanbanError;
use kanban_sqlite::api::{
    SignalLifecycle, SignalListOptions, SignalRecord, SignalRecordInput, SignalRecordResult,
    SignalReviewInput, SignalStatus, get_signal, list_signals, record_signal, review_signals,
    update_signal_status,
};

use crate::commands::common::{read_text_input, resolve_required_text_input};
use crate::{
    args::SignalCommand,
    output::{api_comment_from_record, print_contract_or_human},
};

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
            let output = CliSignalRecordOutput::new(cli_signal_record_result(&result)?);
            print_contract_or_human(json, &output, || signal_line(&result.signal))?;
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
            let output = CliSignalListOutput::new(cli_signals(&signals)?);
            print_contract_or_human(json, &output, || signal_lines(&signals))?;
        }
        SignalCommand::Show { signal_id } => {
            let signal = get_signal(db_path, board, &signal_id)?;
            let output = CliSignalShowOutput::new(cli_signal(&signal)?);
            print_contract_or_human(json, &output, || signal_line(&signal))?;
        }
        SignalCommand::Review(args) => {
            let signals = review_signals(
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
            let output = CliSignalReviewOutput::new(cli_signals(&signals)?);
            print_contract_or_human(json, &output, || signal_lines(&signals))?;
        }
        SignalCommand::Confirm(args) => lifecycle_confirm(
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
        SignalCommand::Reject(args) => lifecycle_reject(
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
        SignalCommand::Resolve(args) => lifecycle_resolve(
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
        SignalCommand::Supersede(args) => lifecycle_supersede(
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
    let contract: SignalRecordMetadataInput =
        serde_json::from_str(&content).context("failed to parse signal input JSON")?;
    serde_json::from_value(serde_json::to_value(contract)?)
        .context("failed to adapt signal input contract")
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

fn update_lifecycle(
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    input: LifecycleCommandInput,
) -> Result<Vec<SignalRecord>> {
    Ok(update_signal_status(
        db_path,
        board,
        actor,
        SignalReviewInput {
            signal_ids: input.signal_ids,
            lifecycle: input.lifecycle,
            replacement_signal_id: input.replacement_signal_id,
            reason: input.reason,
        },
    )?)
}

fn lifecycle_confirm(
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    json: bool,
    input: LifecycleCommandInput,
) -> Result<()> {
    let signals = update_lifecycle(db_path, board, actor, input)?;
    let output = CliSignalConfirmOutput::new(cli_signals(&signals)?);
    print_contract_or_human(json, &output, || signal_lines(&signals))
}

fn lifecycle_reject(
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    json: bool,
    input: LifecycleCommandInput,
) -> Result<()> {
    let signals = update_lifecycle(db_path, board, actor, input)?;
    let output = CliSignalRejectOutput::new(cli_signals(&signals)?);
    print_contract_or_human(json, &output, || signal_lines(&signals))
}

fn lifecycle_resolve(
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    json: bool,
    input: LifecycleCommandInput,
) -> Result<()> {
    let signals = update_lifecycle(db_path, board, actor, input)?;
    let output = CliSignalResolveOutput::new(cli_signals(&signals)?);
    print_contract_or_human(json, &output, || signal_lines(&signals))
}

fn lifecycle_supersede(
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    json: bool,
    input: LifecycleCommandInput,
) -> Result<()> {
    let signals = update_lifecycle(db_path, board, actor, input)?;
    let output = CliSignalSupersedeOutput::new(cli_signals(&signals)?);
    print_contract_or_human(json, &output, || signal_lines(&signals))
}

fn cli_signal_record_result(result: &SignalRecordResult) -> Result<CliSignalRecordResult> {
    Ok(CliSignalRecordResult {
        signal: cli_signal(&result.signal)?,
        backlink_comment: result
            .backlink_comment
            .as_ref()
            .map(api_comment_from_record)
            .transpose()?,
    })
}

fn cli_signals(signals: &[SignalRecord]) -> Result<Vec<CliSignal>> {
    signals.iter().map(cli_signal).collect()
}

fn cli_signal(signal: &SignalRecord) -> Result<CliSignal> {
    Ok(CliSignal {
        id: signal.id.clone(),
        board_id: signal.board_id.clone(),
        observation_id: signal.observation_id.clone(),
        kind: signal.kind.clone(),
        title: signal.title.clone(),
        summary: signal.summary.clone(),
        severity: signal.severity.clone(),
        status: cli_signal_status(&signal.status)?,
        dedupe_key: signal.dedupe_key.clone(),
        superseded_by_signal_id: signal.superseded_by_signal_id.clone(),
        reviewed_by: signal.reviewed_by.clone(),
        reviewed_at: signal.reviewed_at,
        review_reason: signal.review_reason.clone(),
        created_at: signal.created_at,
        updated_at: signal.updated_at,
        observation: CliSignalObservation {
            id: signal.observation.id.clone(),
            board_id: signal.observation.board_id.clone(),
            task_id: signal.observation.task_id.clone(),
            task_ref_snapshot: signal.observation.task_ref_snapshot.clone(),
            run_id: signal.observation.run_id.clone(),
            comment_id: signal.observation.comment_id.clone(),
            actor: signal.observation.actor.clone(),
            agent_type: signal.observation.agent_type.clone(),
            source: signal.observation.source.clone(),
            evidence: serde_json::from_str(&signal.observation.evidence_json).map_err(|error| {
                KanbanError::Storage(format!(
                    "signal observation {} has invalid evidence_json: {error}",
                    signal.observation.id
                ))
            })?,
            created_at: signal.observation.created_at,
        },
    })
}

fn cli_signal_status(status: &str) -> Result<CliSignalStatus> {
    match status {
        "open" => Ok(CliSignalStatus::Open),
        "confirmed" => Ok(CliSignalStatus::Confirmed),
        "rejected" => Ok(CliSignalStatus::Rejected),
        "superseded" => Ok(CliSignalStatus::Superseded),
        "resolved" => Ok(CliSignalStatus::Resolved),
        value => anyhow::bail!("signal output has invalid status {value}"),
    }
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
