use std::path::PathBuf;

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use kanban_sqlite::FinishPolicy;

#[derive(Debug, Parser)]
#[command(
    name = "kanban",
    version,
    about = "Local SQLite-backed Kanban work queue",
    after_help = "Examples:\n  kanban init\n  kanban task create \"Write spec\" --description-file -\n  kanban task list --status ready --json\n  kanban comment add default#1 --body-file - --kind note"
)]
pub(crate) struct Cli {
    /// Use a specific SQLite database path.
    #[arg(long, global = true)]
    pub(crate) db: Option<PathBuf>,
    /// Select the active board by slug or id for this command.
    #[arg(long, global = true)]
    pub(crate) board: Option<String>,
    /// Record this actor name in task events, runs, and comments.
    #[arg(long, global = true)]
    pub(crate) actor: Option<String>,
    /// Choose human-readable output language; JSON keys and enums stay stable.
    #[arg(long, global = true, value_name = "auto|zh-CN|en")]
    pub(crate) locale: Option<String>,
    /// Emit machine-readable JSON where the command supports it.
    #[arg(long, global = true)]
    pub(crate) json: bool,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Initialize the SQLite database, default board, and board columns.
    Init {
        #[arg(
            long,
            help = "Deprecated compatibility no-op; init is already idempotent and never resets data"
        )]
        force: bool,
    },
    /// Manage boards and the project-local active board selection.
    Board {
        #[command(subcommand)]
        command: BoardCommand,
    },
    /// Create, inspect, transition, claim, and archive tasks.
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    /// Append and list task comments, decisions, and signal backlinks.
    Comment {
        #[command(subcommand)]
        command: CommentCommand,
    },
    /// Record and review Agent/Product signals.
    Signal {
        #[command(subcommand)]
        command: SignalCommand,
    },
    /// Install or inspect Codex hooks that make kanban CLI failures actionable.
    Hook {
        #[command(subcommand)]
        command: HookCommand,
    },
    /// Manage task labels, suggestions, proposals, and ontology signals.
    Label {
        #[command(subcommand)]
        command: LabelCommand,
    },
    /// Manage task dependency edges.
    Dep {
        #[command(subcommand)]
        command: DepCommand,
    },
    /// List task events, optionally filtered to one task.
    Events { task_ref: Option<String> },
    /// List task runs, optionally filtered to one task.
    Runs { task_ref: Option<String> },
    /// Inspect individual dispatcher or claim runs.
    Run {
        #[command(subcommand)]
        command: RunCommand,
    },
    /// Search tasks by text with filters and pagination.
    Search(SearchArgs),
    /// Inspect and rebuild the local search index.
    Index {
        #[command(subcommand)]
        command: IndexCommand,
    },
    /// Inspect derived entity records.
    Entity {
        #[command(subcommand)]
        command: EntityCommand,
    },
    /// Inspect pending derived-store outbox work.
    Outbox {
        #[command(subcommand)]
        command: OutboxCommand,
    },
    /// Inspect derived store health.
    Derived {
        #[command(subcommand)]
        command: DerivedCommand,
    },
    /// Query and maintain the local graph index.
    Graph {
        #[command(subcommand)]
        command: GraphCommand,
    },
    /// Configure, query, and maintain the local vector index.
    Vector {
        #[command(subcommand)]
        command: VectorCommand,
    },
    /// Build task context from lexical, graph, and vector sources.
    Context {
        #[command(subcommand)]
        command: ContextCommand,
    },
    /// Run the local dispatcher loop to claim ready tasks and execute a worker command.
    Dispatch(DispatchArgs),
    /// Start the localhost HTTP API and SSE server.
    Serve(ServeArgs),
    /// Generate shell completion scripts.
    Completions { shell: Shell },
    #[command(name = "__complete", hide = true)]
    Complete(CompleteArgs),
    /// Check database integrity and consistency.
    Doctor,
    /// Show board task counts and summary statistics.
    Stats,
    /// Write a SQLite backup copy to a chosen path.
    Backup(BackupArgs),
    /// Export portable board data, including comments and signals.
    Export(ExportArgs),
    /// Import portable data into the selected database.
    Import(ImportArgs),
    /// Run a SQLite WAL checkpoint for the database.
    Checkpoint,
    /// Run SQLite VACUUM maintenance on the database.
    Vacuum,
}

