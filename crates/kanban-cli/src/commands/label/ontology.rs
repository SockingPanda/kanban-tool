//! label semantics 与 ontology ledger 的 CLI adapter。

use clap::{Args, Subcommand};
use serde_json::{Value, json};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Subcommand)]
pub(crate) enum SemanticsCommand {
    List,
    Show(RefArgs),
    Upsert(PayloadArgs),
    Delete(DeleteArgs),
}
#[derive(Debug, Subcommand)]
pub(crate) enum AtomCommand {
    List,
    Explain(RefArgs),
}
#[derive(Debug, Subcommand)]
pub(crate) enum AtomIndexCommand {
    Status,
    Rebuild,
    Query(IndexQueryArgs),
}
#[derive(Debug, Subcommand)]
pub(crate) enum ProposalCommand {
    List(ProposalListArgs),
    Show(RefArgs),
    Accept(DecisionArgs),
    Reject(DecisionArgs),
}
#[derive(Debug, Subcommand)]
pub(crate) enum LedgerCommand {
    #[command(name = "list", visible_alias = "signals")]
    Signals(SignalListArgs),
    Show(RefArgs),
    Review(ReviewArgs),
    Quality(QualityArgs),
    #[command(name = "confirm", visible_alias = "action")]
    Confirm(PayloadArgs),
    Reject(PayloadArgs),
    Resolve(PayloadArgs),
    Supersede(PayloadArgs),
    Apply {
        #[command(subcommand)]
        command: ApplyCommand,
    },
    Revert(PayloadArgs),
    Validate(PayloadArgs),
    Record(RecordArgs),
}

#[derive(Debug, Subcommand)]
pub(crate) enum ApplyCommand {
    Atom(PayloadArgs),
}

#[derive(Debug, Args)]
pub(crate) struct RefArgs {
    pub(crate) reference: String,
}
#[derive(Debug, Args)]
pub(crate) struct PayloadArgs {
    pub(crate) reference: Option<String>,
    #[arg(long, default_value = "{}")]
    pub(crate) payload: String,
}
#[derive(Debug, Args)]
pub(crate) struct DeleteArgs {
    pub(crate) reference: String,
    #[arg(long)]
    pub(crate) expected_hash: String,
    #[arg(long, default_value = "delete label semantics")]
    pub(crate) reason: String,
}
#[derive(Debug, Args)]
pub(crate) struct IndexQueryArgs {
    #[arg(long)]
    pub(crate) q: Option<String>,
    #[arg(long)]
    pub(crate) polarity: Option<String>,
    #[arg(long, default_value_t = 24)]
    pub(crate) limit: usize,
}
#[derive(Debug, Args)]
pub(crate) struct SuggestArgs {
    pub(crate) task_ref: String,
    #[arg(long, default_value_t = 5)]
    pub(crate) limit: usize,
    #[arg(long, default_value_t = 32)]
    pub(crate) candidate_limit: usize,
    #[arg(long, default_value_t = 80)]
    pub(crate) atom_limit: usize,
    #[arg(long, default_value_t = 4)]
    pub(crate) max_selected_labels: usize,
    #[arg(long, default_value_t = 0.15)]
    pub(crate) min_score: f32,
}
#[derive(Debug, Args)]
pub(crate) struct ProposeArgs {
    pub(crate) task_ref: String,
    #[arg(long)]
    pub(crate) name: Option<String>,
    #[arg(long, default_value = "{}")]
    pub(crate) payload: String,
}
#[derive(Debug, Args)]
pub(crate) struct ProposalListArgs {
    #[arg(long)]
    pub(crate) task_ref: Option<String>,
    #[arg(long)]
    pub(crate) status: Option<String>,
}
#[derive(Debug, Args)]
pub(crate) struct DecisionArgs {
    pub(crate) proposal_id: String,
    #[arg(long)]
    pub(crate) reason: Option<String>,
}
#[derive(Debug, Args)]
pub(crate) struct SignalListArgs {
    #[arg(long)]
    pub(crate) status: Option<String>,
    #[arg(long)]
    pub(crate) kind: Option<String>,
    #[arg(long, default_value_t = false)]
    pub(crate) include_all: bool,
    #[arg(long, default_value_t = 100)]
    pub(crate) limit: usize,
}
#[derive(Debug, Args)]
pub(crate) struct ReviewArgs {
    #[arg(long, default_value = "label")]
    pub(crate) group_by: String,
    #[arg(long, default_value_t = false)]
    pub(crate) include_all: bool,
    #[arg(long, default_value_t = 100)]
    pub(crate) limit: usize,
}
#[derive(Debug, Args)]
pub(crate) struct QualityArgs {
    #[arg(long, default_value_t = 20)]
    pub(crate) sample_limit: usize,
}
#[derive(Debug, Args)]
pub(crate) struct RecordArgs {
    pub(crate) task_ref: String,
    #[arg(long, default_value = "{}")]
    pub(crate) payload: String,
}

fn parse_json(raw: &str) -> Result<Value, CliFailure> {
    serde_json::from_str(raw).map_err(|error| CliFailure {
        code: "invalid_input",
        message: format!("JSON payload 无效：{error}"),
        exit_code: 2,
    })
}

fn emit(ctx: &CliContext, value: Value) {
    if ctx.json {
        output::print_json(&value);
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
        );
    }
}

