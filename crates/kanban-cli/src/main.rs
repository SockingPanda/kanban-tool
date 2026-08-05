mod commands;
mod completion;
mod config;
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
    #[arg(long, global = true)]
    board: Option<String>,
    /// Canonical Turso path used by `serve` and configuration inspection only.
    #[arg(long, global = true)]
    db: Option<std::path::PathBuf>,
    /// Human-readable output locale; JSON keys and enums stay stable.
    #[arg(long, global = true)]
    locale: Option<String>,
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
    /// Select and inspect the project-local active board without opening Turso.
    Config {
        #[command(subcommand)]
        command: commands::config::ConfigCommand,
    },
    /// Manage tasks through the canonical localhost host.
    Task {
        #[command(subcommand)]
        command: commands::task::TaskCommand,
    },
    /// Manage board labels and task label bindings through the canonical host.
    #[command(visible_alias = "labels", visible_alias = "ontology")]
    Label {
        #[command(subcommand)]
        command: commands::label::LabelCommand,
    },
    /// Manage task comments through the canonical localhost host.
    Comment {
        #[command(subcommand)]
        command: commands::comment::CommentCommand,
    },
    /// 构建 task/reference/query 的只读混合上下文包。
    Context {
        #[command(subcommand)]
        command: commands::context::ContextCommand,
    },
    /// Manage file-backed task attachments through the canonical localhost host.
    Attachment {
        #[command(subcommand)]
        command: commands::attachment::AttachmentCommand,
    },
    /// Manage task dependencies through the canonical localhost host.
    #[command(name = "dep", visible_alias = "dependency")]
    Dependency {
        #[command(subcommand)]
        command: commands::dependency::DependencyCommand,
    },
    /// Inspect and maintain canonical graph entities.
    #[command(name = "entity", visible_alias = "entities")]
    Entity {
        #[command(subcommand)]
        command: commands::entities::EntityCommand,
    },
    /// Query the bounded canonical task/entity graph.
    Graph {
        #[command(subcommand)]
        command: commands::graph::GraphCommand,
    },
    /// List canonical task events through the localhost host.
    Events(commands::event::ListArgs),
    /// List execution runs for a task through the canonical localhost host.
    Runs(commands::run::ListArgs),
    /// 检查 canonical 数据库健康状态。
    Doctor,
    /// 查询 canonical 队列统计。
    Stats(commands::maintenance::StatsArgs),
    /// 创建 verified backup。
    Backup(commands::maintenance::PathArgs),
    /// 导出 portable canonical JSONL。
    Export(commands::maintenance::PathArgs),
    /// 导入 portable canonical JSONL。
    Import(commands::maintenance::ImportArgs),
    /// 导入 legacy SQLite v30 数据（仅在 host feature 启用时可用）。
    #[command(name = "import-v30")]
    ImportV30(commands::maintenance::LegacyImportArgs),
    /// 运行 WAL checkpoint。
    Checkpoint,
    /// 执行 host-owned compaction。
    Vacuum,
    /// 管理 projection owner、generation 和 recovery。
    Maintenance(commands::maintenance::MaintenanceArgs),
    /// Inspect one execution run through the canonical localhost host.
    Run {
        #[command(subcommand)]
        command: commands::run::RunCommand,
    },
    /// Search tasks through the canonical localhost host.
    Search(commands::search::SearchArgs),
    /// Inspect and maintain the task search projection.
    Index {
        #[command(subcommand)]
        command: commands::index::IndexCommand,
    },
    /// Record and review generic signals through the canonical host.
    Signal {
        #[command(subcommand)]
        command: commands::signal::SignalCommand,
    },
    /// Create the project `.kb/config.toml` selection file without opening Turso.
    Init {
        /// Idempotent compatibility flag; init never resets existing user settings.
        #[arg(
            long,
            help = "Deprecated compatibility no-op; init is idempotent and never resets data"
        )]
        force: bool,
    },
    /// Generate shell completion scripts without touching configuration or Turso.
    Completions { shell: completion::Shell },
    /// Hidden protocol used by generated Bash/Zsh dynamic completion helpers.
    #[command(name = "__complete", hide = true)]
    Complete(completion::CompleteArgs),
    /// Install or inspect Codex lifecycle hooks.
    Hook {
        #[command(subcommand)]
        command: commands::hook::HookCommand,
    },
    /// 管理 host 内 Ollama + Turso vector projection。
    Vector {
        #[command(subcommand)]
        command: commands::vector::VectorCommand,
    },
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
    if let Command::Completions { shell } = &cli.command {
        return completion::generate(*shell).map_err(|error| CliFailure {
            code: "generic_error",
            message: error.to_string(),
            exit_code: 1,
        });
    }
    if let Command::Complete(args) = &cli.command {
        return completion::complete(args, cli.board.as_deref());
    }
    if let Command::Config { command } = &cli.command {
        return commands::config::run(
            command,
            cli.db.as_deref(),
            cli.board.as_deref(),
            cli.locale.as_deref(),
            cli.json,
        );
    }
    if let Command::Init { force: _ } = &cli.command {
        return commands::init::run(cli.db.as_deref(), cli.board.as_deref(), cli.json);
    }
    if let Command::Hook { command } = &cli.command {
        return commands::hook::run(command, cli.json);
    }
    let ctx = CliContext::from_cli(cli)?;
    match &cli.command {
        Command::Serve(args) => server::run(&ctx, args).await,
        Command::Board { command } => commands::board::run(&ctx, command),
        Command::Comment { command } => commands::comment::run(&ctx, command),
        Command::Context { command } => commands::context::run(&ctx, command),
        Command::Attachment { command } => commands::attachment::run(&ctx, command),
        Command::Dependency { command } => commands::dependency::run(&ctx, command),
        Command::Entity { command } => commands::entities::run(&ctx, command),
        Command::Graph { command } => commands::graph::run(&ctx, command),
        Command::Events(args) => commands::event::run(&ctx, args),
        Command::Run { command } => commands::run::run(&ctx, command),
        Command::Signal { command } => commands::signal::run(&ctx, command),
        Command::Runs(args) => commands::run::list(&ctx, args),
        Command::Doctor => commands::maintenance::doctor(&ctx),
        Command::Stats(args) => commands::maintenance::stats(&ctx, args),
        Command::Backup(args) => commands::maintenance::backup(&ctx, args),
        Command::Export(args) => commands::maintenance::export(&ctx, args),
        Command::Import(args) => commands::maintenance::import(&ctx, args),
        Command::ImportV30(args) => commands::maintenance::import_v30(&ctx, args),
        Command::Checkpoint => commands::maintenance::checkpoint(&ctx),
        Command::Vacuum => commands::maintenance::vacuum(&ctx),
        Command::Maintenance(args) => commands::maintenance::maintenance(&ctx, args),
        Command::Task { command } => commands::task::run(&ctx, command),
        Command::Label { command } => commands::label::run(&ctx, command),
        Command::Search(args) => commands::search::run(&ctx, args),
        Command::Index { command } => commands::index::run(&ctx, command),
        Command::Vector { command } => commands::vector::run(&ctx, command),
        Command::Config { .. }
        | Command::Init { .. }
        | Command::Hook { .. }
        | Command::Completions { .. }
        | Command::Complete(_) => unreachable!("handled before client command dispatch"),
        Command::FeatureNotAvailable(parts) => Err(feature_not_available(format!(
            "command `{}` is not available on the single-host path yet",
            parts.join(" ")
        ))),
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::{Cli, Command};

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

    #[test]
    fn parses_label_add_command() {
        let cli = Cli::try_parse_from([
            "kanban",
            "label",
            "add",
            "default#1",
            "backend",
            "api",
            "--create-missing",
        ])
        .expect("label add command should parse");
        let Command::Label {
            command: crate::commands::label::LabelCommand::Add(args),
        } = cli.command
        else {
            panic!("expected label add command");
        };
        assert_eq!(args.task_ref, "default#1");
        assert_eq!(args.labels, ["backend", "api"]);
        assert!(args.create_missing);
    }

    #[test]
    fn parses_label_ontology_and_legacy_alias_roots() {
        let cli = Cli::try_parse_from(["kanban", "labels", "ontology", "apply", "atom"])
            .expect("label ontology alias should parse");
        let Command::Label {
            command:
                crate::commands::label::LabelCommand::Ontology {
                    command: crate::commands::label::ontology::LedgerCommand::Apply { .. },
                },
        } = cli.command
        else {
            panic!("expected label ontology apply atom command");
        };

        let cli = Cli::try_parse_from(["kanban", "label", "ontology", "list"])
            .expect("label ontology list should parse");
        assert!(matches!(
            cli.command,
            Command::Label {
                command: crate::commands::label::LabelCommand::Ontology {
                    command: crate::commands::label::ontology::LedgerCommand::Signals(_),
                },
            }
        ));
    }

    #[test]
    fn parses_vector_status_command() {
        let cli = Cli::try_parse_from(["kanban", "vector", "status"])
            .expect("vector status command should parse");
        let Command::Vector {
            command: crate::commands::vector::VectorCommand::Status(_),
        } = cli.command
        else {
            panic!("expected vector status command");
        };
    }
}