#[derive(Debug, Args)]
pub(crate) struct CompleteArgs {
    pub(crate) kind: CompleteKind,
    pub(crate) current: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum CompleteKind {
    #[value(name = "task-ref")]
    TaskRef,
    #[value(name = "dependency-task-ref")]
    DependencyTaskRef,
    Board,
    Status,
    #[value(name = "comment-kind")]
    CommentKind,
}

#[derive(Debug, Subcommand)]
pub(crate) enum HookCommand {
    /// Manage Codex lifecycle hooks for kanban-aware agent feedback.
    Codex {
        #[command(subcommand)]
        command: CodexHookCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum CodexHookCommand {
    /// Install managed Codex hooks and default prompt configuration.
    Install(CodexHookInstallArgs),
    /// Show whether managed Codex hooks and prompt config are installed.
    Status,
    /// Remove only the managed Codex hook entries.
    Uninstall,
    /// Internal stdin handlers used by installed Codex hooks.
    Handle {
        #[command(subcommand)]
        command: CodexHookHandleCommand,
    },
}

#[derive(Debug, Args)]
pub(crate) struct CodexHookInstallArgs {
    #[arg(
        long,
        value_name = "command-prefix",
        default_value = "kanban hook codex handle"
    )]
    pub(crate) handler_command: String,
    #[arg(long, default_value_t = 30)]
    pub(crate) timeout: u64,
    #[arg(long)]
    pub(crate) record_signals: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum CodexHookHandleCommand {
    /// Handle a failed kanban command trace from a Codex hook payload.
    Failure(CodexHookFailureHandleArgs),
    #[command(name = "task-create")]
    /// Handle a successful kanban task create trace from a Codex hook payload.
    TaskCreate(CodexHookTaskCreateHandleArgs),
}

#[derive(Debug, Args)]
pub(crate) struct CodexHookFailureHandleArgs {
    #[arg(long, hide = true)]
    pub(crate) installed_by: Option<String>,
    #[arg(long)]
    pub(crate) record_signals: bool,
}

#[derive(Debug, Args)]
pub(crate) struct CodexHookTaskCreateHandleArgs {
    #[arg(long, hide = true)]
    pub(crate) installed_by: Option<String>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TaskCommand {
    /// Create a task, preferably with rich description input from a file or stdin.
    Create(CreateArgs),
    /// List tasks with status, label, plan, search, and pagination filters.
    List(ListArgs),
    /// Show one task by ref or id.
    Show {
        task_ref: String,
        #[arg(long)]
        details: bool,
    },
    /// Update selected task fields without replacing unspecified values.
    Update(UpdateArgs),
    /// Move a todo or scheduled task to ready when guards allow it.
    Promote { task_ref: String },
    /// Reopen a completed or reviewed task with a reason.
    Reopen(TaskReopenArgs),
    /// Start work on a task and create a claim token.
    Start(ClaimArgs),
    /// Atomically claim a ready task for execution.
    Claim(ClaimArgs),
    /// Extend an active claim lease.
    Heartbeat(HeartbeatArgs),
    /// Finish a running task as done when claim and guards allow it.
    Done(FinishArgs),
    /// Alias for finishing a running task as done.
    Complete(FinishArgs),
    /// Submit a running task to review instead of completing it.
    Review(FinishArgs),
    /// Block a task with a reason.
    Block(BlockArgs),
    /// Recompute a blocked task and return it to the correct eligible state.
    Unblock { task_ref: String },
    /// Reclaim expired running task claims.
    Reclaim(ReclaimArgs),
    /// Archive a task after normal completion, or with explicit force.
    Archive {
        task_ref: String,
        #[arg(
            long,
            help = "Archive even when normal archive guards would reject the task"
        )]
        force: bool,
    },
    /// Manage task execution steps.
    Step {
        #[command(subcommand)]
        command: TaskStepCommand,
    },
}

#[derive(Debug, Args)]
pub(crate) struct TaskReopenArgs {
    pub(crate) task_ref: String,
    #[arg(long)]
    pub(crate) reason: Option<String>,
    #[arg(
        long = "reason-file",
        value_name = "PATH|-",
        help = "Read reason text from PATH, or stdin with -"
    )]
    pub(crate) reason_file: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TaskStepCommand {
    /// List execution steps for a task.
    List { task_ref: String },
    /// Add an execution step to a task.
    Add(TaskStepAddArgs),
    /// Update a task step title, body, link, position, or requiredness.
    Update(TaskStepUpdateArgs),
    /// Mark a task step done with a note.
    Done(TaskStepDoneArgs),
    /// Skip a task step with a reason.
    Skip(TaskStepReasonArgs),
    /// Reopen a previously done or skipped step.
    Reopen(TaskStepReasonArgs),
    /// Remove a task step.
    Remove { task_ref: String, step_ref: String },
    #[command(name = "not-required")]
    /// Mark all incomplete required steps as no longer required.
    NotRequired(TaskStepNotRequiredArgs),
}

#[derive(Debug, Args)]
pub(crate) struct TaskStepAddArgs {
    pub(crate) task_ref: String,
    pub(crate) title: String,
    #[arg(long)]
    pub(crate) body: Option<String>,
    #[arg(
        long = "body-file",
        value_name = "PATH|-",
        help = "Read body text from PATH, or stdin with -; recommended for multiline or shell-sensitive text"
    )]
    pub(crate) body_file: Option<PathBuf>,
    #[arg(long = "link-task")]
    pub(crate) linked_task_ref: Option<String>,
    #[arg(long)]
    pub(crate) position: Option<i64>,
    #[arg(long)]
    pub(crate) required: bool,
    #[arg(long)]
    pub(crate) optional: bool,
}

#[derive(Debug, Args)]
pub(crate) struct TaskStepUpdateArgs {
    pub(crate) task_ref: String,
    pub(crate) step_ref: String,
    #[arg(long)]
    pub(crate) title: Option<String>,
    #[arg(long)]
    pub(crate) body: Option<String>,
    #[arg(
        long = "body-file",
        value_name = "PATH|-",
        help = "Read body text from PATH, or stdin with -; recommended for multiline or shell-sensitive text"
    )]
    pub(crate) body_file: Option<PathBuf>,
    #[arg(long = "clear-body")]
    pub(crate) clear_body: bool,
    #[arg(long = "link-task")]
    pub(crate) linked_task_ref: Option<String>,
    #[arg(long = "unlink-task")]
    pub(crate) unlink_task: bool,
    #[arg(long)]
    pub(crate) position: Option<i64>,
    #[arg(long)]
    pub(crate) required: bool,
    #[arg(long)]
    pub(crate) optional: bool,
}

#[derive(Debug, Args)]
pub(crate) struct TaskStepDoneArgs {
    pub(crate) task_ref: String,
    pub(crate) step_ref: String,
    #[arg(long)]
    pub(crate) note: Option<String>,
    #[arg(
        long = "note-file",
        value_name = "PATH|-",
        help = "Read note text from PATH, or stdin with -"
    )]
    pub(crate) note_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct TaskStepReasonArgs {
    pub(crate) task_ref: String,
    pub(crate) step_ref: String,
    #[arg(long)]
    pub(crate) reason: Option<String>,
    #[arg(
        long = "reason-file",
        value_name = "PATH|-",
        help = "Read reason text from PATH, or stdin with -"
    )]
    pub(crate) reason_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct TaskStepNotRequiredArgs {
    pub(crate) task_ref: String,
    #[arg(long)]
    pub(crate) reason: Option<String>,
    #[arg(
        long = "reason-file",
        value_name = "PATH|-",
        help = "Read reason text from PATH, or stdin with -"
    )]
    pub(crate) reason_file: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum BoardCommand {
    /// List boards in this database.
    List {
        #[arg(long)]
        include_archived: bool,
    },
    /// Create a board in this database.
    Create(BoardCreateArgs),
    /// Show one board by slug or id.
    Show { board: String },
    /// Persist the active board in project-local .kb/config.toml.
    Use { board: String },
    /// Show the resolved active board.
    Current,
    /// Archive a board.
    Archive { board: String },
}

