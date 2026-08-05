use std::{
    collections::BTreeMap,
    env,
    io::Read,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    process::ExitCode,
    str::FromStr,
};

use clap::{Args, Parser, Subcommand, ValueEnum};
use kanban_client::{ClientError, DEFAULT_SERVER_URL, KanbanClient};
use kanban_contract::{
    ApiCreateTaskStatus, ApiDependencies, ApiDependencyEdge, ApiDependencyTask, ApiTask,
    ApiTaskPriority, ApiTaskStatus, BlockTaskRequest, BlockTaskResponse, ClaimTaskRequest,
    ClaimTaskResponse, CommentAuthorType, CommentKind, CompleteTaskRequest, CompleteTaskResponse,
    CreateCommentRequest, CreateStepRequest, CreateStepResponse, CreateTaskRequest,
    CreateTaskResponse, GetTaskResponse, HeartbeatTaskRequest, HeartbeatTaskResponse,
    ListStepsResponse, ListTasksQuery, MarkExecutionPlanNotRequiredRequest,
    MarkExecutionPlanNotRequiredResponse, PromoteTaskRequest, PromoteTaskResponse,
    ReleaseTaskRequest, ReleaseTaskResponse, SubmitReviewTaskRequest, SubmitReviewTaskResponse,
    TaskReadPlanFilter, TaskReadSort, UpdateStepRequest, UpdateStepResponse,
};
use serde::Serialize;

