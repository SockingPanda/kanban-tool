use std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use kanban_core::TaskStatus;
use kanban_sqlite::{
    CreateTask, DispatchOptions, FinishPolicy, TaskPatch, add_dependency, archive_task,
    backup_database, begin_database_replace, begin_database_runtime, block_task,
    checkpoint_database, claim_task, complete_task, create_task, dispatch_once, export_jsonl,
    get_run_by_id_global, get_task, heartbeat_task, import_jsonl, init_database, list_dependencies,
    list_events, list_runs, list_tasks, promote_task, queue_stats, reclaim_expired,
    remove_dependency, set_task_retry_policy_by_id, submit_review_task, unblock_task, update_task,
    vacuum_database,
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
    Run {
        #[command(subcommand)]
        command: RunCommand,
    },
    Dispatch(DispatchArgs),
    Serve(ServeArgs),
    Doctor,
    Stats,
    Backup(BackupArgs),
    Export(ExportArgs),
    Import(ImportArgs),
    Checkpoint,
    Vacuum,
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
    #[arg(long)]
    max_retries: Option<i64>,
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
    max_retries: Option<i64>,
    #[arg(long)]
    clear_max_retries: bool,
    #[arg(long)]
    metadata: Option<String>,
    #[arg(long)]
    expected_lock_version: Option<i64>,
}

#[derive(Debug, Subcommand)]
enum RunCommand {
    Show {
        run_id: String,
    },
    Logs {
        run_id: String,
        #[arg(long)]
        tail_bytes: Option<usize>,
    },
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
    #[arg(long)]
    profile_config: Option<PathBuf>,
    #[arg(long, default_value_t = 1_000)]
    poll_interval_ms: u64,
    #[arg(long)]
    max_iterations: Option<usize>,
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

#[derive(Debug, Args)]
struct BackupArgs {
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, Args)]
struct ExportArgs {
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value = "jsonl")]
    format: String,
}

#[derive(Debug, Args)]
struct ImportArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    replace: bool,
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

#[derive(Debug, serde::Serialize)]
struct DispatchLoopSummary {
    iterations: usize,
    claimed: usize,
    runs: Vec<kanban_sqlite::DispatchResult>,
}

