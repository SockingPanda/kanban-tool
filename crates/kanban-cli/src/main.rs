use std::{
    collections::BTreeMap,
    env,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    process::ExitCode,
    str::FromStr,
};

use clap::{Args, Parser, Subcommand, ValueEnum};
use kanban_client::{ClientError, DEFAULT_SERVER_URL, KanbanClient};
use kanban_contract::{
    ApiCreateTaskStatus, ApiTaskPriority, ApiTaskStatus, CreateTaskRequest, CreateTaskResponse,
    GetTaskResponse, ListTasksQuery, TaskReadPlanFilter, TaskReadSort,
};
use serde::Serialize;

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
    Serve(ServeArgs),
    /// Query boards through the localhost application host.
    Board {
        #[command(subcommand)]
        command: BoardCommand,
    },
    /// Manage tasks through the canonical localhost host.
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    /// Removed direct-database initialization path.
    Init,
    /// Commands not yet migrated to the canonical host fail without touching storage.
    #[command(external_subcommand)]
    FeatureNotAvailable(Vec<String>),
}

#[derive(Debug, Args)]
struct ServeArgs {
    /// Canonical Turso database owned by this host.
    #[arg(long, env = "KANBAN_DB")]
    db: Option<PathBuf>,
    /// Loopback address to listen on.
    #[arg(long, default_value_t = IpAddr::V4(Ipv4Addr::LOCALHOST))]
    host: IpAddr,
    /// Local HTTP port.
    #[arg(long, default_value_t = 8721)]
    port: u16,
}

#[derive(Debug, Subcommand)]
enum BoardCommand {
    /// List boards from the canonical application service.
    List {
        #[arg(long)]
        include_archived: bool,
    },
    /// List a board's fixed status columns.
    Columns { board: Option<String> },
}

#[derive(Debug, Subcommand)]
enum TaskCommand {
    /// Create a task through the shared application service.
    Create(TaskCreateArgs),
    /// List tasks through the shared application service.
    List(TaskListArgs),
    /// Show a task resolved by global id or board-local reference.
    Show(TaskShowArgs),
}

#[derive(Debug, Args)]
struct TaskCreateArgs {
    title: String,
    #[arg(long)]
    description: Option<String>,
    #[arg(long, value_enum)]
    status: Option<CreateStatus>,
    #[arg(long)]
    assignee: Option<String>,
    #[arg(long, default_value_t = 3)]
    priority: i64,
    #[arg(long)]
    scheduled_at: Option<i64>,
    #[arg(long)]
    due_at: Option<i64>,
    #[arg(long)]
    max_retries: Option<i64>,
    /// JSON object stored as task metadata.
    #[arg(long)]
    metadata: Option<String>,
    /// Stable retry key scoped to this board.
    #[arg(long)]
    idempotency_key: Option<String>,
    /// Optional client-selected typed task id.
    #[arg(long)]
    task_id: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CreateStatus {
    Triage,
    Todo,
    Scheduled,
    Ready,
}

#[derive(Debug, Args)]
struct TaskListArgs {
    #[arg(long, value_enum)]
    status: Vec<ListStatus>,
    #[arg(long)]
    priority: Vec<i64>,
    #[arg(long = "plan-filter")]
    plan_filter: Vec<String>,
    #[arg(long)]
    assignee: Option<String>,
    #[arg(long = "query", alias = "search")]
    query: Option<String>,
    #[arg(long)]
    include_archived: bool,
    #[arg(long, default_value_t = 100)]
    limit: usize,
    #[arg(long, default_value_t = 0)]
    offset: usize,
    #[arg(long, default_value = "position")]
    sort: String,
}

#[derive(Debug, Args)]
struct TaskShowArgs {
    task_ref: String,
    /// Ontology details are intentionally unavailable on the single-host path.
    #[arg(long)]
    details: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ListStatus {
    Triage,
    Todo,
    Scheduled,
    Ready,
    Running,
    Blocked,
    Review,
    Done,
    Archived,
}

#[derive(Debug, Serialize)]
struct CliErrorBody<'a> {
    code: &'a str,
    message: String,
    exit_code: u8,
}

#[derive(Debug, Serialize)]
struct CliErrorEnvelope<'a> {
    error: CliErrorBody<'a>,
}

#[derive(Debug)]
struct CliFailure {
    code: &'static str,
    message: String,
    exit_code: u8,
}

