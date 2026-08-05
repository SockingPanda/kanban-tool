use std::{fs, io::Read};

use clap::{Args, Subcommand};
use kanban_client::KanbanClient;
use kanban_protocol::{
    RecordSignalRequest, ReviewSignalsRequest, SignalQuery, SignalWire,
    cli_operator::{
        CliSignal, CliSignalConfirmOutput, CliSignalListOutput, CliSignalObservation,
        CliSignalRecordOutput, CliSignalRecordResult, CliSignalRejectOutput,
        CliSignalResolveOutput, CliSignalReviewOutput, CliSignalShowOutput, CliSignalStatus,
        CliSignalSupersedeOutput,
    },
};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Subcommand)]
pub(crate) enum SignalCommand {
    /// Record one generic signal from a JSON request body.
    Record(RecordArgs),
    /// List signals on the current board.
    List(ListArgs),
    /// Show one signal by global id.
    Show(ShowArgs),
    /// List signals eligible for review.
    Review(ListReviewArgs),
    /// Confirm one or more signals.
    Confirm(LifecycleArgs),
    /// Reject one or more signals.
    Reject(LifecycleArgs),
    /// Resolve one or more signals.
    Resolve(LifecycleArgs),
    /// Supersede one or more signals with another signal.
    Supersede(SupersedeArgs),
}

#[derive(Debug, Args)]
pub(crate) struct RecordArgs {
    /// JSON file, or '-' to read JSON from stdin.
    #[arg(long, default_value = "-")]
    pub(crate) input: String,
}

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    #[arg(long)]
    pub(crate) status: Vec<String>,
    #[arg(long)]
    pub(crate) kind: Vec<String>,
    #[arg(long)]
    pub(crate) task: Option<String>,
    #[arg(long)]
    pub(crate) include_all: bool,
    #[arg(long, default_value_t = 100)]
    pub(crate) limit: usize,
}

#[derive(Debug, Args)]
pub(crate) struct ListReviewArgs {
    #[arg(long)]
    pub(crate) status: Vec<String>,
    #[arg(long)]
    pub(crate) kind: Vec<String>,
    #[arg(long)]
    pub(crate) task: Option<String>,
    #[arg(long, default_value_t = 100)]
    pub(crate) limit: usize,
}

#[derive(Debug, Args)]
pub(crate) struct ShowArgs {
    pub(crate) signal_id: String,
}

#[derive(Debug, Args)]
pub(crate) struct LifecycleArgs {
    #[arg(required = true)]
    pub(crate) signal_ids: Vec<String>,
    #[arg(long)]
    pub(crate) reason: String,
}

#[derive(Debug, Args)]
pub(crate) struct SupersedeArgs {
    #[arg(required = true)]
    pub(crate) signal_ids: Vec<String>,
    #[arg(long = "by")]
    pub(crate) replacement_signal_id: String,
    #[arg(long)]
    pub(crate) reason: String,
}

pub(crate) fn run(ctx: &CliContext, command: &SignalCommand) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    match command {
        SignalCommand::Record(args) => record(ctx, &client, args),
        SignalCommand::List(args) => list(ctx, &client, args),
        SignalCommand::Show(args) => show(ctx, &client, args),
        SignalCommand::Review(args) => review(ctx, &client, args),
        SignalCommand::Confirm(args) => lifecycle(
            ctx,
            &client,
            "confirm",
            &args.signal_ids,
            None,
            &args.reason,
        ),
        SignalCommand::Reject(args) => {
            lifecycle(ctx, &client, "reject", &args.signal_ids, None, &args.reason)
        }
        SignalCommand::Resolve(args) => lifecycle(
            ctx,
            &client,
            "resolve",
            &args.signal_ids,
            None,
            &args.reason,
        ),
        SignalCommand::Supersede(args) => lifecycle(
            ctx,
            &client,
            "supersede",
            &args.signal_ids,
            Some(&args.replacement_signal_id),
            &args.reason,
        ),
    }
}

