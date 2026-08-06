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
    about = "基于本地 Turso 的 Kanban 工作队列",
    arg_required_else_help = true,
    after_help = "所有产品命令都通过 kanban serve；只有 `kanban serve` 会打开数据库。"
)]
struct Cli {
    /// canonical localhost 应用 host。
    #[arg(
        long,
        global = true,
        env = "KANBAN_SERVER_URL",
        default_value = DEFAULT_SERVER_URL
    )]
    server_url: String,
    /// board-scoped client 命令使用的看板 slug 或 ID。
    #[arg(long, global = true)]
    board: Option<String>,
    /// 仅供 `serve` 和配置检查使用的 canonical Turso 路径。
    #[arg(long, global = true)]
    db: Option<std::path::PathBuf>,
    /// 人类可读输出的 locale；JSON key 和枚举保持稳定。
    #[arg(long, global = true)]
    locale: Option<String>,
    /// 发送给应用 host 的审计 actor。
    #[arg(long, global = true, env = "KANBAN_ACTOR")]
    actor: Option<String>,
    /// 输出稳定的 JSON envelope。
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// 启动唯一允许打开 Turso 数据库的进程。
    Serve(server::ServeArgs),
    /// 通过 localhost 应用 host 查询看板。
    Board {
        #[command(subcommand)]
        command: commands::board::BoardCommand,
    },
    /// 不打开 Turso，选择并查看项目本地 active board。
    Config {
        #[command(subcommand)]
        command: commands::config::ConfigCommand,
    },
    /// 通过 canonical localhost host 管理任务。
    Task {
        #[command(subcommand)]
        command: commands::task::TaskCommand,
    },
    /// 通过 canonical host 管理看板 label 和任务 label 绑定。
    #[command(visible_alias = "labels", visible_alias = "ontology")]
    Label {
        #[command(subcommand)]
        command: commands::label::LabelCommand,
    },
    /// 通过 canonical localhost host 管理任务评论。
    Comment {
        #[command(subcommand)]
        command: commands::comment::CommentCommand,
    },
    /// 构建 task/reference/query 的只读混合上下文包。
    Context {
        #[command(subcommand)]
        command: commands::context::ContextCommand,
    },
    /// 通过 canonical localhost host 管理文件型任务附件。
    Attachment {
        #[command(subcommand)]
        command: commands::attachment::AttachmentCommand,
    },
    /// 通过 canonical localhost host 管理任务依赖。
    #[command(name = "dep", visible_alias = "dependency")]
    Dependency {
        #[command(subcommand)]
        command: commands::dependency::DependencyCommand,
    },
    /// 查看并维护 canonical graph entity。
    #[command(name = "entity", visible_alias = "entities")]
    Entity {
        #[command(subcommand)]
        command: commands::entities::EntityCommand,
    },
    /// 查询有界的 canonical task/entity graph。
    Graph {
        #[command(subcommand)]
        command: commands::graph::GraphCommand,
    },
    /// 通过 localhost host 列出 canonical 任务事件。
    Events(commands::event::ListArgs),
    /// 通过 canonical localhost host 列出任务的执行 run。
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
    /// 通过 canonical localhost host 查看一个执行 run。
    Run {
        #[command(subcommand)]
        command: commands::run::RunCommand,
    },
    /// 通过 canonical localhost host 搜索任务。
    Search(commands::search::SearchArgs),
    /// 查看并维护任务搜索 projection。
    Index {
        #[command(subcommand)]
        command: commands::index::IndexCommand,
    },
    /// 通过 canonical host 记录并审核通用 signal。
    Signal {
        #[command(subcommand)]
        command: commands::signal::SignalCommand,
    },
    /// 创建项目 `.kb/config.toml` 选择文件，不打开 Turso。
    Init {
        /// 幂等兼容 flag；init 从不重置已有用户设置。
        #[arg(long, help = "已弃用的兼容空操作；init 幂等且从不重置数据")]
        force: bool,
    },
    /// 生成 shell completion 脚本，不触碰配置或 Turso。
    Completions { shell: completion::Shell },
    /// 供生成的 Bash/Zsh 动态 completion helper 使用的隐藏协议。
    #[command(name = "__complete", hide = true)]
    Complete(completion::CompleteArgs),
    /// 安装或查看 Codex 生命周期 hook。
    Hook {
        #[command(subcommand)]
        command: commands::hook::HookCommand,
    },
    /// 管理 host 内 Ollama + Turso vector projection。
    Vector {
        #[command(subcommand)]
        command: commands::vector::VectorCommand,
    },
    /// 尚未迁移到 canonical host 的命令会失败，且不会触碰存储。
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
            "命令 `{}` 尚未在单 Host 路径上提供",
            parts.join(" ")
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use clap::{Command as ClapCommand, CommandFactory, Parser};
    use kanban_protocol::{ContractSurface, surface_operation_keys};

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

    #[test]
    fn clap_leaf_commands_match_exact_contract_catalog() {
        let mut actual = BTreeSet::new();
        collect_leaf_commands(&Cli::command(), &mut Vec::new(), &mut actual);
        let expected = surface_operation_keys(ContractSurface::Cli).collect::<BTreeSet<_>>();

        assert_eq!(
            actual, expected,
            "新增、删除或重命名 CLI leaf command 时必须同步精确 contract catalog"
        );
    }

    fn collect_leaf_commands(
        command: &ClapCommand,
        prefix: &mut Vec<String>,
        output: &mut BTreeSet<String>,
    ) {
        let subcommands = command.get_subcommands().collect::<Vec<_>>();
        if subcommands.is_empty() {
            if !prefix.is_empty() {
                output.insert(prefix.join(" "));
            }
            return;
        }

        for subcommand in subcommands {
            // `get_name` 是 Clap 的 canonical name；不把 visible/hidden alias
            // 当成第二个 leaf。`get_subcommands` 会保留 hidden command（例如
            // `__complete`），而 external subcommand 没有静态名字，因而不会被
            // 错误地伪造为 contract operation。
            prefix.push(subcommand.get_name().to_owned());
            collect_leaf_commands(subcommand, prefix, output);
            prefix.pop();
        }
    }
}