impl From<ClientError> for CliFailure {
    fn from(error: ClientError) -> Self {
        let code = error.code();
        let exit_code = match code {
            "not_found" => 3,
            "invalid_transition" | "execution_plan_required" | "steps_incomplete" => 4,
            "claim_conflict" | "claim_token_mismatch" | "idempotency_conflict" => 5,
            "dependency_blocked" => 6,
            "server_unavailable" => 9,
            "feature_not_available" => 10,
            "invalid_input" | "invalid_response" => 2,
            _ => 1,
        };
        Self {
            code,
            message: error.to_string(),
            exit_code,
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string(&CliErrorEnvelope {
                        error: CliErrorBody {
                            code: error.code,
                            message: error.message,
                            exit_code: error.exit_code,
                        },
                    })
                    .expect("CLI error envelope is serializable")
                );
            } else {
                eprintln!("{}: {}", error.code, error.message);
            }
            ExitCode::from(error.exit_code)
        }
    }
}

async fn run(cli: &Cli) -> Result<(), CliFailure> {
    match &cli.command {
        Command::Serve(args) => run_server(cli, args).await,
        Command::Board { command } => {
            let client = KanbanClient::new(&cli.server_url, actor(cli))?;
            match command {
                BoardCommand::List { include_archived } => {
                    let boards = client.list_boards(*include_archived)?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string(&kanban_contract::ListBoardsResponse {
                                data: boards
                            })
                            .expect("board response is serializable")
                        );
                    } else {
                        for board in boards {
                            println!("{} {} {}", board.id, board.slug, board.name);
                        }
                    }
                }
                BoardCommand::Columns { board } => {
                    let board = board.as_deref().unwrap_or(&cli.board);
                    let columns = client.list_board_columns(board)?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string(&kanban_contract::ListBoardColumnsResponse {
                                data: columns
                            })
                            .expect("column response is serializable")
                        );
                    } else {
                        for column in columns {
                            println!(
                                "{} {}{}",
                                column.status.as_str(),
                                column.title,
                                if column.hidden { " (hidden)" } else { "" }
                            );
                        }
                    }
                }
            }
            Ok(())
        }
        Command::Task { command } => {
            let client = KanbanClient::new(&cli.server_url, actor(cli))?;
            match command {
                TaskCommand::Create(args) => {
                    let metadata = parse_metadata(args.metadata.as_deref())?;
                    let task = client.create_task(
                        &cli.board,
                        CreateTaskRequest {
                            task_id: args.task_id.clone(),
                            idempotency_key: args.idempotency_key.clone(),
                            title: args.title.clone(),
                            description: args.description.clone(),
                            status: args.status.map(api_create_status),
                            assignee: args.assignee.clone(),
                            priority: args.priority,
                            scheduled_at: args.scheduled_at,
                            due_at: args.due_at,
                            max_retries: args.max_retries,
                            metadata,
                            labels: Vec::new(),
                            depends_on: Vec::new(),
                            actor: None,
                        },
                    )?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string(&CreateTaskResponse { data: task })
                                .expect("task response is serializable")
                        );
                    } else {
                        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
                    }
                }
                TaskCommand::List(args) => {
                    let query = list_tasks_query(args)?;
                    let response = client.list_tasks(&cli.board, &query)?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string(&kanban_contract::CliTaskListOutput::new(
                                response.data,
                            ))
                            .expect("task list response is serializable")
                        );
                    } else {
                        for task in response.data {
                            println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
                        }
                    }
                }
                TaskCommand::Show(args) => {
                    if args.details {
                        return Err(feature_not_available(
                            "`task show --details` requires the deferred ontology projection",
                        ));
                    }
                    let task = client.get_task_by_selector(&cli.board, &args.task_ref)?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string(&GetTaskResponse::new(task, None))
                                .expect("task show response is serializable")
                        );
                    } else {
                        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
                    }
                }
            }
            Ok(())
        }
        Command::Init => Err(feature_not_available(
            "`kanban init` was removed; start `kanban serve` to initialize the canonical Turso database",
        )),
        Command::FeatureNotAvailable(parts) => Err(feature_not_available(format!(
            "command `{}` is not available on the single-host path yet",
            parts.join(" ")
        ))),
    }
}