#[derive(Debug, Subcommand)]
pub(crate) enum CommentCommand {
    /// Append a note, decision, or signal backlink comment to a task.
    Add(CommentAddArgs),
    /// List comments for a task.
    List { task_ref: String },
}

#[derive(Debug, Subcommand)]
pub(crate) enum SignalCommand {
    /// Record an Agent/Product signal from JSON input.
    Record(SignalRecordArgs),
    /// List signals with status, kind, task, and history filters.
    List(SignalListArgs),
    /// Show one signal by id.
    Show { signal_id: String },
    /// List signals that need review.
    Review(SignalReviewListArgs),
    /// Confirm open signals with a reason.
    Confirm(SignalLifecycleArgs),
    /// Reject signals with a reason.
    Reject(SignalLifecycleArgs),
    /// Resolve confirmed signals with a reason.
    Resolve(SignalLifecycleArgs),
    /// Supersede signals with another signal.
    Supersede(SignalSupersedeArgs),
}

#[derive(Debug, Args)]
#[command(
    after_help = "Examples:\n  kanban signal record --input signal.json --json\n  kanban signal record --input - --json < signal.json"
)]
pub(crate) struct SignalRecordArgs {
    #[arg(long)]
    pub(crate) input: String,
}

#[derive(Debug, Args)]
pub(crate) struct SignalListArgs {
    #[arg(long)]
    pub(crate) status: Vec<String>,
    #[arg(long)]
    pub(crate) kind: Vec<String>,
    #[arg(long)]
    pub(crate) task: Option<String>,
    #[arg(long)]
    pub(crate) include_all: bool,
    #[arg(long, default_value_t = 100)]
    pub(crate) limit: usize,
}

#[derive(Debug, Args)]
pub(crate) struct SignalReviewListArgs {
    #[arg(long)]
    pub(crate) status: Vec<String>,
    #[arg(long)]
    pub(crate) kind: Vec<String>,
    #[arg(long)]
    pub(crate) task: Option<String>,
    #[arg(long, default_value_t = 100)]
    pub(crate) limit: usize,
}

#[derive(Debug, Args)]
pub(crate) struct SignalLifecycleArgs {
    #[arg(required = true)]
    pub(crate) signal_ids: Vec<String>,
    #[arg(long)]
    pub(crate) reason: Option<String>,
    #[arg(
        long = "reason-file",
        value_name = "PATH|-",
        help = "Read reason text from PATH, or stdin with -"
    )]
    pub(crate) reason_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct SignalSupersedeArgs {
    #[arg(required = true)]
    pub(crate) signal_ids: Vec<String>,
    #[arg(long)]
    pub(crate) by: String,
    #[arg(long)]
    pub(crate) reason: Option<String>,
    #[arg(
        long = "reason-file",
        value_name = "PATH|-",
        help = "Read reason text from PATH, or stdin with -"
    )]
    pub(crate) reason_file: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum LabelCommand {
    /// List labels on the active board.
    List,
    /// Create a label identity.
    Create(LabelCreateArgs),
    /// Create a label with initial semantics from one task.
    Bootstrap(LabelBootstrapArgs),
    /// Delete a label identity, optionally removing task bindings with force.
    Delete(LabelDeleteArgs),
    /// Add one or more labels to a task.
    Add(LabelAddTaskArgs),
    /// Remove a label from one task.
    Remove(LabelTaskArgs),
    /// Manage label semantics text and examples.
    Semantics {
        #[command(subcommand)]
        command: LabelSemanticsCommand,
    },
    #[command(alias = "atom")]
    /// Inspect label ontology atoms.
    Atoms {
        #[command(subcommand)]
        command: LabelAtomsCommand,
    },
    #[command(name = "atom-index")]
    /// Maintain and query the label atom vector index.
    AtomIndex {
        #[command(subcommand)]
        command: LabelAtomIndexCommand,
    },
    /// Suggest labels for a task using lexical and optional vector evidence.
    Suggest(LabelSuggestArgs),
    /// Create a label proposal from task context or JSON input.
    Propose(LabelProposeArgs),
    /// Review and decide pending label proposals.
    Proposals {
        #[command(subcommand)]
        command: LabelProposalsCommand,
    },
    /// Record, review, apply, and validate label ontology signals.
    Ontology {
        #[command(subcommand)]
        command: LabelOntologyCommand,
    },
}