pub(crate) fn run_semantics(
    ctx: &CliContext,
    command: &SemanticsCommand,
) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let value = match command {
        SemanticsCommand::List => client.list_label_semantics(&ctx.board)?,
        SemanticsCommand::Show(args) => client.get_label_semantics(&ctx.board, &args.reference)?,
        SemanticsCommand::Upsert(args) => client.upsert_label_semantics(
            &ctx.board,
            args.reference.as_deref().unwrap_or(""),
            parse_json(&args.payload)?,
        )?,
        SemanticsCommand::Delete(args) => client.delete_label_semantics(
            &ctx.board,
            &args.reference,
            &args.expected_hash,
            &args.reason,
        )?,
    };
    emit(ctx, value);
    Ok(())
}

pub(crate) fn run_atoms(ctx: &CliContext, command: &AtomCommand) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let value = match command {
        AtomCommand::List => client.list_label_atoms(&ctx.board)?,
        AtomCommand::Explain(args) => client.explain_label_atom(&ctx.board, &args.reference)?,
    };
    emit(ctx, value);
    Ok(())
}

pub(crate) fn run_atom_index(
    ctx: &CliContext,
    command: &AtomIndexCommand,
) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let value = match command {
        AtomIndexCommand::Status => client.label_atom_index_status(&ctx.board)?,
        AtomIndexCommand::Rebuild => client.rebuild_label_atom_index(&ctx.board)?,
        AtomIndexCommand::Query(args) => client.query_label_atom_index(
            &ctx.board,
            args.q.as_deref(),
            args.polarity.as_deref(),
            args.limit,
        )?,
    };
    emit(ctx, value);
    Ok(())
}

pub(crate) fn run_suggest(ctx: &CliContext, args: &SuggestArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let value = client.suggest_task_labels(
        &args.task_ref,
        Some(&ctx.board),
        json!({
            "limit": args.limit,
            "candidate_limit": args.candidate_limit,
            "atom_limit": args.atom_limit,
            "max_selected_labels": args.max_selected_labels,
            "min_score": args.min_score
        }),
    )?;
    emit(ctx, value);
    Ok(())
}

pub(crate) fn run_propose(ctx: &CliContext, args: &ProposeArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let mut payload = parse_json(&args.payload)?;
    if let Some(name) = &args.name {
        payload["name"] = Value::String(name.clone());
    }
    let value = client.propose_task_label(&ctx.board, &args.task_ref, payload)?;
    emit(ctx, value);
    Ok(())
}

pub(crate) fn run_proposals(ctx: &CliContext, command: &ProposalCommand) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let value = match command {
        ProposalCommand::List(args) => client.list_label_proposals(
            &ctx.board,
            args.task_ref.as_deref(),
            args.status.as_deref(),
        )?,
        ProposalCommand::Show(args) => client.get_label_proposal(&args.reference)?,
        ProposalCommand::Accept(args) => client.decide_label_proposal(
            &args.proposal_id,
            true,
            json!({"reason": args.reason, "actor": ctx.actor()}),
        )?,
        ProposalCommand::Reject(args) => client.decide_label_proposal(
            &args.proposal_id,
            false,
            json!({"reason": args.reason, "actor": ctx.actor()}),
        )?,
    };
    emit(ctx, value);
    Ok(())
}

fn action_payload(args: &PayloadArgs, action_type: &str) -> Result<Value, CliFailure> {
    let mut payload = parse_json(&args.payload)?;
    if payload.get("action_type").is_none() {
        payload["action_type"] = Value::String(action_type.to_owned());
    }
    Ok(payload)
}

pub(crate) fn run_ledger(ctx: &CliContext, command: &LedgerCommand) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    let value = match command {
        LedgerCommand::Signals(args) => client.list_label_ontology_signals(
            &ctx.board,
            json!({
                "status": args.status,
                "kind": args.kind,
                "include_all": args.include_all,
                "limit": args.limit
            }),
        )?,
        LedgerCommand::Show(args) => client.get_label_ontology_signal(&args.reference)?,
        LedgerCommand::Review(args) => client.review_label_ontology(
            &ctx.board,
            json!({
                "group_by": args.group_by,
                "include_all": args.include_all,
                "limit": args.limit
            }),
        )?,
        LedgerCommand::Quality(args) => {
            client.label_ontology_quality(&ctx.board, args.sample_limit)?
        }
        LedgerCommand::Confirm(args) => {
            client.create_label_ontology_action(&ctx.board, action_payload(args, "confirm")?)?
        }
        LedgerCommand::Reject(args) => {
            client.create_label_ontology_action(&ctx.board, action_payload(args, "reject")?)?
        }
        LedgerCommand::Resolve(args) => client
            .create_label_ontology_action(&ctx.board, action_payload(args, "resolve_no_change")?)?,
        LedgerCommand::Supersede(args) => {
            client.create_label_ontology_action(&ctx.board, action_payload(args, "supersede")?)?
        }
        LedgerCommand::Apply { command } => match command {
            ApplyCommand::Atom(args) => {
                client.apply_label_ontology_atom(&ctx.board, parse_json(&args.payload)?)?
            }
        },
        LedgerCommand::Revert(args) => {
            client.revert_label_ontology(&ctx.board, parse_json(&args.payload)?)?
        }
        LedgerCommand::Validate(args) => {
            client.validate_label_ontology(&ctx.board, parse_json(&args.payload)?)?
        }
        LedgerCommand::Record(args) => client.record_label_ontology_observation(
            &ctx.board,
            &args.task_ref,
            parse_json(&args.payload)?,
        )?,
    };
    emit(ctx, value);
    Ok(())
}