fn api_create_status(status: CreateStatus) -> ApiCreateTaskStatus {
    match status {
        CreateStatus::Triage => ApiCreateTaskStatus::Triage,
        CreateStatus::Todo => ApiCreateTaskStatus::Todo,
        CreateStatus::Scheduled => ApiCreateTaskStatus::Scheduled,
        CreateStatus::Ready => ApiCreateTaskStatus::Ready,
    }
}

fn list_tasks_query(args: &TaskListArgs) -> Result<ListTasksQuery, CliFailure> {
    let priorities = args
        .priority
        .iter()
        .copied()
        .map(|value| {
            ApiTaskPriority::try_from(value).map_err(|value| CliFailure {
                code: "invalid_input",
                message: format!("priority must be between 0 and 3, got {value}"),
                exit_code: 2,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let plan_filter = args
        .plan_filter
        .iter()
        .map(|value| {
            TaskReadPlanFilter::from_str(value).map_err(|()| CliFailure {
                code: "invalid_input",
                message: format!("unsupported --plan-filter: {value}"),
                exit_code: 2,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let sort = TaskReadSort::from_str(&args.sort).map_err(|()| CliFailure {
        code: "invalid_input",
        message: format!("unsupported --sort: {}", args.sort),
        exit_code: 2,
    })?;
    Ok(ListTasksQuery {
        status: args.status.iter().copied().map(api_list_status).collect(),
        priority: priorities,
        label: Vec::new(),
        plan_filter,
        assignee: args.assignee.clone(),
        q: args.query.clone(),
        include_archived: args.include_archived,
        limit: args.limit,
        offset: args.offset,
        sort,
    })
}

fn api_list_status(status: ListStatus) -> ApiTaskStatus {
    match status {
        ListStatus::Triage => ApiTaskStatus::Triage,
        ListStatus::Todo => ApiTaskStatus::Todo,
        ListStatus::Scheduled => ApiTaskStatus::Scheduled,
        ListStatus::Ready => ApiTaskStatus::Ready,
        ListStatus::Running => ApiTaskStatus::Running,
        ListStatus::Blocked => ApiTaskStatus::Blocked,
        ListStatus::Review => ApiTaskStatus::Review,
        ListStatus::Done => ApiTaskStatus::Done,
        ListStatus::Archived => ApiTaskStatus::Archived,
    }
}

fn parse_metadata(
    metadata: Option<&str>,
) -> Result<Option<BTreeMap<String, serde_json::Value>>, CliFailure> {
    metadata
        .map(|metadata| {
            serde_json::from_str(metadata).map_err(|error| CliFailure {
                code: "invalid_input",
                message: format!("--metadata must be a JSON object: {error}"),
                exit_code: 2,
            })
        })
        .transpose()
}

async fn run_server(cli: &Cli, args: &ServeArgs) -> Result<(), CliFailure> {
    if !args.host.is_loopback() {
        return Err(CliFailure {
            code: "invalid_input",
            message: "kanban serve only accepts a loopback --host".to_owned(),
            exit_code: 2,
        });
    }
    let db_path = args.db.clone().unwrap_or_else(default_db_path);
    let state = kanban_server::AppState::open(&db_path, actor(cli))
        .await
        .map_err(|error| CliFailure {
            code: "storage_error",
            message: error.to_string(),
            exit_code: 1,
        })?;
    let addr = SocketAddr::new(args.host, args.port);
    eprintln!(
        "kanban serve listening on http://{addr}; database={}",
        db_path.display()
    );
    kanban_server::serve_with_shutdown(addr, state, async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
    .map_err(|error| CliFailure {
        code: "server_error",
        message: error.to_string(),
        exit_code: 1,
    })
}

fn actor(cli: &Cli) -> String {
    cli.actor
        .clone()
        .or_else(|| env::var("USER").ok())
        .or_else(|| env::var("USERNAME").ok())
        .unwrap_or_else(|| "local".to_owned())
}

fn default_db_path() -> PathBuf {
    dirs_next::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kb")
        .join("kanban.db")
}

fn feature_not_available(message: impl Into<String>) -> CliFailure {
    CliFailure {
        code: "feature_not_available",
        message: message.into(),
        exit_code: 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_database_uses_new_filename() {
        assert_eq!(
            default_db_path().file_name().and_then(|name| name.to_str()),
            Some("kanban.db")
        );
    }

    #[test]
    fn init_is_a_stable_unavailable_feature() {
        let failure = feature_not_available("not migrated");
        assert_eq!(failure.code, "feature_not_available");
        assert_eq!(failure.exit_code, 10);
    }
}