#[derive(Debug, Clone)]
struct WorkerProfileConfig {
    command: Option<String>,
    claim_ttl_ms: Option<i64>,
    heartbeat_interval_ms: Option<i64>,
    on_success: Option<FinishPolicy>,
    on_failure: Option<FinishPolicy>,
    log_dir: Option<PathBuf>,
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
        Command::Run { command } => handle_run(command, &db_path, cli.json)?,
        Command::Dispatch(args) => {
            let options = dispatch_options(&args, actor.clone())?;
            if args.once {
                let result = dispatch_once(&db_path, &cli.board, options)?;
                print_or_json(cli.json, &result, || {
                    format!(
                        "claimed={} task={:?} exit={:?}",
                        result.claimed, result.task_id, result.exit_code
                    )
                })?;
            } else {
                let _runtime_guard = begin_database_runtime(&db_path)?;
                let summary = dispatch_loop(
                    &db_path,
                    &cli.board,
                    options,
                    args.poll_interval_ms,
                    args.max_iterations,
                )?;
                print_or_json(cli.json, &summary, || {
                    format!(
                        "iterations={} claimed={}",
                        summary.iterations, summary.claimed
                    )
                })?;
            }
        }
        Command::Serve(args) => serve(args, db_path, actor)?,
        Command::Doctor => {
            let report = kanban_sqlite::doctor_database(&db_path)?;
            print_or_json(cli.json, &report, || {
                format!(
                    "ok={} integrity={} migration={:?} user_version={} expired_running={} running_without_run={} orphan_running_runs={} dependency_cycles={} archived_dependency_edges={} missing_run_logs={} executable_dependency_violations={} executable_spec_violations={} executable_schedule_violations={}",
                    report.ok,
                    report.integrity_check,
                    report.migration_version,
                    report.user_version,
                    report.expired_running_tasks,
                    report.running_tasks_without_active_run,
                    report.orphan_running_runs,
                    report.dependency_cycles,
                    report.archived_dependency_edges,
                    report.missing_run_logs,
                    report.executable_dependency_violations,
                    report.executable_spec_violations,
                    report.executable_schedule_violations
                )
            })?;
        }
        Command::Stats => {
            let stats = queue_stats(&db_path, &cli.board)?;
            print_or_json(cli.json, &stats, || {
                let stale = stats.stale_claims.len();
                let blocked = stats
                    .blocked_reasons
                    .iter()
                    .map(|reason| format!("{}={}", reason.reason, reason.count))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("stale_claims={stale} blocked_reasons=[{blocked}]")
            })?;
        }
        Command::Backup(args) => {
            let result = backup_database(&db_path, args.out)?;
            print_or_json(cli.json, &result, || {
                format!("Backup written to {}", result.out_path.display())
            })?;
        }
        Command::Export(args) => {
            if args.format != "jsonl" {
                bail!("unsupported export format: {}", args.format);
            }
            let result = export_jsonl(&db_path, &cli.board, args.out)?;
            print_or_json(cli.json, &result, || {
                format!(
                    "Exported {} record(s) to {}",
                    result.records,
                    result.out_path.display()
                )
            })?;
        }
        Command::Import(args) => {
            if !args.input.is_file() {
                bail!("import input does not exist: {}", args.input.display());
            }
            if !args.replace {
                bail!("import requires --replace");
            }
            let result = import_command(&db_path, &actor, args)?;
            print_or_json(cli.json, &result, || {
                format!(
                    "Imported {} record(s) from {}",
                    result.records,
                    result.input_path.display()
                )
            })?;
        }
        Command::Checkpoint => {
            let result = checkpoint_database(&db_path)?;
            print_or_json(cli.json, &result, || {
                format!(
                    "checkpoint busy={} log_frames={} checkpointed_frames={}",
                    result.busy, result.log_frames, result.checkpointed_frames
                )
            })?;
        }
        Command::Vacuum => {
            let result = vacuum_database(&db_path)?;
            print_or_json(cli.json, &result, || "Vacuum complete".to_owned())?;
        }
    }
    Ok(())
}

fn serve(args: ServeArgs, db_path: PathBuf, actor: String) -> Result<()> {
    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .with_context(|| format!("invalid bind address {}:{}", args.host, args.port))?;
    if !addr.ip().is_loopback() {
        bail!("kb serve only supports loopback hosts; use 127.0.0.1 or ::1");
    }
    let _runtime_guard = begin_database_runtime(&db_path)?;
    let _init = init_database(&db_path, &actor)
        .with_context(|| format!("failed to initialize/open {}", db_path.display()))?;
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

fn import_command(
    db_path: &Path,
    actor: &str,
    args: ImportArgs,
) -> Result<kanban_sqlite::ImportResult> {
    let temp_path = temporary_import_db_path(db_path)?;
    let restore_path = temporary_restore_db_path(db_path)?;
    let replaced_path = temporary_replaced_db_path(db_path)?;
    let result = (|| {
        let _replace_guard = begin_database_replace(db_path)?;
        let _init = init_database(&temp_path, actor)
            .with_context(|| format!("failed to initialize/open {}", temp_path.display()))?;
        let result = import_jsonl(&temp_path, &args.input, args.replace)?;
        backup_database(&temp_path, &restore_path)?;
        replace_database_main_file(db_path, &restore_path, &replaced_path)?;
        Ok(result)
    })();
    remove_sqlite_file_family(&temp_path);
    remove_sqlite_file_family(&restore_path);
    result
}

fn temporary_import_db_path(db_path: &Path) -> Result<PathBuf> {
    temporary_sibling_db_path(db_path, "import")
}

fn temporary_restore_db_path(db_path: &Path) -> Result<PathBuf> {
    temporary_sibling_db_path(db_path, "restore")
}

fn temporary_replaced_db_path(db_path: &Path) -> Result<PathBuf> {
    temporary_sibling_db_path(db_path, "replaced")
}

fn temporary_sibling_db_path(db_path: &Path, label: &str) -> Result<PathBuf> {
    if let Some(parent) = db_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let file_name = db_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("kb.db");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?
        .as_nanos();
    Ok(db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            ".{file_name}.{label}.{}.{}.tmp",
            std::process::id(),
            nanos
        )))
}

