mod add;
mod create;
mod list;
pub(crate) mod ontology;
mod remove;

use clap::Subcommand;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum LabelCommand {
    /// List labels defined on the selected board.
    List(list::ListArgs),
    /// Create a board label, or return the existing label with the same name.
    Create(create::CreateArgs),
    /// Attach one or more labels to a task.
    Add(add::AddArgs),
    /// Remove a label from a task by id or name.
    Remove(remove::RemoveArgs),
    /// Manage label semantics text and examples.
    Semantics {
        #[command(subcommand)]
        command: ontology::SemanticsCommand,
    },
    /// Inspect label ontology atoms.
    #[command(alias = "atom")]
    Atoms {
        #[command(subcommand)]
        command: ontology::AtomCommand,
    },
    /// Maintain and query the label atom index.
    #[command(name = "atom-index")]
    AtomIndex {
        #[command(subcommand)]
        command: ontology::AtomIndexCommand,
    },
    /// Suggest labels for a task.
    Suggest(ontology::SuggestArgs),
    /// Create a label proposal from task context or JSON input.
    Propose(ontology::ProposeArgs),
    /// Review and decide pending label proposals.
    #[command(alias = "proposal")]
    Proposals {
        #[command(subcommand)]
        command: ontology::ProposalCommand,
    },
    /// Record, review, apply, and validate label ontology signals.
    Ontology {
        #[command(subcommand)]
        command: ontology::LedgerCommand,
    },
}

pub(crate) fn run(ctx: &CliContext, command: &LabelCommand) -> Result<(), CliFailure> {
    match command {
        LabelCommand::List(args) => list::run(ctx, args),
        LabelCommand::Create(args) => create::run(ctx, args),
        LabelCommand::Add(args) => add::run(ctx, args),
        LabelCommand::Remove(args) => remove::run(ctx, args),
        LabelCommand::Semantics { command } => ontology::run_semantics(ctx, command),
        LabelCommand::Atoms { command } => ontology::run_atoms(ctx, command),
        LabelCommand::AtomIndex { command } => ontology::run_atom_index(ctx, command),
        LabelCommand::Suggest(args) => ontology::run_suggest(ctx, args),
        LabelCommand::Propose(args) => ontology::run_propose(ctx, args),
        LabelCommand::Proposals { command } => ontology::run_proposals(ctx, command),
        LabelCommand::Ontology { command } => ontology::run_ledger(ctx, command),
    }
}