#[derive(Debug, Args)]
pub(crate) struct LabelCreateArgs {
    pub(crate) name: String,
    #[arg(long)]
    pub(crate) color: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct LabelDeleteArgs {
    pub(crate) label: String,
    #[arg(long)]
    pub(crate) force: bool,
}

#[derive(Debug, Args)]
pub(crate) struct LabelTaskArgs {
    pub(crate) task_ref: String,
    pub(crate) label: String,
}

#[derive(Debug, Args)]
pub(crate) struct LabelAddTaskArgs {
    #[arg(long)]
    pub(crate) create_missing: bool,
    pub(crate) task_ref: String,
    #[arg(required = true)]
    pub(crate) labels: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct LabelBootstrapArgs {
    pub(crate) task_ref: String,
    pub(crate) label: String,
    #[arg(long)]
    pub(crate) description: Option<String>,
    #[arg(long = "applies-when")]
    pub(crate) applies_when: Vec<String>,
    #[arg(long = "excludes-when")]
    pub(crate) excludes_when: Vec<String>,
    #[arg(long = "positive-example")]
    pub(crate) positive_examples: Vec<String>,
    #[arg(long = "negative-example")]
    pub(crate) negative_examples: Vec<String>,
    #[arg(long)]
    pub(crate) verify: bool,
    #[arg(long = "min-verify-score", default_value_t = 0.50)]
    pub(crate) min_verify_score: f32,
    #[arg(long = "vector-config", alias = "config")]
    pub(crate) vector_config: Option<std::path::PathBuf>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum LabelSemanticsCommand {
    /// List labels with semantics metadata.
    List,
    /// Show semantics for one label.
    Show { label: String },
    /// Insert or update label semantics with an optional CAS hash.
    Upsert(Box<LabelSemanticsUpsertArgs>),
    /// Delete label semantics with a required hash and reason.
    Delete(LabelSemanticsDeleteArgs),
}

#[derive(Debug, Args)]
pub(crate) struct LabelSemanticsDeleteArgs {
    pub(crate) label: String,
    #[arg(long = "expected-semantics-hash")]
    pub(crate) expected_semantics_hash: String,
    #[arg(long)]
    pub(crate) reason: Option<String>,
    #[arg(
        long = "reason-file",
        value_name = "PATH|-",
        help = "Read reason text from PATH, or stdin with -"
    )]
    pub(crate) reason_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct LabelSemanticsUpsertArgs {
    pub(crate) label: String,
    #[arg(long = "expected-semantics-hash")]
    pub(crate) expected_semantics_hash: Option<String>,
    #[arg(long)]
    pub(crate) replace: bool,
    #[arg(long)]
    pub(crate) reason: Option<String>,
    #[arg(
        long = "reason-file",
        value_name = "PATH|-",
        help = "Read reason text from PATH, or stdin with -"
    )]
    pub(crate) reason_file: Option<PathBuf>,
    #[arg(long = "source-signal")]
    pub(crate) source_signal_ids: Vec<String>,
    #[arg(long)]
    pub(crate) description: Option<String>,
    #[arg(long = "applies-when")]
    pub(crate) applies_when: Vec<String>,
    #[arg(long = "excludes-when")]
    pub(crate) excludes_when: Vec<String>,
    #[arg(long = "positive-example")]
    pub(crate) positive_examples: Vec<String>,
    #[arg(long = "negative-example")]
    pub(crate) negative_examples: Vec<String>,
    #[arg(long = "remove-applies-when")]
    pub(crate) remove_applies_when: Vec<String>,
    #[arg(long = "remove-excludes-when")]
    pub(crate) remove_excludes_when: Vec<String>,
    #[arg(long = "remove-positive-example")]
    pub(crate) remove_positive_examples: Vec<String>,
    #[arg(long = "remove-negative-example")]
    pub(crate) remove_negative_examples: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum LabelAtomsCommand {
    /// List label ontology atoms.
    List,
    /// Explain one label ontology atom.
    Explain { atom_ref: String },
}

#[derive(Debug, Subcommand)]
pub(crate) enum LabelAtomIndexCommand {
    /// Show label atom vector index status.
    Status(LabelAtomIndexStatusArgs),
    /// Rebuild an index from current SQLite data.
    Rebuild(LabelAtomIndexRebuildArgs),
    /// Query an index for matching records.
    Query(LabelAtomIndexQueryArgs),
}

#[derive(Debug, Args)]
pub(crate) struct LabelAtomIndexStatusArgs {
    #[arg(long = "vector-config", alias = "config")]
    pub(crate) vector_config: Option<std::path::PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct LabelAtomIndexRebuildArgs {
    #[arg(long = "vector-config", alias = "config")]
    pub(crate) vector_config: Option<std::path::PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct LabelAtomIndexQueryArgs {
    pub(crate) text: String,
    #[arg(long)]
    pub(crate) polarity: Option<LabelAtomPolarityArg>,
    #[arg(long, default_value_t = 24)]
    pub(crate) limit: usize,
    #[arg(long = "vector-config", alias = "config")]
    pub(crate) vector_config: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum LabelAtomPolarityArg {
    Positive,
    Negative,
}

#[derive(Debug, Args)]
pub(crate) struct LabelSuggestArgs {
    pub(crate) task_ref: String,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_OUTPUT_LIMIT)]
    pub(crate) limit: usize,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_CANDIDATE_LIMIT)]
    pub(crate) candidate_limit: usize,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_ATOM_LIMIT)]
    pub(crate) atom_limit: usize,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_MAX_SELECTED_LABELS)]
    pub(crate) max_selected_labels: usize,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_MIN_SCORE)]
    pub(crate) min_score: f32,
    #[arg(long = "vector-config", alias = "config")]
    pub(crate) vector_config: Option<std::path::PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct LabelProposeArgs {
    pub(crate) task_ref: String,
    #[arg(long = "proposal-json")]
    pub(crate) proposal_json: Option<std::path::PathBuf>,
    #[arg(long = "source-signal")]
    pub(crate) source_signal_ids: Vec<String>,
    #[arg(long)]
    pub(crate) allow_retarget: bool,
    #[arg(long = "retarget-reason")]
    pub(crate) retarget_reason: Option<String>,
    #[arg(
        long = "retarget-reason-file",
        value_name = "PATH|-",
        help = "Read retarget reason text from PATH, or stdin with -"
    )]
    pub(crate) retarget_reason_file: Option<PathBuf>,
    #[command(flatten)]
    pub(crate) ontology_actor: LabelOntologyActorArgs,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_OUTPUT_LIMIT)]
    pub(crate) limit: usize,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_CANDIDATE_LIMIT)]
    pub(crate) candidate_limit: usize,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_ATOM_LIMIT)]
    pub(crate) atom_limit: usize,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_MAX_SELECTED_LABELS)]
    pub(crate) max_selected_labels: usize,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_MIN_SCORE)]
    pub(crate) min_score: f32,
    #[arg(long = "vector-config", alias = "config")]
    pub(crate) vector_config: Option<std::path::PathBuf>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum LabelProposalsCommand {
    /// List label proposals.
    List(LabelProposalsListArgs),
    /// Show one proposal by id.
    Show { proposal_id: String },
    /// Accept a proposal and apply its action.
    Accept(LabelProposalAcceptArgs),
    /// Reject a proposal with an optional reason.
    Reject(LabelProposalDecisionArgs),
}