fn remove_sqlite_file_family(path: &Path) {
    let _ = fs::remove_file(path);
    remove_sqlite_sidecars(path);
}

fn remove_sqlite_sidecars(path: &Path) {
    let _ = fs::remove_file(format!("{}-wal", path.display()));
    let _ = fs::remove_file(format!("{}-shm", path.display()));
}

fn replace_database_main_file(
    db_path: &Path,
    restore_path: &Path,
    replaced_path: &Path,
) -> Result<()> {
    remove_sqlite_sidecars(db_path);
    let had_existing = db_path.exists();
    if had_existing {
        fs::rename(db_path, replaced_path).with_context(|| {
            format!(
                "failed to move existing database {} out of the way",
                db_path.display()
            )
        })?;
    }
    if let Err(error) = fs::rename(restore_path, db_path) {
        if had_existing {
            let _ = fs::rename(replaced_path, db_path);
        }
        return Err(error).with_context(|| {
            format!(
                "failed to replace {} with restored import",
                db_path.display()
            )
        });
    }
    if had_existing {
        remove_sqlite_file_family(replaced_path);
    }
    remove_sqlite_sidecars(db_path);
    Ok(())
}

fn dispatch_options(args: &DispatchArgs, actor: String) -> Result<DispatchOptions> {
    let profile = args
        .profile_config
        .as_ref()
        .map(|path| load_worker_profile(path, &args.worker_profile))
        .transpose()?;
    let log_dir = profile
        .as_ref()
        .and_then(|profile| profile.log_dir.clone())
        .or_else(|| args.log_dir.clone())
        .unwrap_or_else(|| default_log_dir().join("runs"));
    let log_dir = absolute_path(log_dir)?;
    Ok(DispatchOptions {
        actor,
        command: profile
            .as_ref()
            .and_then(|profile| profile.command.clone())
            .unwrap_or_else(|| args.command.clone()),
        worker_profile: args.worker_profile.clone(),
        claim_ttl_ms: profile
            .as_ref()
            .and_then(|profile| profile.claim_ttl_ms)
            .unwrap_or(args.claim_ttl_ms),
        heartbeat_interval_ms: profile
            .as_ref()
            .and_then(|profile| profile.heartbeat_interval_ms)
            .unwrap_or(args.heartbeat_interval_ms),
        on_success: profile
            .as_ref()
            .and_then(|profile| profile.on_success)
            .unwrap_or_else(|| args.on_success.into()),
        on_failure: profile
            .as_ref()
            .and_then(|profile| profile.on_failure)
            .unwrap_or_else(|| args.on_failure.into()),
        log_dir,
    })
}

fn dispatch_loop(
    db_path: &PathBuf,
    board: &str,
    options: DispatchOptions,
    poll_interval_ms: u64,
    max_iterations: Option<usize>,
) -> Result<DispatchLoopSummary> {
    let mut iterations = 0;
    let mut claimed = 0;
    let mut runs = Vec::new();
    loop {
        iterations += 1;
        let result = dispatch_once(db_path, board, options.clone())?;
        claimed += result.claimed;
        runs.push(result);
        if max_iterations.is_some_and(|max| iterations >= max) {
            break;
        }
        thread::sleep(Duration::from_millis(poll_interval_ms));
    }
    Ok(DispatchLoopSummary {
        iterations,
        claimed,
        runs,
    })
}

fn absolute_path(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()
            .context("failed to resolve current directory")?
            .join(path))
    }
}

