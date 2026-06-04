use std::{net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use kanban_core::TaskStatus;
use kanban_sqlite::{
    CreateTask, DispatchOptions, FinishPolicy, TaskPatch, add_dependency, archive_task, block_task,
    claim_task, complete_task, create_task, dispatch_once, get_task, heartbeat_task, init_database,
    list_dependencies, list_events, list_runs, list_tasks, promote_task, reclaim_expired,
    remove_dependency, submit_review_task, unblock_task, update_task,
};

#[derive(Debug, Parser)]
#[command(name = "kb", version, about = "Local SQLite-backed Kanban work queue")]
struct Cli {
    #[arg(long, global = true)]
    db: Option<PathBuf>,
    #[arg(long, global = true, default_value = "default")]
    board: String,
    #[arg(long, global = true)]
    actor: Option<String>,
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Init {
        #[arg(long)]
        force: bool,
    },
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    Dep {
        #[command(subcommand)]
        command: DepCommand,
    },
    Events {
        task_ref: Option<String>,
    },
    Runs {
        task_ref: Option<String>,
    },
    Dispatch(DispatchArgs),
    Serve(ServeArgs),
}

#[derive(Debug, Subcommand)]
enum TaskCommand {
    Create(CreateArgs),
    List(ListArgs),
    Show {
        task_ref: String,
    },
    Update(UpdateArgs),
    Promote {
        task_ref: String,
    },
    Start(ClaimArgs),
    Claim(ClaimArgs),
    Heartbeat(HeartbeatArgs),
    Done(FinishArgs),
    Complete(FinishArgs),
    Review(FinishArgs),
    Block(BlockArgs),
    Unblock {
        task_ref: String,
    },
    Reclaim(ReclaimArgs),
    Archive {
        task_ref: String,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Args)]
struct CreateArgs {
    title: String,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    assignee: Option<String>,
    #[arg(long, default_value_t = 0)]
    priority: i64,
    #[arg(long)]
    scheduled_at: Option<i64>,
    #[arg(long)]
    due_at: Option<i64>,
    #[arg(long, default_value = "{}")]
    metadata: String,
}

#[derive(Debug, Args)]
struct ListArgs {
    #[arg(long)]
    status: Vec<String>,
    #[arg(long)]
    include_archived: bool,
}

#[derive(Debug, Args)]
struct UpdateArgs {
    task_ref: String,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    assignee: Option<String>,
    #[arg(long)]
    clear_assignee: bool,
    #[arg(long)]
    priority: Option<i64>,
    #[arg(long)]
    scheduled_at: Option<i64>,
    #[arg(long)]
    clear_scheduled_at: bool,
    #[arg(long)]
    due_at: Option<i64>,
    #[arg(long)]
    clear_due_at: bool,
    #[arg(long)]
    metadata: Option<String>,
    #[arg(long)]
    expected_lock_version: Option<i64>,
}

#[derive(Debug, Args, Clone)]
struct ClaimArgs {
    task_ref: String,
    #[arg(long, default_value_t = 300_000)]
    ttl_ms: i64,
}

#[derive(Debug, Args, Clone)]
struct HeartbeatArgs {
    task_ref: String,
    #[arg(long)]
    claim_token: String,
    #[arg(long, default_value_t = 300_000)]
    ttl_ms: i64,
}

#[derive(Debug, Args, Clone)]
struct FinishArgs {
    task_ref: String,
    #[arg(long)]
    claim_token: Option<String>,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
struct BlockArgs {
    task_ref: String,
    reason: String,
    #[arg(long)]
    claim_token: Option<String>,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
struct ReclaimArgs {
    #[arg(long)]
    expired: bool,
}

#[derive(Debug, Subcommand)]
enum DepCommand {
    Add {
        parent_ref: String,
        child_ref: String,
    },
    Remove {
        parent_ref: String,
        child_ref: String,
    },
    List {
        task_ref: String,
    },
}

#[derive(Debug, Args)]
struct DispatchArgs {
    #[arg(long)]
    once: bool,
    #[arg(long, default_value = "sh -c 'true'")]
    command: String,
    #[arg(long, default_value = "default")]
    worker_profile: String,
    #[arg(long, default_value_t = 300_000)]
    claim_ttl_ms: i64,
    #[arg(long, default_value_t = 30_000)]
    heartbeat_interval_ms: i64,
    #[arg(long, value_enum, default_value_t = PolicyArg::Done)]
    on_success: PolicyArg,
    #[arg(long, value_enum, default_value_t = PolicyArg::Blocked)]
    on_failure: PolicyArg,
    #[arg(long)]
    log_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct ServeArgs {
    /// Host interface to bind. Defaults to localhost only; non-local hosts bind only when explicitly passed.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    /// TCP port to bind.
    #[arg(long, default_value_t = 8721)]
    port: u16,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PolicyArg {
    Done,
    Review,
    Blocked,
    Ready,
}

impl From<PolicyArg> for FinishPolicy {
    fn from(value: PolicyArg) -> Self {
        match value {
            PolicyArg::Done => Self::Done,
            PolicyArg::Review => Self::Review,
            PolicyArg::Blocked => Self::Blocked,
            PolicyArg::Ready => Self::Ready,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let db_path = cli.db.clone().unwrap_or_else(default_db_path);
    let actor = cli.actor.clone().unwrap_or_else(default_actor);
    match cli.command {
        Command::Init { force: _ } => {
            let result = init_database(&db_path, &actor)
                .with_context(|| format!("failed to initialize {}", db_path.display()))?;
            print_or_json(cli.json, &result, || {
                format!(
                    "Initialized kb database at {}\nDefault board: {}",
                    result.db_path.display(),
                    result.board_slug
                )
            })?;
        }
        Command::Task { command } => handle_task(command, &db_path, &cli.board, &actor, cli.json)?,
        Command::Dep { command } => handle_dep(command, &db_path, &cli.board, &actor, cli.json)?,
        Command::Events { task_ref } => {
            let events = list_events(&db_path, &cli.board, task_ref.as_deref())?;
            print_or_json(cli.json, &events, || {
                events
                    .iter()
                    .map(|e| format!("{} {} {:?}", e.id, e.kind, e.task_id))
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
        }
        Command::Runs { task_ref } => {
            let runs = list_runs(&db_path, &cli.board, task_ref.as_deref())?;
            print_or_json(cli.json, &runs, || {
                runs.iter()
                    .map(|r| {
                        format!(
                            "{} [{}] task={} exit={:?}",
                            r.id, r.status, r.task_id, r.exit_code
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
        }
        Command::Dispatch(args) => {
            if !args.once {
                bail!("v0.5 supports dispatch --once; run it from a scheduler/loop for now");
            }
            let log_dir = args
                .log_dir
                .unwrap_or_else(|| default_log_dir().join("runs"));
            let result = dispatch_once(
                &db_path,
                &cli.board,
                DispatchOptions {
                    actor,
                    command: args.command,
                    worker_profile: args.worker_profile,
                    claim_ttl_ms: args.claim_ttl_ms,
                    heartbeat_interval_ms: args.heartbeat_interval_ms,
                    on_success: args.on_success.into(),
                    on_failure: args.on_failure.into(),
                    log_dir,
                },
            )?;
            print_or_json(cli.json, &result, || {
                format!(
                    "claimed={} task={:?} exit={:?}",
                    result.claimed, result.task_id, result.exit_code
                )
            })?;
        }
        Command::Serve(args) => serve(args, db_path, actor)?,
    }
    Ok(())
}

fn serve(args: ServeArgs, db_path: PathBuf, actor: String) -> Result<()> {
    let _init = init_database(&db_path, &actor)
        .with_context(|| format!("failed to initialize/open {}", db_path.display()))?;
    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .with_context(|| format!("invalid bind address {}:{}", args.host, args.port))?;
    eprintln!(
        "Serving kb API on http://{addr} using {}",
        db_path.display()
    );
    let runtime = tokio::runtime::Runtime::new().context("failed to start tokio runtime")?;
    runtime
        .block_on(kanban_server::serve(
            addr,
            kanban_server::AppState::new(db_path, actor),
        ))
        .context("kb server failed")
}

fn handle_task(
    command: TaskCommand,
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    json: bool,
) -> Result<()> {
    match command {
        TaskCommand::Create(args) => {
            let task = create_task(
                db_path,
                board,
                actor,
                CreateTask {
                    title: args.title,
                    description: args.description,
                    status: args.status.as_deref().map(parse_status).transpose()?,
                    assignee: args.assignee,
                    priority: args.priority,
                    scheduled_at: args.scheduled_at,
                    due_at: args.due_at,
                    metadata_json: args.metadata,
                },
            )?;
            print_task(json, &task)?;
        }
        TaskCommand::List(args) => {
            let statuses = args
                .status
                .iter()
                .map(|s| parse_status(s))
                .collect::<Result<Vec<_>>>()?;
            let tasks = list_tasks(db_path, board, &statuses, args.include_archived)?;
            print_or_json(json, &tasks, || {
                tasks.iter().map(task_line).collect::<Vec<_>>().join("\n")
            })?;
        }
        TaskCommand::Show { task_ref } => print_task(json, &get_task(db_path, board, &task_ref)?)?,
        TaskCommand::Update(args) => {
            let task = update_task(
                db_path,
                board,
                actor,
                &args.task_ref,
                TaskPatch {
                    title: args.title,
                    description: args.description.map(Some),
                    assignee: if args.clear_assignee {
                        Some(None)
                    } else {
                        args.assignee.map(Some)
                    },
                    priority: args.priority,
                    scheduled_at: optional_clearable(args.scheduled_at, args.clear_scheduled_at),
                    due_at: optional_clearable(args.due_at, args.clear_due_at),
                    metadata_json: args.metadata,
                    expected_lock_version: args.expected_lock_version,
                },
            )?;
            print_task(json, &task)?;
        }
        TaskCommand::Promote { task_ref } => {
            print_task(json, &promote_task(db_path, board, actor, &task_ref)?)?
        }
        TaskCommand::Start(args) | TaskCommand::Claim(args) => {
            let claim = claim_task(db_path, board, actor, &args.task_ref, args.ttl_ms)?;
            print_or_json(json, &claim, || {
                format!("Claimed {} token={}", claim.task.id, claim.claim_token)
            })?;
        }
        TaskCommand::Heartbeat(args) => {
            print_task(
                json,
                &heartbeat_task(
                    db_path,
                    board,
                    actor,
                    &args.task_ref,
                    &args.claim_token,
                    args.ttl_ms,
                )?,
            )?;
        }
        TaskCommand::Done(args) | TaskCommand::Complete(args) => print_task(
            json,
            &complete_task(
                db_path,
                board,
                actor,
                &args.task_ref,
                args.claim_token.as_deref(),
                args.force,
            )?,
        )?,
        TaskCommand::Review(args) => print_task(
            json,
            &submit_review_task(
                db_path,
                board,
                actor,
                &args.task_ref,
                args.claim_token.as_deref(),
                args.force,
            )?,
        )?,
        TaskCommand::Block(args) => print_task(
            json,
            &block_task(
                db_path,
                board,
                actor,
                &args.task_ref,
                &args.reason,
                args.claim_token.as_deref(),
                args.force,
            )?,
        )?,
        TaskCommand::Unblock { task_ref } => {
            print_task(json, &unblock_task(db_path, board, actor, &task_ref)?)?
        }
        TaskCommand::Reclaim(args) => {
            let _expired_only = args.expired;
            let count = reclaim_expired(db_path, board, actor)?;
            print_or_json(json, &serde_json::json!({"reclaimed": count}), || {
                format!("Reclaimed {count} task(s)")
            })?;
        }
        TaskCommand::Archive { task_ref, force } => print_task(
            json,
            &archive_task(db_path, board, actor, &task_ref, force)?,
        )?,
    }
    Ok(())
}

fn handle_dep(
    command: DepCommand,
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    json: bool,
) -> Result<()> {
    match command {
        DepCommand::Add {
            parent_ref,
            child_ref,
        } => {
            add_dependency(db_path, board, actor, &parent_ref, &child_ref)?;
            print_or_json(
                json,
                &serde_json::json!({"parent": parent_ref, "child": child_ref}),
                || "Dependency added".into(),
            )?;
        }
        DepCommand::Remove {
            parent_ref,
            child_ref,
        } => {
            remove_dependency(db_path, board, actor, &parent_ref, &child_ref)?;
            print_or_json(
                json,
                &serde_json::json!({"parent": parent_ref, "child": child_ref}),
                || "Dependency removed".into(),
            )?;
        }
        DepCommand::List { task_ref } => {
            let deps = list_dependencies(db_path, board, &task_ref)?;
            print_or_json(json, &deps, || {
                deps.iter()
                    .map(|(p, c)| format!("{} -> {}", p, c))
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
        }
    }
    Ok(())
}

fn print_task(json: bool, task: &kanban_sqlite::TaskRecord) -> Result<()> {
    print_or_json(json, task, || task_line(task))
}

fn print_or_json<T: serde::Serialize>(
    json: bool,
    data: &T,
    human: impl FnOnce() -> String,
) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({"data": data}))?
        );
    } else {
        println!("{}", human());
    }
    Ok(())
}

fn task_line(task: &kanban_sqlite::TaskRecord) -> String {
    format!(
        "#{} {} [{}] {}",
        task.seq,
        task.id,
        task.status.as_str(),
        task.title
    )
}

fn parse_status(value: &str) -> Result<TaskStatus> {
    TaskStatus::try_from(value).map_err(|err| anyhow::anyhow!(err))
}

fn optional_clearable<T>(value: Option<T>, clear: bool) -> Option<Option<T>> {
    if clear { Some(None) } else { value.map(Some) }
}

fn default_db_path() -> PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kb")
        .join("kb.db")
}

fn default_log_dir() -> PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kb")
        .join("logs")
}

fn default_actor() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "local".to_owned())
}