#[derive(Debug, Args)]
pub(crate) struct LabelProposalsListArgs {
    #[arg(long)]
    pub(crate) task: Option<String>,
    #[arg(long)]
    pub(crate) status: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct LabelProposalDecisionArgs {
    pub(crate) proposal_id: String,
    #[arg(long)]
    pub(crate) reason: Option<String>,
    #[arg(
        long = "reason-file",
        value_name = "PATH|-",
        help = "Read reason text from PATH, or stdin with -"
    )]
    pub(crate) reason_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct LabelProposalAcceptArgs {
    pub(crate) proposal_id: String,
    #[arg(long)]
    pub(crate) reason: Option<String>,
    #[arg(
        long = "reason-file",
        value_name = "PATH|-",
        help = "Read reason text from PATH, or stdin with -"
    )]
    pub(crate) reason_file: Option<PathBuf>,
    #[arg(long = "source-signal")]
    pub(crate) source_signal_ids: Vec<String>,
    #[arg(long)]
    pub(crate) allow_retarget: bool,
    #[arg(long = "retarget-reason")]
    pub(crate) retarget_reason: Option<String>,
    #[arg(
        long = "retarget-reason-file",
        value_name = "PATH|-",
        help = "Read retarget reason text from PATH, or stdin with -"
    )]
    pub(crate) retarget_reason_file: Option<PathBuf>,
    #[command(flatten)]
    pub(crate) ontology_actor: LabelOntologyActorArgs,
}

#[derive(Debug, Subcommand)]
pub(crate) enum LabelOntologyCommand {
    /// Record a label ontology signal from JSON input.
    Record(LabelOntologyRecordArgs),
    /// List label ontology signals.
    List(LabelOntologyListArgs),
    /// Show one label ontology signal.
    Show { signal_id: String },
    /// Show grouped ontology review queues.
    Review(LabelOntologyReviewArgs),
    /// Report ontology quality samples and gaps.
    Quality(LabelOntologyQualityArgs),
    /// Confirm ontology signals with actor attribution.
    Confirm(LabelOntologyActionArgs),
    /// Reject ontology signals with actor attribution.
    Reject(LabelOntologyActionArgs),
    /// Supersede ontology signals with another signal.
    Supersede(LabelOntologySupersedeArgs),
    /// Resolve ontology signals without applying a change.
    Resolve(LabelOntologyResolveArgs),
    /// Apply approved ontology changes.
    Apply {
        #[command(subcommand)]
        command: LabelOntologyApplyCommand,
    },
    /// Revert an ontology action with a reason and optional hash guard.
    Revert(LabelOntologyRevertArgs),
    /// Attach validation evidence to ontology actions.
    Validate(LabelOntologyValidateArgs),
}

#[derive(Debug, Args)]
#[command(
    after_help = "Examples:\n  kanban label ontology record default#1 --input ontology-signal.json --capture-suggest --json\n  kanban label ontology record default#1 --input - --suggestion-snapshot suggest.json"
)]
pub(crate) struct LabelOntologyRecordArgs {
    pub(crate) task_ref: String,
    #[arg(long)]
    pub(crate) input: String,
    #[arg(long = "suggestion-snapshot")]
    pub(crate) suggestion_snapshot: Option<String>,
    #[arg(long)]
    pub(crate) capture_suggest: bool,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_OUTPUT_LIMIT)]
    pub(crate) limit: usize,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_CANDIDATE_LIMIT)]
    pub(crate) candidate_limit: usize,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_ATOM_LIMIT)]
    pub(crate) atom_limit: usize,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_MAX_SELECTED_LABELS)]
    pub(crate) max_selected_labels: usize,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_MIN_SCORE)]
    pub(crate) min_score: f32,
    #[arg(long = "vector-config", alias = "config")]
    pub(crate) vector_config: Option<std::path::PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct LabelOntologyListArgs {
    #[arg(long)]
    pub(crate) status: Vec<String>,
    #[arg(long)]
    pub(crate) kind: Vec<String>,
    #[arg(long)]
    pub(crate) task: Option<String>,
    #[arg(long)]
    pub(crate) label: Option<String>,
    #[arg(long = "proposed-label")]
    pub(crate) proposed_label: Option<String>,
    #[arg(long)]
    pub(crate) include_all: bool,
    #[arg(long, default_value_t = 100)]
    pub(crate) limit: usize,
}

#[derive(Debug, Args)]
pub(crate) struct LabelOntologyReviewArgs {
    #[arg(long, value_enum, default_value = "label")]
    pub(crate) group_by: LabelOntologyReviewGroupByArg,
    #[arg(long)]
    pub(crate) include_all: bool,
    #[arg(long, default_value_t = 100)]
    pub(crate) limit: usize,
}

#[derive(Debug, Args)]
pub(crate) struct LabelOntologyQualityArgs {
    #[arg(long, default_value_t = 20)]
    pub(crate) sample_limit: usize,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum LabelOntologyReviewGroupByArg {
    Label,
    #[value(name = "candidate-atom")]
    CandidateAtom,
    #[value(name = "proposed-label")]
    ProposedLabel,
    Cluster,
}

#[derive(Debug, Args)]
pub(crate) struct LabelOntologyActionArgs {
    #[arg(required = true)]
    pub(crate) signal_ids: Vec<String>,
    #[arg(long)]
    pub(crate) reason: Option<String>,
    #[arg(
        long = "reason-file",
        value_name = "PATH|-",
        help = "Read reason text from PATH, or stdin with -"
    )]
    pub(crate) reason_file: Option<PathBuf>,
    #[command(flatten)]
    pub(crate) actor: LabelOntologyActorArgs,
}

#[derive(Debug, Args)]
pub(crate) struct LabelOntologySupersedeArgs {
    #[arg(required = true)]
    pub(crate) signal_ids: Vec<String>,
    #[arg(long = "by")]
    pub(crate) superseded_by_signal_id: String,
    #[arg(long)]
    pub(crate) reason: Option<String>,
    #[arg(
        long = "reason-file",
        value_name = "PATH|-",
        help = "Read reason text from PATH, or stdin with -"
    )]
    pub(crate) reason_file: Option<PathBuf>,
    #[command(flatten)]
    pub(crate) actor: LabelOntologyActorArgs,
}