fn record(ctx: &CliContext, client: &KanbanClient, args: &RecordArgs) -> Result<(), CliFailure> {
    let request = read_record_request(&args.input)?;
    let response = client.record_signal(&ctx.board, &request)?;
    let signal = cli_signal(response.data.signal)?;
    if ctx.json {
        output::print_json(&CliSignalRecordOutput::new(CliSignalRecordResult {
            signal,
            backlink_comment: response.data.backlink_comment,
        }));
    } else {
        println!("{}", signal_line(&signal));
    }
    Ok(())
}

fn list(ctx: &CliContext, client: &KanbanClient, args: &ListArgs) -> Result<(), CliFailure> {
    let response = client.list_signals(
        &ctx.board,
        &signal_query(
            &args.status,
            &args.kind,
            &args.task,
            args.include_all,
            args.limit,
        ),
    )?;
    let signals = response
        .data
        .into_iter()
        .map(cli_signal)
        .collect::<Result<Vec<_>, _>>()?;
    if ctx.json {
        output::print_json(&CliSignalListOutput::new(signals));
    } else {
        print_signal_lines(&signals);
    }
    Ok(())
}

fn review(
    ctx: &CliContext,
    client: &KanbanClient,
    args: &ListReviewArgs,
) -> Result<(), CliFailure> {
    let query = signal_query(&args.status, &args.kind, &args.task, false, args.limit);
    let response = client.review_signals(&ctx.board, &query)?;
    let signals = response
        .data
        .into_iter()
        .map(cli_signal)
        .collect::<Result<Vec<_>, _>>()?;
    if ctx.json {
        output::print_json(&CliSignalReviewOutput::new(signals));
    } else {
        print_signal_lines(&signals);
    }
    Ok(())
}

fn show(ctx: &CliContext, client: &KanbanClient, args: &ShowArgs) -> Result<(), CliFailure> {
    let response = client.get_signal(&args.signal_id)?;
    let signal = cli_signal(response.data)?;
    if ctx.json {
        output::print_json(&CliSignalShowOutput::new(signal));
    } else {
        println!("{}", signal_line(&signal));
    }
    Ok(())
}

fn lifecycle(
    ctx: &CliContext,
    client: &KanbanClient,
    action: &str,
    signal_ids: &[String],
    replacement_signal_id: Option<&String>,
    reason: &str,
) -> Result<(), CliFailure> {
    let request = ReviewSignalsRequest {
        signal_ids: signal_ids.to_vec(),
        reason: reason.to_owned(),
        replacement_signal_id: replacement_signal_id.cloned(),
        actor: Some(ctx.actor()),
        expected_updated_at: None,
    };
    let signals = match action {
        "confirm" => client.confirm_signals(&ctx.board, &request)?.data,
        "reject" => client.reject_signals(&ctx.board, &request)?.data,
        "resolve" => client.resolve_signals(&ctx.board, &request)?.data,
        "supersede" => client.supersede_signals(&ctx.board, &request)?.data,
        _ => unreachable!("signal lifecycle action is selected by clap"),
    }
    .into_iter()
    .map(cli_signal)
    .collect::<Result<Vec<_>, _>>()?;
    if ctx.json {
        match action {
            "confirm" => output::print_json(&CliSignalConfirmOutput::new(signals)),
            "reject" => output::print_json(&CliSignalRejectOutput::new(signals)),
            "resolve" => output::print_json(&CliSignalResolveOutput::new(signals)),
            "supersede" => output::print_json(&CliSignalSupersedeOutput::new(signals)),
            _ => unreachable!(),
        }
    } else {
        print_signal_lines(&signals);
    }
    Ok(())
}

