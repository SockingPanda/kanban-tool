use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use kanban_sqlite::FinishPolicy;

#[derive(Debug, Parser)]
#[command(
    name = "kanban",
    version,
    about = "Local SQLite-backed Kanban work queue"
)]
pub(crate) struct Cli {
    #[arg(long, global = true)]
    pub(crate) db: Option<PathBuf>,
    #[arg(long, global = true)]
    pub(crate) board: Option<String>,
    #[arg(long, global = true)]
    pub(crate) actor: Option<String>,
    #[arg(long, global = true)]
    pub(crate) json: bool,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Init {
        #[arg(long)]
        force: bool,
    },
    Board {
        #[command(subcommand)]
        command: BoardCommand,
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
    Search(SearchArgs),
    Index {
        #[command(subcommand)]
        command: IndexCommand,
    },
    Entity {
        #[command(subcommand)]
        command: EntityCommand,
    },
    Outbox {
        #[command(subcommand)]
        command: OutboxCommand,
    },
    Derived {
        #[command(subcommand)]
        command: DerivedCommand,
    },
    Graph {
        #[command(subcommand)]
        command: GraphCommand,
    },
    Vector {
        #[command(subcommand)]
        command: VectorCommand,
    },
    Context {
        #[command(subcommand)]
        command: ContextCommand,
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
pub(crate) enum TaskCommand {
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

#[derive(Debug, Subcommand)]
pub(crate) enum BoardCommand {
    List {
        #[arg(long)]
        include_archived: bool,
    },
    Create(BoardCreateArgs),
    Show {
        board: String,
    },
    Use {
        board: String,
    },
    Current,
    Archive {
        board: String,
    },
}

#[derive(Debug, Args)]
pub(crate) struct BoardCreateArgs {
    pub(crate) slug: String,
    #[arg(long)]
    pub(crate) name: String,
    #[arg(long)]
    pub(crate) description: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct CreateArgs {
    pub(crate) title: String,
    #[arg(long)]
    pub(crate) description: Option<String>,
    #[arg(long)]
    pub(crate) status: Option<String>,
    #[arg(long)]
    pub(crate) assignee: Option<String>,
    #[arg(long, default_value_t = 0)]
    pub(crate) priority: i64,
    #[arg(long)]
    pub(crate) scheduled_at: Option<i64>,
    #[arg(long)]
    pub(crate) due_at: Option<i64>,
    #[arg(long)]
    pub(crate) max_retries: Option<i64>,
    #[arg(long, default_value = "{}")]
    pub(crate) metadata: String,
}

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    #[arg(long)]
    pub(crate) status: Vec<String>,
    #[arg(long)]
    pub(crate) search: Option<String>,
    #[arg(long)]
    pub(crate) assignee: Option<String>,
    #[arg(long)]
    pub(crate) include_archived: bool,
    #[arg(long)]
    pub(crate) limit: Option<usize>,
    #[arg(long)]
    pub(crate) offset: Option<usize>,
    #[arg(long)]
    pub(crate) sort: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct SearchArgs {
    pub(crate) query: String,
    #[arg(long)]
    pub(crate) status: Vec<String>,
    #[arg(long)]
    pub(crate) assignee: Option<String>,
    #[arg(long)]
    pub(crate) include_archived: bool,
    #[arg(long, default_value_t = 20)]
    pub(crate) limit: usize,
    #[arg(long, default_value_t = 0)]
    pub(crate) offset: usize,
}

#[derive(Debug, Subcommand)]
pub(crate) enum IndexCommand {
    Status,
    Doctor,
    Rebuild,
    Sync,
}

#[derive(Debug, Subcommand)]
pub(crate) enum EntityCommand {
    List(EntityListArgs),
    Show { uri: String },
}

#[derive(Debug, Args)]
pub(crate) struct EntityListArgs {
    #[arg(long)]
    pub(crate) kind: Option<String>,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Debug, Subcommand)]
pub(crate) enum OutboxCommand {
    List(OutboxListArgs),
}

#[derive(Debug, Args)]
pub(crate) struct OutboxListArgs {
    #[arg(long)]
    pub(crate) status: Option<String>,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Debug, Subcommand)]
pub(crate) enum DerivedCommand {
    Status,
}

#[derive(Debug, Subcommand)]
pub(crate) enum GraphCommand {
    Status,
    Neighbors(GraphNeighborsArgs),
    Rebuild,
    Sync,
    Query(GraphQueryArgs),
}

#[derive(Debug, Args)]
pub(crate) struct GraphNeighborsArgs {
    pub(crate) entity_uri: String,
    #[arg(long)]
    pub(crate) predicate: Option<String>,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Debug, Args)]
pub(crate) struct GraphQueryArgs {
    pub(crate) sparql: String,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Debug, Subcommand)]
pub(crate) enum VectorCommand {
    Status,
    Rebuild,
    Sync,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ContextCommand {
    Build(ContextBuildArgs),
}

#[derive(Debug, Args)]
pub(crate) struct ContextBuildArgs {
    pub(crate) task_ref: String,
    #[arg(long, default_value_t = 5)]
    pub(crate) lexical_limit: usize,
    #[arg(long, default_value_t = 10)]
    pub(crate) graph_limit: usize,
    #[arg(long, default_value_t = 5)]
    pub(crate) vector_limit: usize,
    #[arg(long, default_value_t = 20)]
    pub(crate) max_items: usize,
}

#[derive(Debug, Args)]
pub(crate) struct UpdateArgs {
    pub(crate) task_ref: String,
    #[arg(long)]
    pub(crate) title: Option<String>,
    #[arg(long)]
    pub(crate) description: Option<String>,
    #[arg(long)]
    pub(crate) assignee: Option<String>,
    #[arg(long)]
    pub(crate) clear_assignee: bool,
    #[arg(long)]
    pub(crate) priority: Option<i64>,
    #[arg(long)]
    pub(crate) scheduled_at: Option<i64>,
    #[arg(long)]
    pub(crate) clear_scheduled_at: bool,
    #[arg(long)]
    pub(crate) due_at: Option<i64>,
    #[arg(long)]
    pub(crate) clear_due_at: bool,
    #[arg(long)]
    pub(crate) max_retries: Option<i64>,
    #[arg(long)]
    pub(crate) clear_max_retries: bool,
    #[arg(long)]
    pub(crate) metadata: Option<String>,
    #[arg(long)]
    pub(crate) expected_lock_version: Option<i64>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum RunCommand {
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
pub(crate) struct ClaimArgs {
    pub(crate) task_ref: String,
    #[arg(long, default_value_t = 300_000)]
    pub(crate) ttl_ms: i64,
}

#[derive(Debug, Args, Clone)]
pub(crate) struct HeartbeatArgs {
    pub(crate) task_ref: String,
    #[arg(long)]
    pub(crate) claim_token: String,
    #[arg(long, default_value_t = 300_000)]
    pub(crate) ttl_ms: i64,
}

#[derive(Debug, Args, Clone)]
pub(crate) struct FinishArgs {
    pub(crate) task_ref: String,
    #[arg(long)]
    pub(crate) claim_token: Option<String>,
    #[arg(long)]
    pub(crate) force: bool,
}

#[derive(Debug, Args)]
pub(crate) struct BlockArgs {
    pub(crate) task_ref: String,
    pub(crate) reason: String,
    #[arg(long)]
    pub(crate) claim_token: Option<String>,
    #[arg(long)]
    pub(crate) force: bool,
}

#[derive(Debug, Args)]
pub(crate) struct ReclaimArgs {
    #[arg(long)]
    pub(crate) expired: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum DepCommand {
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
pub(crate) struct DispatchArgs {
    #[arg(long)]
    pub(crate) once: bool,
    #[arg(long)]
    pub(crate) profile_config: Option<PathBuf>,
    #[arg(long, default_value_t = 1_000)]
    pub(crate) poll_interval_ms: u64,
    #[arg(long)]
    pub(crate) max_iterations: Option<usize>,
    #[arg(long, default_value = "sh -c 'true'")]
    pub(crate) command: String,
    #[arg(long, default_value = "default")]
    pub(crate) worker_profile: String,
    #[arg(long, default_value_t = 300_000)]
    pub(crate) claim_ttl_ms: i64,
    #[arg(long, default_value_t = 30_000)]
    pub(crate) heartbeat_interval_ms: i64,
    #[arg(long, value_enum, default_value_t = PolicyArg::Done)]
    pub(crate) on_success: PolicyArg,
    #[arg(long, value_enum, default_value_t = PolicyArg::Blocked)]
    pub(crate) on_failure: PolicyArg,
    #[arg(long)]
    pub(crate) log_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct ServeArgs {
    /// Loopback host interface to bind. Only loopback hosts are supported.
    #[arg(long, default_value = "127.0.0.1")]
    pub(crate) host: String,
    /// TCP port to bind.
    #[arg(long, default_value_t = 8721)]
    pub(crate) port: u16,
    /// Background search index sync interval in milliseconds. Use 0 to disable.
    #[arg(long, default_value_t = 5_000)]
    pub(crate) search_sync_interval_ms: u64,
}

#[derive(Debug, Args)]
pub(crate) struct BackupArgs {
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct ExportArgs {
    #[arg(long)]
    pub(crate) out: PathBuf,
    #[arg(long, default_value = "jsonl")]
    pub(crate) format: String,
}

#[derive(Debug, Args)]
pub(crate) struct ImportArgs {
    #[arg(long)]
    pub(crate) input: PathBuf,
    #[arg(long)]
    pub(crate) replace: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum PolicyArg {
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
pub(crate) struct DispatchLoopSummary {
    pub(crate) iterations: usize,
    pub(crate) claimed: usize,
    pub(crate) runs: Vec<kanban_sqlite::DispatchResult>,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct SearchOutput {
    pub(crate) hits: Vec<SearchOutputHit>,
    pub(crate) meta: kanban_search::SearchMeta,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct SearchOutputHit {
    pub(crate) task_id: String,
    pub(crate) seq: i64,
    pub(crate) score: f64,
    pub(crate) snippet: Option<String>,
    pub(crate) task: kanban_sqlite::TaskRecord,
}

#[derive(Debug, Clone)]
pub(crate) struct WorkerProfileConfig {
    pub(crate) command: Option<String>,
    pub(crate) claim_ttl_ms: Option<i64>,
    pub(crate) heartbeat_interval_ms: Option<i64>,
    pub(crate) on_success: Option<FinishPolicy>,
    pub(crate) on_failure: Option<FinishPolicy>,
    pub(crate) log_dir: Option<PathBuf>,
}