#[derive(Debug, Args)]
pub(crate) struct LabelOntologyResolveArgs {
    #[arg(required = true)]
    pub(crate) signal_ids: Vec<String>,
    #[arg(long = "no-change")]
    pub(crate) no_change: bool,
    #[arg(long)]
    pub(crate) reason: Option<String>,
    #[arg(
        long = "reason-file",
        value_name = "PATH|-",
        help = "Read reason text from PATH, or stdin with -"
    )]
    pub(crate) reason_file: Option<PathBuf>,
    #[command(flatten)]
    pub(crate) actor: LabelOntologyActorArgs,
}

#[derive(Debug, Subcommand)]
pub(crate) enum LabelOntologyApplyCommand {
    /// Apply atom changes proposed by ontology signals.
    Atom(LabelOntologyApplyAtomArgs),
}

#[derive(Debug, Args)]
pub(crate) struct LabelOntologyApplyAtomArgs {
    #[arg(required = true)]
    pub(crate) signal_ids: Vec<String>,
    #[arg(long)]
    pub(crate) label: String,
    #[arg(long)]
    pub(crate) kind: LabelOntologyAtomKindArg,
    #[arg(long)]
    pub(crate) text: Option<String>,
    #[arg(
        long = "text-file",
        value_name = "PATH|-",
        help = "Read atom text from PATH, or stdin with -"
    )]
    pub(crate) text_file: Option<PathBuf>,
    #[arg(long)]
    pub(crate) reason: Option<String>,
    #[arg(
        long = "reason-file",
        value_name = "PATH|-",
        help = "Read reason text from PATH, or stdin with -"
    )]
    pub(crate) reason_file: Option<PathBuf>,
    #[command(flatten)]
    pub(crate) actor: LabelOntologyActorArgs,
    #[arg(long)]
    pub(crate) allow_retarget: bool,
    #[arg(long = "retarget-reason")]
    pub(crate) retarget_reason: Option<String>,
    #[arg(
        long = "retarget-reason-file",
        value_name = "PATH|-",
        help = "Read retarget reason text from PATH, or stdin with -"
    )]
    pub(crate) retarget_reason_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct LabelOntologyRevertArgs {
    pub(crate) action_id: String,
    #[arg(long = "expected-current-hash")]
    pub(crate) expected_current_hash: Option<String>,
    #[arg(long)]
    pub(crate) reason: Option<String>,
    #[arg(
        long = "reason-file",
        value_name = "PATH|-",
        help = "Read reason text from PATH, or stdin with -"
    )]
    pub(crate) reason_file: Option<PathBuf>,
    #[command(flatten)]
    pub(crate) actor: LabelOntologyActorArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct LabelOntologyActorArgs {
    #[arg(long = "actor-type", value_enum, default_value = "user")]
    pub(crate) actor_type: LabelOntologyActorTypeArg,
    #[arg(long = "agent-type")]
    pub(crate) agent_type: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum LabelOntologyActorTypeArg {
    User,
    Agent,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum LabelOntologyAtomKindArg {
    #[value(name = "applies-when")]
    AppliesWhen,
    #[value(name = "positive-example")]
    PositiveExample,
    #[value(name = "excludes-when")]
    ExcludesWhen,
    #[value(name = "negative-example")]
    NegativeExample,
}

#[derive(Debug, Args)]
pub(crate) struct LabelOntologyValidateArgs {
    pub(crate) action_id: String,
    #[arg(long)]
    pub(crate) status: LabelOntologyValidationStatusArg,
    #[arg(long)]
    pub(crate) reason: Option<String>,
    #[arg(
        long = "reason-file",
        value_name = "PATH|-",
        help = "Read reason text from PATH, or stdin with -"
    )]
    pub(crate) reason_file: Option<PathBuf>,
    #[arg(long)]
    pub(crate) input: Option<String>,
    #[arg(long)]
    pub(crate) trusted: bool,
    #[arg(long = "positive-control", conflicts_with = "positive_control_waiver")]
    pub(crate) positive_controls: Vec<String>,
    #[arg(long = "positive-control-waiver", conflicts_with = "positive_controls")]
    pub(crate) positive_control_waiver: Option<String>,
    #[arg(
        long = "positive-control-waiver-file",
        value_name = "PATH|-",
        conflicts_with = "positive_controls",
        help = "Read positive control waiver text from PATH, or stdin with -"
    )]
    pub(crate) positive_control_waiver_file: Option<PathBuf>,
    #[arg(long = "vector-config", alias = "config")]
    pub(crate) vector_config: Option<std::path::PathBuf>,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_OUTPUT_LIMIT)]
    pub(crate) limit: usize,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_CANDIDATE_LIMIT)]
    pub(crate) candidate_limit: usize,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_ATOM_LIMIT)]
    pub(crate) atom_limit: usize,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_MAX_SELECTED_LABELS)]
    pub(crate) max_selected_labels: usize,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_LABEL_SUGGESTION_MIN_SCORE)]
    pub(crate) min_score: f32,
    #[command(flatten)]
    pub(crate) actor: LabelOntologyActorArgs,
    pub(crate) signal_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum LabelOntologyValidationStatusArg {
    Passed,
    Failed,
    Partial,
}