fn read_record_request(path: &str) -> Result<RecordSignalRequest, CliFailure> {
    let content = if path == "-" {
        let mut content = String::new();
        std::io::stdin()
            .read_to_string(&mut content)
            .map_err(|error| invalid_input(format!("failed to read signal input: {error}")))?;
        content
    } else {
        fs::read_to_string(path).map_err(|error| {
            invalid_input(format!("failed to read signal input {path}: {error}"))
        })?
    };
    serde_json::from_str(&content)
        .map_err(|error| invalid_input(format!("failed to parse signal input JSON: {error}")))
}

fn signal_query(
    status: &[String],
    kind: &[String],
    task: &Option<String>,
    include_all: bool,
    limit: usize,
) -> SignalQuery {
    SignalQuery {
        status: status.to_vec(),
        kind: kind.to_vec(),
        task_ref: task.clone(),
        include_all,
        limit,
    }
}

fn cli_signal(signal: SignalWire) -> Result<CliSignal, CliFailure> {
    let status = match signal.status.as_str() {
        "open" => CliSignalStatus::Open,
        "confirmed" => CliSignalStatus::Confirmed,
        "rejected" => CliSignalStatus::Rejected,
        "superseded" => CliSignalStatus::Superseded,
        "resolved" => CliSignalStatus::Resolved,
        value => return Err(invalid_input(format!("unknown signal status: {value}"))),
    };
    let observation = signal.observation;
    Ok(CliSignal {
        id: signal.id,
        board_id: signal.board_id,
        observation_id: signal.observation_id,
        kind: signal.kind,
        title: signal.title,
        summary: signal.summary,
        severity: signal.severity,
        status,
        dedupe_key: signal.dedupe_key,
        superseded_by_signal_id: signal.superseded_by_signal_id,
        reviewed_by: signal.reviewed_by,
        reviewed_at: signal.reviewed_at,
        review_reason: signal.review_reason,
        created_at: signal.created_at,
        updated_at: signal.updated_at,
        observation: CliSignalObservation {
            id: observation.id,
            board_id: observation.board_id,
            task_id: observation.task_id,
            task_ref_snapshot: observation.task_ref_snapshot,
            run_id: observation.run_id,
            comment_id: observation.comment_id,
            actor: observation.actor,
            agent_type: observation.agent_type,
            source: observation.source,
            evidence: observation.evidence,
            created_at: observation.created_at,
        },
    })
}

fn signal_line(signal: &CliSignal) -> String {
    format!(
        "{} [{}] kind={} severity={} title={} task={}",
        signal.id,
        signal.status.as_str(),
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

fn print_signal_lines(signals: &[CliSignal]) {
    for signal in signals {
        println!("{}", signal_line(signal));
    }
}

fn invalid_input(message: String) -> CliFailure {
    CliFailure {
        code: "invalid_input",
        message,
        exit_code: 2,
    }
}

trait SignalStatusDisplay {
    fn as_str(self) -> &'static str;
}

impl SignalStatusDisplay for CliSignalStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Confirmed => "confirmed",
            Self::Rejected => "rejected",
            Self::Superseded => "superseded",
            Self::Resolved => "resolved",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_signal_lifecycle_commands() {
        let cli = crate::Cli::try_parse_from([
            "kanban",
            "signal",
            "supersede",
            "sig_one",
            "--by",
            "sig_two",
            "--reason",
            "duplicate",
        ])
        .expect("signal supersede args");
        let crate::Command::Signal { command } = cli.command else {
            panic!("expected signal command");
        };
        let SignalCommand::Supersede(args) = command else {
            panic!("expected supersede command");
        };
        assert_eq!(args.signal_ids, vec!["sig_one"]);
        assert_eq!(args.replacement_signal_id, "sig_two");
    }

    #[test]
    fn signal_query_preserves_repeated_filters() {
        let query = signal_query(
            &["open".into(), "confirmed".into()],
            &["failure".into()],
            &Some("default#1".into()),
            true,
            10,
        );
        assert_eq!(query.status.len(), 2);
        assert!(query.include_all);
    }
}