const MAX_TEXT_INPUT_BYTES: usize = 1024 * 1024;

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
    /// Manage task comments through the canonical localhost host.
    Comment {
        #[command(subcommand)]
        command: CommentCommand,
    },
    /// Manage task dependencies through the canonical localhost host.
    #[command(name = "dep", visible_alias = "dependency")]
    Dependency {
        #[command(subcommand)]
        command: DependencyCommand,
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
    /// Enable the in-process single-worker dispatcher with a strict TOML profile.
    #[arg(long)]
    dispatcher_profile: Option<PathBuf>,
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
enum CommentCommand {
    /// Add one note or decision comment to a task.
    Add(CommentAddArgs),
    /// List task comments from the canonical application host.
    List(CommentListArgs),
}

#[derive(Debug, Subcommand)]
enum DependencyCommand {
    /// Add a parent dependency to a child task.
    Add(DependencyAddArgs),
    /// List direct parent and child dependencies for a task.
    List(DependencyListArgs),
}

#[derive(Debug, Args)]
struct DependencyAddArgs {
    child_task_ref: String,
    parent_task_ref: String,
}

#[derive(Debug, Args)]
struct DependencyListArgs {
    task_ref: String,
}

#[derive(Debug, Args)]
struct CommentAddArgs {
    task_ref: String,
    body: String,
    #[arg(long, value_enum)]
    kind: Option<CommentKindArg>,
    #[arg(long)]
    author: Option<String>,
    #[arg(long, value_enum)]
    author_type: Option<CommentAuthorTypeArg>,
    #[arg(long)]
    agent_type: Option<String>,
    /// JSON object stored as comment metadata.
    #[arg(long = "metadata-json")]
    metadata_json: Option<String>,
    #[arg(long)]
    idempotency_key: Option<String>,
}

#[derive(Debug, Args)]
struct CommentListArgs {
    task_ref: String,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CommentKindArg {
    Note,
    Decision,
    Signal,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CommentAuthorTypeArg {
    User,
    Agent,
}

#[derive(Debug, Subcommand)]
enum TaskCommand {
    /// Create a task through the shared application service.
    Create(TaskCreateArgs),
    /// List tasks through the shared application service.
    List(TaskListArgs),
    /// Show a task resolved by global id or board-local reference.
    Show(TaskShowArgs),
    /// Manage a task's execution plan.
    Step {
        #[command(subcommand)]
        command: TaskStepCommand,
    },
    /// Promote an eligible todo or due scheduled task to ready.
    Promote(TaskRefArgs),
    /// Atomically claim a ready task and start a run.
    Claim(TaskClaimArgs),
    /// Extend the active claim lease with a matching token.
    Heartbeat(TaskHeartbeatArgs),
    /// Return an actively claimed task to ready.
    Release(TaskReleaseArgs),
    /// Finish the active run and submit the task for review.
    Review(TaskReviewArgs),
    /// Complete a running or reviewed task.
    #[command(visible_alias = "complete")]
    Done(TaskDoneArgs),
    /// Block an active task with a required reason.
    Block(TaskBlockArgs),
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

#[derive(Debug, Subcommand)]
enum TaskStepCommand {
    /// Add a todo step to a task execution plan.
    Add(TaskStepAddArgs),
    /// List the task execution plan steps.
    List(TaskStepListArgs),
    /// Update editable execution-plan fields without changing step status.
    Update(TaskStepUpdateArgs),
    /// Mark this task as not requiring structured execution steps.
    NotRequired(TaskPlanNotRequiredArgs),
}

#[derive(Debug, Args)]
struct TaskStepAddArgs {
    task_ref: String,
    title: String,
    #[arg(long)]
    body: Option<String>,
    #[arg(long = "link-task")]
    linked_task_ref: Option<String>,
    #[arg(long)]
    position: Option<i64>,
    #[arg(long, conflicts_with = "optional")]
    required: bool,
    #[arg(long, conflicts_with = "required")]
    optional: bool,
    #[arg(long)]
    idempotency_key: Option<String>,
}

#[derive(Debug, Args)]
struct TaskStepListArgs {
    task_ref: String,
}

#[derive(Debug, Args)]
struct TaskStepUpdateArgs {
    task_ref: String,
    step_ref: String,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    body: Option<String>,
    #[arg(long = "link-task", conflicts_with = "unlink_task")]
    linked_task_ref: Option<String>,
    #[arg(long, conflicts_with = "linked_task_ref")]
    unlink_task: bool,
    #[arg(long)]
    position: Option<i64>,
    #[arg(long, conflicts_with = "optional")]
    required: bool,
    #[arg(long, conflicts_with = "required")]
    optional: bool,
}

#[derive(Debug, Args)]
struct TaskPlanNotRequiredArgs {
    task_ref: String,
    #[arg(long)]
    reason: String,
}

#[derive(Debug, Args)]
struct TaskRefArgs {
    task_ref: String,
}

#[derive(Debug, Args)]
struct TaskClaimArgs {
    task_ref: String,
    #[arg(long, default_value_t = 300_000)]
    ttl_ms: i64,
}

#[derive(Debug, Args)]
struct TaskHeartbeatArgs {
    task_ref: String,
    #[arg(long)]
    claim_token: String,
    #[arg(long, default_value_t = 300_000)]
    ttl_ms: i64,
    #[arg(long)]
    note: Option<String>,
}

#[derive(Debug, Args)]
struct TaskReleaseArgs {
    task_ref: String,
    #[arg(long)]
    claim_token: String,
}

#[derive(Debug, Args)]
struct TaskReviewArgs {
    task_ref: String,
    #[arg(long)]
    claim_token: Option<String>,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
struct TaskDoneArgs {
    task_ref: String,
    #[arg(long)]
    claim_token: Option<String>,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
struct TaskBlockArgs {
    task_ref: String,
    #[arg(
        required_unless_present = "reason_file",
        conflicts_with = "reason_file"
    )]
    reason: Option<String>,
    #[arg(
        long,
        value_name = "PATH|->",
        required_unless_present = "reason",
        conflicts_with = "reason"
    )]
    reason_file: Option<PathBuf>,
    #[arg(long)]
    claim_token: Option<String>,
    #[arg(long)]
    force: bool,
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
            "invalid_transition"
            | "execution_plan_required"
            | "steps_incomplete"
            | "dependency_cycle" => 4,
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
        Command::Comment { command } => {
            let client = KanbanClient::new(&cli.server_url, actor(cli))?;
            match command {
                CommentCommand::Add(args) => {
                    let metadata = parse_metadata(args.metadata_json.as_deref())?;
                    let comment = client.create_comment_by_selector(
                        &cli.board,
                        &args.task_ref,
                        &CreateCommentRequest {
                            idempotency_key: args.idempotency_key.clone(),
                            author: args.author.clone(),
                            body: args.body.clone(),
                            kind: args.kind.map(api_comment_kind),
                            author_type: args.author_type.map(api_comment_author_type),
                            agent_type: args.agent_type.clone(),
                            metadata: metadata.map(|metadata| {
                                serde_json::Value::Object(metadata.into_iter().collect())
                            }),
                        },
                    )?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string(&kanban_contract::CliCommentAddOutput::new(
                                comment,
                            ))
                            .expect("comment response is serializable")
                        );
                    } else {
                        println!(
                            "{} task={} created_at={} [{}] {} ({}): {}",
                            comment.id,
                            comment.task_id,
                            comment.created_at,
                            comment.kind.as_str(),
                            comment.author,
                            comment.author_type.as_str(),
                            comment.body
                        );
                    }
                }
                CommentCommand::List(args) => {
                    let comments = client.list_comments_by_selector(&cli.board, &args.task_ref)?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string(&kanban_contract::CliCommentListOutput::new(
                                comments,
                            ))
                            .expect("comment list response is serializable")
                        );
                    } else {
                        for comment in comments {
                            println!(
                                "{} task={} created_at={} [{}] {} ({}): {}",
                                comment.id,
                                comment.task_id,
                                comment.created_at,
                                comment.kind.as_str(),
                                comment.author,
                                comment.author_type.as_str(),
                                comment.body
                            );
                        }
                    }
                }
            }
            Ok(())
        }
        Command::Dependency { command } => {
            let client = KanbanClient::new(&cli.server_url, actor(cli))?;
            match command {
                DependencyCommand::Add(args) => {
                    let parent_id = client
                        .get_task_by_selector(&cli.board, &args.parent_task_ref)?
                        .id;
                    let dependencies = client.add_dependency_by_selector(
                        &cli.board,
                        &args.child_task_ref,
                        &args.parent_task_ref,
                    )?;
                    if cli.json {
                        let edge = dependencies
                            .edges
                            .iter()
                            .find(|edge| {
                                edge.child.id == dependencies.task.id && edge.parent.id == parent_id
                            })
                            .cloned()
                            .ok_or_else(|| CliFailure {
                                code: "invalid_response",
                                message: "dependency add response omitted the new edge".to_owned(),
                                exit_code: 2,
                            })?;
                        println!(
                            "{}",
                            serde_json::to_string(&kanban_contract::CliDependencyAddOutput {
                                data: kanban_contract::CliDependencyMutation {
                                    edge: cli_dependency_edge(&edge),
                                    dependencies: cli_dependency_snapshot(&dependencies),
                                },
                            })
                            .expect("dependency response is serializable")
                        );
                    } else {
                        println!(
                            "{} depends_on {} ({})",
                            dependencies.task.task_ref,
                            args.parent_task_ref,
                            dependencies.task.status.as_str()
                        );
                    }
                }
                DependencyCommand::List(args) => {
                    let dependencies =
                        client.list_dependencies_by_selector(&cli.board, &args.task_ref)?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string(&kanban_contract::CliDependencyListOutput {
                                data: cli_dependency_snapshot(&dependencies),
                            })
                            .expect("dependency response is serializable")
                        );
                    } else {
                        println!("{}", dependencies.task.task_ref);
                        for parent in &dependencies.parents {
                            println!("  parent {} {}", parent.task_ref, parent.status.as_str());
                        }
                        for child in &dependencies.children {
                            println!("  child {} {}", child.task_ref, child.status.as_str());
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
                TaskCommand::Step { command } => match command {
                    TaskStepCommand::Add(args) => {
                        let steps = client.create_step_by_selector(
                            &cli.board,
                            &args.task_ref,
                            &CreateStepRequest {
                                idempotency_key: args.idempotency_key.clone(),
                                title: args.title.clone(),
                                body: args.body.clone(),
                                linked_task_ref: args.linked_task_ref.clone(),
                                position: args.position,
                                required: !args.optional,
                                actor: None,
                            },
                        )?;
                        if cli.json {
                            println!(
                                "{}",
                                serde_json::to_string(&CreateStepResponse { data: steps })
                                    .expect("step create response is serializable")
                            );
                        } else {
                            for (index, step) in steps.steps.iter().enumerate() {
                                println!("S{} {} {}", index + 1, step.status.as_str(), step.title);
                            }
                        }
                    }
                    TaskStepCommand::List(args) => {
                        let steps = client.list_steps_by_selector(&cli.board, &args.task_ref)?;
                        if cli.json {
                            println!(
                                "{}",
                                serde_json::to_string(&ListStepsResponse { data: steps })
                                    .expect("step list response is serializable")
                            );
                        } else {
                            for (index, step) in steps.steps.iter().enumerate() {
                                println!("S{} {} {}", index + 1, step.status.as_str(), step.title);
                            }
                        }
                    }
                    TaskStepCommand::Update(args) => {
                        let steps = client.update_step_by_selector(
                            &cli.board,
                            &args.task_ref,
                            &args.step_ref,
                            &UpdateStepRequest {
                                title: args.title.clone(),
                                body: args.body.clone(),
                                linked_task_ref: args.linked_task_ref.clone(),
                                unlink_task: args.unlink_task,
                                position: args.position,
                                required: if args.required {
                                    Some(true)
                                } else if args.optional {
                                    Some(false)
                                } else {
                                    None
                                },
                                actor: None,
                            },
                        )?;
                        if cli.json {
                            println!(
                                "{}",
                                serde_json::to_string(&UpdateStepResponse { data: steps })
                                    .expect("step update response is serializable")
                            );
                        } else {
                            for (index, step) in steps.steps.iter().enumerate() {
                                println!("S{} {} {}", index + 1, step.status.as_str(), step.title);
                            }
                        }
                    }
                    TaskStepCommand::NotRequired(args) => {
                        let plan = client.mark_execution_plan_not_required_by_selector(
                            &cli.board,
                            &args.task_ref,
                            &MarkExecutionPlanNotRequiredRequest {
                                reason: args.reason.clone(),
                                actor: None,
                            },
                        )?;
                        if cli.json {
                            println!(
                                "{}",
                                serde_json::to_string(&MarkExecutionPlanNotRequiredResponse {
                                    data: plan,
                                })
                                .expect("execution plan response is serializable")
                            );
                        } else {
                            println!(
                                "{} {} {}",
                                plan.task_id,
                                plan.state.as_str(),
                                plan.reason.as_deref().unwrap_or("")
                            );
                        }
                    }
                },
                TaskCommand::Promote(args) => {
                    let task = client.promote_task_by_selector(
                        &cli.board,
                        &args.task_ref,
                        &PromoteTaskRequest { actor: None },
                    )?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string(&PromoteTaskResponse::new(task))
                                .expect("promote response is serializable")
                        );
                    } else {
                        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
                    }
                }
                TaskCommand::Claim(args) => {
                    let claim = client.claim_task_by_selector(
                        &cli.board,
                        &args.task_ref,
                        &ClaimTaskRequest {
                            actor: None,
                            ttl_ms: args.ttl_ms,
                            worker_profile: None,
                            metadata: None,
                        },
                    )?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string(&ClaimTaskResponse::new(claim))
                                .expect("claim response is serializable")
                        );
                    } else {
                        println!("Claimed {} token={}", claim.task.id, claim.claim_token);
                    }
                }
                TaskCommand::Heartbeat(args) => {
                    let task = client.heartbeat_task_by_selector(
                        &cli.board,
                        &args.task_ref,
                        &HeartbeatTaskRequest {
                            actor: None,
                            claim_token: args.claim_token.clone(),
                            ttl_ms: args.ttl_ms,
                            note: args.note.clone(),
                        },
                    )?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string(&HeartbeatTaskResponse::new(task))
                                .expect("heartbeat response is serializable")
                        );
                    } else {
                        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
                    }
                }
                TaskCommand::Release(args) => {
                    let task = client.release_task_by_selector(
                        &cli.board,
                        &args.task_ref,
                        &ReleaseTaskRequest {
                            actor: None,
                            claim_token: args.claim_token.clone(),
                        },
                    )?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string(&ReleaseTaskResponse::new(task))
                                .expect("release response is serializable")
                        );
                    } else {
                        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
                    }
                }
                TaskCommand::Review(args) => {
                    let task = client.submit_review_task_by_selector(
                        &cli.board,
                        &args.task_ref,
                        &SubmitReviewTaskRequest {
                            actor: None,
                            claim_token: args.claim_token.clone(),
                            force: args.force,
                            summary: None,
                        },
                    )?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string(&SubmitReviewTaskResponse::new(task))
                                .expect("review response is serializable")
                        );
                    } else {
                        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
                    }
                }
                TaskCommand::Done(args) => {
                    let task = client.complete_task_by_selector(
                        &cli.board,
                        &args.task_ref,
                        &CompleteTaskRequest {
                            actor: None,
                            claim_token: args.claim_token.clone(),
                            force: args.force,
                            summary: None,
                            result: None,
                        },
                    )?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string(&CompleteTaskResponse::new(task))
                                .expect("done response is serializable")
                        );
                    } else {
                        println!("{} {} {}", task.task_ref, task.status.as_str(), task.title);
                    }
                }
                TaskCommand::Block(args) => {
                    let reason = block_reason(args)?;
                    let task = client.block_task_by_selector(
                        &cli.board,
                        &args.task_ref,
                        &BlockTaskRequest {
                            actor: None,
                            reason,
                            claim_token: args.claim_token.clone(),
                            force: args.force,
                        },
                    )?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string(&BlockTaskResponse::new(task))
                                .expect("block response is serializable")
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

fn api_comment_kind(kind: CommentKindArg) -> CommentKind {
    match kind {
        CommentKindArg::Note => CommentKind::Note,
        CommentKindArg::Decision => CommentKind::Decision,
        CommentKindArg::Signal => CommentKind::Signal,
    }
}

fn api_comment_author_type(author_type: CommentAuthorTypeArg) -> CommentAuthorType {
    match author_type {
        CommentAuthorTypeArg::User => CommentAuthorType::User,
        CommentAuthorTypeArg::Agent => CommentAuthorType::Agent,
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

fn cli_dependency_task(task: &ApiTask) -> kanban_contract::CliDependencyTask {
    kanban_contract::CliDependencyTask {
        id: task.id.clone(),
        board_id: task.board_id.clone(),
        board_slug: task.board_slug.clone(),
        task_ref: task.task_ref.clone(),
        title: task.title.clone(),
        status: task.status,
    }
}

fn cli_dependency_task_compact(task: &ApiDependencyTask) -> kanban_contract::CliDependencyTask {
    kanban_contract::CliDependencyTask {
        id: task.id.clone(),
        board_id: task.board_id.clone(),
        board_slug: task.board_slug.clone(),
        task_ref: task.task_ref.clone(),
        title: task.title.clone(),
        status: task.status,
    }
}

fn cli_dependency_edge(edge: &ApiDependencyEdge) -> kanban_contract::CliDependencyEdge {
    kanban_contract::CliDependencyEdge {
        parent: cli_dependency_task_compact(&edge.parent),
        child: cli_dependency_task_compact(&edge.child),
    }
}

fn cli_dependency_snapshot(
    dependencies: &ApiDependencies,
) -> kanban_contract::CliDependencySnapshot {
    kanban_contract::CliDependencySnapshot {
        task: cli_dependency_task_compact(&dependencies.task),
        parents: dependencies
            .parents
            .iter()
            .map(cli_dependency_task)
            .collect(),
        children: dependencies
            .children
            .iter()
            .map(cli_dependency_task)
            .collect(),
        edges: dependencies.edges.iter().map(cli_dependency_edge).collect(),
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

fn block_reason(args: &TaskBlockArgs) -> Result<String, CliFailure> {
    let reason = match (&args.reason, &args.reason_file) {
        (Some(reason), None) => reason.clone(),
        (None, Some(path)) if path.as_os_str() == "-" => {
            let stdin = std::io::stdin();
            read_limited_text(stdin.lock(), "--reason-file -")?
        }
        (None, Some(path)) => {
            let file = std::fs::File::open(path).map_err(|error| CliFailure {
                code: "invalid_input",
                message: format!("failed to read --reason-file {}: {error}", path.display()),
                exit_code: 2,
            })?;
            read_limited_text(file, &format!("--reason-file {}", path.display()))?
        }
        _ => {
            return Err(CliFailure {
                code: "invalid_input",
                message: "block requires exactly one reason or --reason-file".to_owned(),
                exit_code: 2,
            });
        }
    };
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(CliFailure {
            code: "invalid_input",
            message: "block reason is required".to_owned(),
            exit_code: 2,
        });
    }
    Ok(reason.to_owned())
}

fn read_limited_text(reader: impl Read, label: &str) -> Result<String, CliFailure> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_TEXT_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| CliFailure {
            code: "invalid_input",
            message: format!("failed to read {label}: {error}"),
            exit_code: 2,
        })?;
    if bytes.len() > MAX_TEXT_INPUT_BYTES {
        return Err(CliFailure {
            code: "invalid_input",
            message: format!("{label} exceeds the 1 MiB input limit"),
            exit_code: 2,
        });
    }
    String::from_utf8(bytes).map_err(|error| CliFailure {
        code: "invalid_input",
        message: format!("{label} must be UTF-8: {error}"),
        exit_code: 2,
    })
}

async fn run_server(cli: &Cli, args: &ServeArgs) -> Result<(), CliFailure> {
    if !args.host.is_loopback() {
        return Err(CliFailure {
            code: "invalid_input",
            message: "kanban serve only accepts a loopback --host".to_owned(),
            exit_code: 2,
        });
    }
    let dispatcher =
        match args.dispatcher_profile.as_deref() {
            Some(path) => Some(kanban_server::DispatcherConfig::load(path).await.map_err(
                |error| CliFailure {
                    code: "invalid_input",
                    message: error.to_string(),
                    exit_code: 2,
                },
            )?),
            None => None,
        };
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
        "kanban serve listening on http://{addr}; database={}; dispatcher={}",
        db_path.display(),
        dispatcher
            .as_ref()
            .map(|config| config.board())
            .unwrap_or("disabled")
    );
    let (shutdown_tx, shutdown_rx) =
        tokio::sync::watch::channel(kanban_server::ShutdownSignal::Running);
    let signal_task = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_err() {
            return;
        }
        shutdown_tx
            .send(kanban_server::ShutdownSignal::Graceful)
            .ok();
        if tokio::signal::ctrl_c().await.is_err() {
            return;
        }
        shutdown_tx.send(kanban_server::ShutdownSignal::Force).ok();
    });
    let result =
        kanban_server::serve_with_dispatcher_shutdown(addr, state, dispatcher, shutdown_rx).await;
    signal_task.abort();
    result.map_err(|error| {
        if error.kind() == std::io::ErrorKind::Interrupted {
            CliFailure {
                code: "interrupted",
                message: error.to_string(),
                exit_code: 130,
            }
        } else {
            CliFailure {
                code: "server_error",
                message: error.to_string(),
                exit_code: 1,
            }
        }
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
    fn serve_dispatcher_is_opt_in_by_profile_path() {
        let disabled = Cli::try_parse_from(["kanban", "serve"]).expect("serve args");
        let Command::Serve(disabled) = disabled.command else {
            panic!("expected serve command");
        };
        assert_eq!(disabled.dispatcher_profile, None);

        let enabled =
            Cli::try_parse_from(["kanban", "serve", "--dispatcher-profile", "dispatcher.toml"])
                .expect("dispatcher serve args");
        let Command::Serve(enabled) = enabled.command else {
            panic!("expected serve command");
        };
        assert_eq!(
            enabled.dispatcher_profile.as_deref(),
            Some(std::path::Path::new("dispatcher.toml"))
        );
    }

    #[test]
    fn init_is_a_stable_unavailable_feature() {
        let failure = feature_not_available("not migrated");
        assert_eq!(failure.code, "feature_not_available");
        assert_eq!(failure.exit_code, 10);
    }

    #[test]
    fn parses_comment_add_command() {
        let cli = Cli::try_parse_from([
            "kanban",
            "comment",
            "add",
            "default#1",
            "handoff",
            "--kind",
            "decision",
            "--author-type",
            "agent",
            "--agent-type",
            "executor",
            "--metadata-json",
            "{\"options\":[]}",
        ])
        .expect("comment add args");
        let Command::Comment {
            command: CommentCommand::Add(args),
        } = cli.command
        else {
            panic!("expected comment add");
        };
        assert_eq!(args.task_ref, "default#1");
        assert_eq!(args.body, "handoff");
        assert!(matches!(args.kind, Some(CommentKindArg::Decision)));
        assert!(matches!(
            args.author_type,
            Some(CommentAuthorTypeArg::Agent)
        ));
    }

    #[test]
    fn parses_comment_list_command() {
        let cli = Cli::try_parse_from(["kanban", "comment", "list", "default#1"])
            .expect("comment list args");
        let Command::Comment {
            command: CommentCommand::List(args),
        } = cli.command
        else {
            panic!("expected comment list");
        };
        assert_eq!(args.task_ref, "default#1");
    }

    #[test]
    fn parses_dependency_commands() {
        let cli = Cli::try_parse_from(["kanban", "dep", "add", "default#2", "default#1"])
            .expect("dependency add args");
        let Command::Dependency {
            command: DependencyCommand::Add(args),
        } = cli.command
        else {
            panic!("expected dependency add");
        };
        assert_eq!(args.child_task_ref, "default#2");
        assert_eq!(args.parent_task_ref, "default#1");

        let cli = Cli::try_parse_from(["kanban", "dep", "list", "default#2"])
            .expect("dependency list args");
        let Command::Dependency {
            command: DependencyCommand::List(args),
        } = cli.command
        else {
            panic!("expected dependency list");
        };
        assert_eq!(args.task_ref, "default#2");
    }

    #[test]
    fn parses_execution_plan_not_required_command() {
        let cli = Cli::try_parse_from([
            "kanban",
            "task",
            "step",
            "not-required",
            "default#1",
            "--reason",
            "small task",
        ])
        .unwrap();
        let Command::Task {
            command:
                TaskCommand::Step {
                    command: TaskStepCommand::NotRequired(args),
                },
        } = cli.command
        else {
            panic!("expected task step not-required");
        };
        assert_eq!(args.task_ref, "default#1");
        assert_eq!(args.reason, "small task");
    }

    #[test]
    fn parses_task_step_update_command() {
        let cli = Cli::try_parse_from([
            "kanban",
            "task",
            "step",
            "update",
            "default#1",
            "S2",
            "--title",
            "Updated",
            "--position",
            "2048",
            "--optional",
        ])
        .expect("step update args");
        let Command::Task {
            command:
                TaskCommand::Step {
                    command: TaskStepCommand::Update(args),
                },
        } = cli.command
        else {
            panic!("expected task step update");
        };
        assert_eq!(args.task_ref, "default#1");
        assert_eq!(args.step_ref, "S2");
        assert_eq!(args.title.as_deref(), Some("Updated"));
        assert_eq!(args.position, Some(2048));
        assert!(args.optional);
    }

    #[test]
    fn parses_task_promote_command() {
        let cli =
            Cli::try_parse_from(["kanban", "task", "promote", "default#1"]).expect("promote args");
        let Command::Task {
            command: TaskCommand::Promote(args),
        } = cli.command
        else {
            panic!("expected task promote");
        };
        assert_eq!(args.task_ref, "default#1");
    }

    #[test]
    fn parses_task_claim_command() {
        let cli =
            Cli::try_parse_from(["kanban", "task", "claim", "default#1", "--ttl-ms", "120000"])
                .expect("claim args");
        let Command::Task {
            command: TaskCommand::Claim(args),
        } = cli.command
        else {
            panic!("expected task claim");
        };
        assert_eq!(args.task_ref, "default#1");
        assert_eq!(args.ttl_ms, 120_000);
    }

    #[test]
    fn parses_task_heartbeat_command() {
        let cli = Cli::try_parse_from([
            "kanban",
            "task",
            "heartbeat",
            "default#1",
            "--claim-token",
            "claim_test",
            "--ttl-ms",
            "120000",
            "--note",
            "alive",
        ])
        .expect("heartbeat args");
        let Command::Task {
            command: TaskCommand::Heartbeat(args),
        } = cli.command
        else {
            panic!("expected task heartbeat");
        };
        assert_eq!(args.task_ref, "default#1");
        assert_eq!(args.claim_token, "claim_test");
        assert_eq!(args.ttl_ms, 120_000);
        assert_eq!(args.note.as_deref(), Some("alive"));
    }

    #[test]
    fn parses_task_release_command() {
        let cli = Cli::try_parse_from([
            "kanban",
            "task",
            "release",
            "default#1",
            "--claim-token",
            "claim_test",
        ])
        .expect("release args");
        let Command::Task {
            command: TaskCommand::Release(args),
        } = cli.command
        else {
            panic!("expected task release");
        };
        assert_eq!(args.task_ref, "default#1");
        assert_eq!(args.claim_token, "claim_test");
    }

    #[test]
    fn parses_task_review_command() {
        let cli = Cli::try_parse_from([
            "kanban",
            "task",
            "review",
            "default#1",
            "--claim-token",
            "claim_test",
        ])
        .expect("review args");
        let Command::Task {
            command: TaskCommand::Review(args),
        } = cli.command
        else {
            panic!("expected task review");
        };
        assert_eq!(args.task_ref, "default#1");
        assert_eq!(args.claim_token.as_deref(), Some("claim_test"));
        assert!(!args.force);

        let cli = Cli::try_parse_from(["kanban", "task", "review", "t_global", "--force"])
            .expect("forced review args");
        let Command::Task {
            command: TaskCommand::Review(args),
        } = cli.command
        else {
            panic!("expected forced task review");
        };
        assert_eq!(args.claim_token, None);
        assert!(args.force);
    }

    #[test]
    fn parses_task_done_and_complete_commands() {
        for command in ["done", "complete"] {
            let cli = Cli::try_parse_from([
                "kanban",
                "task",
                command,
                "default#1",
                "--claim-token",
                "claim_test",
            ])
            .expect("done args");
            let Command::Task {
                command: TaskCommand::Done(args),
            } = cli.command
            else {
                panic!("expected task done");
            };
            assert_eq!(args.task_ref, "default#1");
            assert_eq!(args.claim_token.as_deref(), Some("claim_test"));
            assert!(!args.force);
        }
    }

    #[test]
    fn task_done_output_contract() {
        let fixture = include_str!("../../../schemas/fixtures/cli/task-done-output.v1.valid.json");
        let output: kanban_contract::CliTaskDoneOutput = serde_json::from_str(fixture).unwrap();
        assert_eq!(output.data.status.as_str(), "done");
        assert_eq!(
            serde_json::to_value(CompleteTaskResponse::new(output.data.clone())).unwrap(),
            serde_json::from_str::<serde_json::Value>(fixture).unwrap()
        );
    }

    #[test]
    fn task_complete_output_contract() {
        let fixture =
            include_str!("../../../schemas/fixtures/cli/task-complete-output.v1.valid.json");
        let output: kanban_contract::CliTaskCompleteOutput = serde_json::from_str(fixture).unwrap();
        assert_eq!(output.data.status.as_str(), "done");
        assert_eq!(
            serde_json::to_value(CompleteTaskResponse::new(output.data.clone())).unwrap(),
            serde_json::from_str::<serde_json::Value>(fixture).unwrap()
        );
    }

    #[test]
    fn parses_task_block_command() {
        let cli = Cli::try_parse_from([
            "kanban",
            "task",
            "block",
            "default#1",
            "waiting",
            "--claim-token",
            "claim_test",
        ])
        .expect("block args");
        let Command::Task {
            command: TaskCommand::Block(args),
        } = cli.command
        else {
            panic!("expected task block");
        };
        assert_eq!(args.task_ref, "default#1");
        assert_eq!(block_reason(&args).unwrap(), "waiting");
        assert_eq!(args.claim_token.as_deref(), Some("claim_test"));
        assert!(!args.force);
    }

    #[test]
    fn block_reason_file_input_is_bounded() {
        let accepted = read_limited_text(
            std::io::Cursor::new(vec![b'x'; MAX_TEXT_INPUT_BYTES]),
            "reason",
        )
        .unwrap();
        assert_eq!(accepted.len(), MAX_TEXT_INPUT_BYTES);
        let error = read_limited_text(
            std::io::Cursor::new(vec![b'x'; MAX_TEXT_INPUT_BYTES + 1]),
            "reason",
        )
        .unwrap_err();
        assert_eq!(error.code, "invalid_input");
        assert!(error.message.contains("1 MiB"));
    }

    #[test]
    fn task_block_output_contract() {
        let fixture = include_str!("../../../schemas/fixtures/cli/task-block-output.v1.valid.json");
        let output: kanban_contract::CliTaskBlockOutput = serde_json::from_str(fixture).unwrap();
        assert_eq!(output.data.status.as_str(), "blocked");
        assert_eq!(
            serde_json::to_value(BlockTaskResponse::new(output.data.clone())).unwrap(),
            serde_json::from_str::<serde_json::Value>(fixture).unwrap()
        );
    }
}