#[derive(Debug, Args)]
#[command(
    after_help = "Examples:\n  kanban comment add default#1 --body-file - --kind note\n  kanban comment add default#1 --body-file note.md --metadata-json-file metadata.json"
)]
pub(crate) struct CommentAddArgs {
    pub(crate) task_ref: String,
    pub(crate) body: Option<String>,
    #[arg(
        long = "body-file",
        value_name = "PATH|-",
        help = "Read body text from PATH, or stdin with -; recommended for multiline or shell-sensitive text"
    )]
    pub(crate) body_file: Option<PathBuf>,
    #[arg(long)]
    pub(crate) kind: Option<String>,
    #[arg(long)]
    pub(crate) author_type: Option<String>,
    #[arg(long)]
    pub(crate) agent_type: Option<String>,
    #[arg(long)]
    pub(crate) metadata_json: Option<String>,
    #[arg(
        long = "metadata-json-file",
        value_name = "PATH|-",
        help = "Read comment metadata JSON from PATH, or stdin with -; avoids shell quoting issues"
    )]
    pub(crate) metadata_json_file: Option<PathBuf>,
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
#[command(
    after_help = "Examples:\n  kanban task create \"Draft release note\" --description-file -\n  kanban task create \"Fix CLI help\" --status ready --label cli --json\n\nUse --description-file - for multiline or shell-sensitive text containing $, backticks, or JSON."
)]
pub(crate) struct CreateArgs {
    pub(crate) title: String,
    #[arg(long)]
    pub(crate) description: Option<String>,
    #[arg(
        long = "description-file",
        value_name = "PATH|-",
        help = "Read description text from PATH, or stdin with -; recommended for multiline or shell-sensitive text"
    )]
    pub(crate) description_file: Option<PathBuf>,
    #[arg(long)]
    pub(crate) status: Option<String>,
    #[arg(long)]
    pub(crate) assignee: Option<String>,
    #[arg(long, default_value_t = kanban_sqlite::DEFAULT_PRIORITY)]
    pub(crate) priority: i64,
    #[arg(long)]
    pub(crate) scheduled_at: Option<i64>,
    #[arg(long)]
    pub(crate) due_at: Option<i64>,
    #[arg(long)]
    pub(crate) max_retries: Option<i64>,
    #[arg(long)]
    pub(crate) metadata: Option<String>,
    #[arg(
        long = "metadata-file",
        value_name = "PATH|-",
        help = "Read task metadata JSON from PATH, or stdin with -; avoids shell quoting issues"
    )]
    pub(crate) metadata_file: Option<PathBuf>,
    #[arg(long = "label")]
    pub(crate) labels: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    #[arg(long)]
    pub(crate) status: Vec<String>,
    #[arg(long)]
    pub(crate) search: Option<String>,
    #[arg(long)]
    pub(crate) assignee: Option<String>,
    #[arg(long = "label")]
    pub(crate) labels: Vec<String>,
    #[arg(long)]
    pub(crate) include_archived: bool,
    #[arg(long)]
    pub(crate) limit: Option<usize>,
    #[arg(long)]
    pub(crate) offset: Option<usize>,
    #[arg(long)]
    pub(crate) sort: Option<String>,
    #[arg(long = "plan-needed")]
    pub(crate) plan_needed: bool,
    #[arg(long = "has-steps")]
    pub(crate) has_steps: bool,
    #[arg(long = "incomplete-required-steps")]
    pub(crate) incomplete_required_steps: bool,
    #[arg(long = "plan-filter", value_enum)]
    pub(crate) plan_filters: Vec<TaskPlanFilterArg>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum TaskPlanFilterArg {
    #[value(name = "plan-needed")]
    PlanNeeded,
    #[value(name = "has-steps")]
    HasSteps,
    #[value(name = "incomplete-required-steps")]
    IncompleteRequiredSteps,
}

#[derive(Debug, Args)]
pub(crate) struct SearchArgs {
    pub(crate) query: String,
    #[arg(long)]
    pub(crate) status: Vec<String>,
    #[arg(long)]
    pub(crate) assignee: Option<String>,
    #[arg(long = "label")]
    pub(crate) labels: Vec<String>,
    #[arg(long)]
    pub(crate) include_archived: bool,
    #[arg(long, default_value_t = 20)]
    pub(crate) limit: usize,
    #[arg(long, default_value_t = 0)]
    pub(crate) offset: usize,
}

#[derive(Debug, Subcommand)]
pub(crate) enum IndexCommand {
    /// Show search index status.
    Status,
    /// Diagnose search index consistency.
    Doctor,
    /// Rebuild the search index.
    Rebuild,
    /// Synchronize pending search index work.
    Sync,
}

#[derive(Debug, Subcommand)]
pub(crate) enum EntityCommand {
    /// List derived entities.
    List(EntityListArgs),
    /// Show one derived entity by URI.
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
    /// List derived outbox items.
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
    /// Show derived-store status.
    Status,
}

#[derive(Debug, Subcommand)]
pub(crate) enum GraphCommand {
    /// Show graph index status.
    Status,
    /// List graph neighbors for an entity URI.
    Neighbors(GraphNeighborsArgs),
    /// Rebuild the graph index.
    Rebuild,
    /// Synchronize pending graph index work.
    Sync,
    /// Run a bounded SPARQL query against the graph index.
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
    pub(crate) sparql: Option<String>,
    #[arg(
        long = "sparql-file",
        value_name = "PATH|-",
        help = "Read SPARQL query from PATH, or stdin with -"
    )]
    pub(crate) sparql_file: Option<PathBuf>,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Debug, Subcommand)]
pub(crate) enum VectorCommand {
    /// Show vector index status and configuration.
    Status(VectorConfigPathArgs),
    /// Write local vector provider configuration.
    Configure(VectorConfigureArgs),
    /// Rebuild the vector index.
    Rebuild(VectorConfigPathArgs),
    /// Synchronize pending vector index work.
    Sync(VectorConfigPathArgs),
    /// Query task/context chunks by text.
    QueryChunks(VectorQueryChunksArgs),
    /// Query label atoms by text or supplied vector JSON.
    QueryLabelAtoms(VectorQueryLabelAtomsArgs),
}