fn load_worker_profile(path: &PathBuf, profile_name: &str) -> Result<WorkerProfileConfig> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read worker profile config {}", path.display()))?;
    let mut active = false;
    let mut found = false;
    let mut profile = WorkerProfileConfig {
        command: None,
        claim_ttl_ms: None,
        heartbeat_interval_ms: None,
        on_success: None,
        on_failure: None,
        log_dir: None,
    };
    let section = format!("workers.{profile_name}");
    for raw_line in text.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            active = name.trim() == section;
            found |= active;
            continue;
        }
        if !active {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            bail!("invalid worker profile line: {raw_line}");
        };
        let key = key.trim();
        let value = unquote(value.trim());
        match key {
            "command" => profile.command = Some(value.to_owned()),
            "claim_ttl_ms" => profile.claim_ttl_ms = Some(value.parse()?),
            "heartbeat_interval_ms" => profile.heartbeat_interval_ms = Some(value.parse()?),
            "on_success" => profile.on_success = Some(parse_finish_policy(value)?),
            "on_failure" => profile.on_failure = Some(parse_finish_policy(value)?),
            "log_dir" => profile.log_dir = Some(PathBuf::from(value)),
            _ => bail!("unsupported worker profile key: {key}"),
        }
    }
    if !found {
        bail!(
            "worker profile {profile_name} not found in {}",
            path.display()
        );
    }
    Ok(profile)
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
}

fn parse_finish_policy(value: &str) -> Result<FinishPolicy> {
    match value {
        "done" => Ok(FinishPolicy::Done),
        "review" => Ok(FinishPolicy::Review),
        "blocked" => Ok(FinishPolicy::Blocked),
        "ready" => Ok(FinishPolicy::Ready),
        _ => bail!("unsupported finish policy: {value}"),
    }
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
            let mut task = create_task(
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
            if args.max_retries.is_some() {
                task = set_task_retry_policy_by_id(db_path, actor, &task.id, args.max_retries)?;
            }
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
            let mut task = update_task(
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
            if args.max_retries.is_some() || args.clear_max_retries {
                task = set_task_retry_policy_by_id(
                    db_path,
                    actor,
                    &task.id,
                    if args.clear_max_retries {
                        None
                    } else {
                        args.max_retries
                    },
                )?;
            }
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

fn handle_run(command: RunCommand, db_path: &PathBuf, json: bool) -> Result<()> {
    match command {
        RunCommand::Show { run_id } => {
            let run = get_run_by_id_global(db_path, &run_id)?;
            print_or_json(json, &run, || {
                format!(
                    "{} [{}] task={} exit={:?}",
                    run.id, run.status, run.task_id, run.exit_code
                )
            })?;
        }
        RunCommand::Logs { run_id, tail_bytes } => {
            let log = read_run_log(db_path, &run_id, tail_bytes)?;
            print_or_json(json, &log, || log.content.clone())?;
        }
    }
    Ok(())
}

#[derive(Debug, serde::Serialize)]
struct RunLogOutput {
    run_id: String,
    content: String,
    truncated: bool,
    tail_bytes: Option<usize>,
}

fn read_run_log(
    db_path: &PathBuf,
    run_id: &str,
    tail_bytes: Option<usize>,
) -> Result<RunLogOutput> {
    const DEFAULT_MAX_RUN_LOG_BYTES: usize = 256 * 1024;
    let run = get_run_by_id_global(db_path, run_id)?;
    let path = run
        .log_path
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("run log not found for {run_id}"))?;
    let bytes = fs::read(path).with_context(|| format!("failed to read run log {}", path))?;
    let limit = tail_bytes.unwrap_or(DEFAULT_MAX_RUN_LOG_BYTES);
    let truncated = bytes.len() > limit;
    let start = if truncated { bytes.len() - limit } else { 0 };
    Ok(RunLogOutput {
        run_id: run_id.to_owned(),
        content: String::from_utf8_lossy(&bytes[start..]).into_owned(),
        truncated,
        tail_bytes,
    })
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
    kanban_local::default_db_path()
}

fn default_log_dir() -> PathBuf {
    kanban_local::default_log_dir()
}

fn default_actor() -> String {
    kanban_local::default_actor()
}
