mod commands;
mod context;
mod error;
mod output;
mod server;

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use kanban_client::DEFAULT_SERVER_URL;

use context::CliContext;
use error::{CliFailure, feature_not_available};

#[derive(Debug, Parser)]
#[command(
    name = "kanban",
    version,
    about = "Local Turso-backed Kanban work queue",
    arg_required_else_help = true,
    after_help = "All product commands call kanban serve; only `kanban serve` opens the database."
)]
struct Cli {
    /// Canonical localhost application host.
    #[arg(
        long,
        global = true,
        env = "KANBAN_SERVER_URL",
        default_value = DEFAULT_SERVER_URL
    )]
    server_url: String,
    /// Board slug or id used by board-scoped client commands.
    #[arg(long, global = true, env = "KB_BOARD", default_value = "default")]
    board: String,
    /// Audit actor sent to the application host.
    #[arg(long, global = true, env = "KANBAN_ACTOR")]
    actor: Option<String>,
    /// Emit stable JSON envelopes.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Start the only process allowed to open the Turso database.
    Serve(server::ServeArgs),
    /// Query boards through the localhost application host.
    Board {
        #[command(subcommand)]
        command: commands::board::BoardCommand,
    },
    /// Manage tasks through the canonical localhost host.
    Task {
        #[command(subcommand)]
        command: commands::task::TaskCommand,
    },
    /// Manage label semantics and ontology ledger through the canonical host.
    #[command(name = "label", visible_alias = "ontology", visible_alias = "labels")]
    Label {
        #[command(subcommand)]
        command: commands::ontology::OntologyCommand,
    },
    /// Manage task comments through the canonical localhost host.
    Comment {
        #[command(subcommand)]
        command: commands::comment::CommentCommand,
    },
    /// Manage task dependencies through the canonical localhost host.
    #[command(name = "dep", visible_alias = "dependency")]
    Dependency {
        #[command(subcommand)]
        command: commands::dependency::DependencyCommand,
    },
    /// List canonical task events through the localhost host.
    Events(commands::event::ListArgs),
    /// List execution runs for a task through the canonical localhost host.
    Runs(commands::run::ListArgs),
    /// Inspect one execution run through the canonical localhost host.
    Run {
        #[command(subcommand)]
        command: commands::run::RunCommand,
    },
    /// Removed direct-database initialization path.
    Init,
    /// Commands not yet migrated to the canonical host fail without touching storage.
    #[command(external_subcommand)]
    FeatureNotAvailable(Vec<String>),
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => output::finish_failure(cli.json, &error),
    }
}

async fn run(cli: &Cli) -> Result<(), CliFailure> {
    let ctx = CliContext::from_cli(cli);
    match &cli.command {
        Command::Serve(args) => server::run(&ctx, args).await,
        Command::Board { command } => commands::board::run(&ctx, command),
        Command::Comment { command } => commands::comment::run(&ctx, command),
        Command::Dependency { command } => commands::dependency::run(&ctx, command),
        Command::Events(args) => commands::event::run(&ctx, args),
        Command::Run { command } => commands::run::run(&ctx, command),
        Command::Runs(args) => commands::run::list(&ctx, args),
        Command::Task { command } => commands::task::run(&ctx, command),
        Command::Label { command } => commands::ontology::run(&ctx, command),
        Command::Init => Err(feature_not_available(
            "`kanban init` was removed; start `kanban serve` to initialize the canonical Turso database",
        )),
        Command::FeatureNotAvailable(parts) => Err(feature_not_available(format!(
            "command `{}` is not available on the single-host path yet",
            parts.join(" ")
        ))),
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::error::feature_not_available;
    use crate::{Cli, Command};

    #[test]
    fn init_is_a_stable_unavailable_feature() {
        let failure = feature_not_available("not migrated");
        assert_eq!(failure.code, "feature_not_available");
        assert_eq!(failure.exit_code, 10);
    }

    #[test]
    fn parses_run_list_command() {
        let cli = Cli::try_parse_from(["kanban", "runs", "default#1"])
            .expect("run list command should parse");
        let Command::Runs(args) = cli.command else {
            panic!("expected runs command");
        };
        assert_eq!(args.task_ref, "default#1");
    }

    #[test]
    fn parses_run_show_command() {
        let cli = Cli::try_parse_from(["kanban", "run", "show", "r_example"])
            .expect("run show command should parse");
        let Command::Run {
            command: crate::commands::run::RunCommand::Show(args),
        } = cli.command
        else {
            panic!("expected run show command");
        };
        assert_eq!(args.run_id, "r_example");
    }

    #[test]
    fn parses_run_logs_without_tail_options() {
        let cli = Cli::try_parse_from(["kanban", "run", "logs", "r_example"])
            .expect("run logs command should parse");
        let Command::Run {
            command: crate::commands::run::RunCommand::Logs(args),
        } = cli.command
        else {
            panic!("expected run logs command");
        };
        assert_eq!(args.run_id, "r_example");

        assert!(
            Cli::try_parse_from(["kanban", "run", "logs", "r_example", "--tail-bytes", "1024"])
                .is_err()
        );
    }

    #[test]
    fn parses_event_list_command() {
        let cli = Cli::try_parse_from([
            "kanban",
            "events",
            "default#1",
            "--after",
            "10",
            "--limit",
            "25",
        ])
        .expect("event list command should parse");
        let Command::Events(args) = cli.command else {
            panic!("expected events command");
        };
        assert_eq!(args.task_ref.as_deref(), Some("default#1"));
        assert_eq!(args.after, 10);
        assert_eq!(args.limit, 25);
    }
}