#[derive(Debug, Args)]
pub(crate) struct VectorConfigPathArgs {
    #[arg(long = "vector-config", alias = "config")]
    pub(crate) vector_config: Option<std::path::PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct VectorQueryChunksArgs {
    pub(crate) text: String,
    #[arg(long, default_value_t = 10)]
    pub(crate) limit: usize,
    #[arg(long = "vector-config", alias = "config")]
    pub(crate) vector_config: Option<std::path::PathBuf>,
}

#[derive(Debug, Args)]
#[command(
    group(
        ArgGroup::new("query_input")
            .required(true)
            .multiple(false)
            .args(["text", "text_file", "vector_json", "vector_json_file"])
    ),
    after_help = "Examples:\n  kanban vector query-label-atoms --text-file query.txt --limit 5\n  kanban vector query-label-atoms --text-file - --polarity positive\n  kanban vector query-label-atoms --vector-json-file vector.json --include-vector\n  kanban vector query-label-atoms --vector-json-file - --include-vector"
)]
pub(crate) struct VectorQueryLabelAtomsArgs {
    #[arg(value_name = "TEXT")]
    pub(crate) text: Option<String>,
    #[arg(
        long = "text-file",
        value_name = "PATH|-",
        help = "Read query text from PATH, or stdin with -"
    )]
    pub(crate) text_file: Option<PathBuf>,
    #[arg(long = "vector-json", value_name = "JSON")]
    pub(crate) vector_json: Option<String>,
    #[arg(
        long = "vector-json-file",
        value_name = "PATH|-",
        help = "Read raw vector JSON from PATH, or stdin with -"
    )]
    pub(crate) vector_json_file: Option<PathBuf>,
    #[arg(long, default_value_t = 10)]
    pub(crate) limit: usize,
    #[arg(long)]
    pub(crate) board_id: Option<String>,
    #[arg(long = "embedding-model")]
    pub(crate) embedding_model: Option<String>,
    #[arg(long)]
    pub(crate) polarity: Option<String>,
    #[arg(long = "include-vector")]
    pub(crate) include_vector: bool,
    #[arg(long = "vector-config", alias = "config")]
    pub(crate) vector_config: Option<std::path::PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct VectorConfigureArgs {
    #[arg(long, default_value = kanban_local::DEFAULT_VECTOR_PROVIDER)]
    pub(crate) provider: String,
    #[arg(long, default_value = kanban_local::DEFAULT_OLLAMA_ENDPOINT)]
    pub(crate) endpoint: String,
    #[arg(long, default_value = kanban_local::DEFAULT_OLLAMA_EMBEDDING_MODEL)]
    pub(crate) model: String,
    #[arg(long, default_value_t = kanban_local::DEFAULT_OLLAMA_EMBEDDING_DIMENSIONS)]
    pub(crate) dimensions: usize,
    #[arg(long)]
    pub(crate) skip_check: bool,
    #[arg(long = "vector-config", alias = "config")]
    pub(crate) vector_config: Option<std::path::PathBuf>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ContextCommand {
    /// Build context for one task.
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
    #[arg(long = "vector-config", alias = "config")]
    pub(crate) vector_config: Option<std::path::PathBuf>,
}

#[derive(Debug, Args)]
#[command(
    after_help = "Examples:\n  kanban task update default#1 --description-file updated.md --expected-lock-version 7\n  kanban task update default#1 --clear-assignee --json\n\nUse file inputs for rich text or JSON; unspecified fields are left unchanged."
)]
pub(crate) struct UpdateArgs {
    pub(crate) task_ref: String,
    #[arg(long)]
    pub(crate) title: Option<String>,
    #[arg(long)]
    pub(crate) description: Option<String>,
    #[arg(
        long = "description-file",
        value_name = "PATH|-",
        help = "Read description text from PATH, or stdin with -; recommended for multiline or shell-sensitive text"
    )]
    pub(crate) description_file: Option<PathBuf>,
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
    #[arg(
        long = "metadata-file",
        value_name = "PATH|-",
        help = "Read task metadata JSON from PATH, or stdin with -; avoids shell quoting issues"
    )]
    pub(crate) metadata_file: Option<PathBuf>,
    #[arg(long)]
    pub(crate) expected_lock_version: Option<i64>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum RunCommand {
    /// Show one run by id.
    Show { run_id: String },
    /// Show stored run logs, optionally tailed by byte count.
    Logs {
        run_id: String,
        #[arg(long)]
        tail_bytes: Option<usize>,
    },
}

#[derive(Debug, Args, Clone)]
pub(crate) struct ClaimArgs {
    pub(crate) task_ref: String,
    #[arg(long, default_value_t = 300_000, allow_hyphen_values = true, value_parser = parse_positive_i64)]
    pub(crate) ttl_ms: i64,
}

#[derive(Debug, Args, Clone)]
pub(crate) struct HeartbeatArgs {
    pub(crate) task_ref: String,
    #[arg(long)]
    pub(crate) claim_token: String,
    #[arg(long, default_value_t = 300_000, allow_hyphen_values = true, value_parser = parse_positive_i64)]
    pub(crate) ttl_ms: i64,
}

fn parse_positive_i64(value: &str) -> std::result::Result<i64, String> {
    let parsed = value
        .parse::<i64>()
        .map_err(|err| format!("invalid integer for ttl_ms: {err}"))?;
    if parsed <= 0 {
        return Err("ttl_ms must be positive".to_owned());
    }
    Ok(parsed)
}

#[derive(Debug, Args, Clone)]
pub(crate) struct FinishArgs {
    pub(crate) task_ref: String,
    #[arg(long)]
    pub(crate) claim_token: Option<String>,
    #[arg(
        long,
        help = "Bypass normal finish guards when intentionally closing without an active claim"
    )]
    pub(crate) force: bool,
}

#[derive(Debug, Args)]
pub(crate) struct BlockArgs {
    pub(crate) task_ref: String,
    pub(crate) reason: Option<String>,
    #[arg(
        long = "reason-file",
        value_name = "PATH|-",
        help = "Read reason text from PATH, or stdin with -"
    )]
    pub(crate) reason_file: Option<PathBuf>,
    #[arg(long)]
    pub(crate) claim_token: Option<String>,
    #[arg(
        long,
        help = "Bypass normal block guards when intentionally blocking without an active claim"
    )]
    pub(crate) force: bool,
}

#[derive(Debug, Args)]
pub(crate) struct ReclaimArgs {
    #[arg(long)]
    pub(crate) expired: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum DepCommand {
    /// Add a dependency edge from parent to child.
    Add {
        parent_ref: String,
        child_ref: String,
    },
    /// Remove a dependency edge.
    Remove {
        parent_ref: String,
        child_ref: String,
    },
    /// List dependencies for one task.
    List { task_ref: String },
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
#[command(
    after_help = "Examples:\n  kanban import --input backup.jsonl --replace\n\n--replace clears existing importable records before loading the input file; use only with an intentional backup/restore flow."
)]
pub(crate) struct ImportArgs {
    #[arg(long)]
    pub(crate) input: PathBuf,
    #[arg(
        long,
        help = "Clear existing importable records before loading input; intended for restore flows"
    )]
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
