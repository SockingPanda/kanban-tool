use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    time::Duration,
};

use kanban_service::{
    BlockTaskCommand, ClaimRecord, ClaimTaskCommand, CompleteTaskCommand, HeartbeatTaskCommand,
    ReleaseTaskCommand, SubmitReviewTaskCommand, TaskListOptions, TaskListSort,
};
use kanban_service::{KanbanError, Result, TaskStatus};
use serde::Deserialize;
use tokio::{
    process::{Child, Command},
    sync::watch,
    time::{MissedTickBehavior, interval},
};
use tracing::{info, warn};

use crate::state::{AppState, KanbanService};

const DISPATCHER_ACTOR: &str = "dispatcher";
const DISPATCHER_WORKER_PROFILE: &str = "dispatcher";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownSignal {
    Running,
    Graceful,
    Force,
}

#[derive(Debug, Clone)]
pub struct DispatcherConfig {
    board: String,
    command: String,
    poll_interval: Duration,
    claim_ttl_ms: i64,
    heartbeat_interval: Duration,
    on_success: SuccessPolicy,
    on_failure: FailurePolicy,
    log_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DispatcherProfile {
    #[serde(default = "default_board")]
    board: String,
    command: String,
    #[serde(default = "default_poll_interval_ms")]
    poll_interval_ms: u64,
    #[serde(default = "default_claim_ttl_ms")]
    claim_ttl_ms: i64,
    #[serde(default = "default_heartbeat_interval_ms")]
    heartbeat_interval_ms: i64,
    #[serde(default)]
    on_success: SuccessPolicy,
    #[serde(default)]
    on_failure: FailurePolicy,
    log_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SuccessPolicy {
    #[default]
    Done,
    Review,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FailurePolicy {
    #[default]
    Blocked,
    Ready,
}

impl DispatcherConfig {
    /// 加载、校验并准备严格的单 worker dispatcher profile。
    ///
    /// 有意在 host 打开数据库之前调用。
    pub async fn load(profile_path: &Path) -> Result<Self> {
        let profile_path = tokio::fs::canonicalize(profile_path)
            .await
            .map_err(|error| {
                KanbanError::InvalidInput(format!(
                    "无法解析 dispatcher profile {}：{error}",
                    profile_path.display()
                ))
            })?;
        let source = tokio::fs::read_to_string(&profile_path)
            .await
            .map_err(|error| {
                KanbanError::InvalidInput(format!(
                    "无法读取 dispatcher profile {}：{error}",
                    profile_path.display()
                ))
            })?;
        let profile: DispatcherProfile = toml::from_str(&source).map_err(|error| {
            KanbanError::InvalidInput(format!(
                "dispatcher profile {} 无效：{error}",
                profile_path.display()
            ))
        })?;
        let board = profile.board.trim().to_owned();
        if board.is_empty() {
            return Err(KanbanError::InvalidInput(
                "dispatcher 必须提供 board".to_owned(),
            ));
        }
        let command = profile.command.trim().to_owned();
        if command.is_empty() {
            return Err(KanbanError::InvalidInput(
                "dispatcher 必须提供 command".to_owned(),
            ));
        }
        if profile.poll_interval_ms == 0 {
            return Err(KanbanError::InvalidInput(
                "dispatcher poll_interval_ms 必须为正数".to_owned(),
            ));
        }
        if profile.claim_ttl_ms <= 0 {
            return Err(KanbanError::InvalidInput(
                "dispatcher claim_ttl_ms 必须为正数".to_owned(),
            ));
        }
        if profile.heartbeat_interval_ms <= 0 {
            return Err(KanbanError::InvalidInput(
                "dispatcher heartbeat_interval_ms 必须为正数".to_owned(),
            ));
        }
        if profile.heartbeat_interval_ms >= profile.claim_ttl_ms {
            return Err(KanbanError::InvalidInput(
                "dispatcher heartbeat_interval_ms 必须小于 claim_ttl_ms".to_owned(),
            ));
        }
        let profile_dir = profile_path.parent().ok_or_else(|| {
            KanbanError::InvalidInput(format!(
                "dispatcher profile 没有父目录：{}",
                profile_path.display()
            ))
        })?;
        let configured_log_dir = profile.log_dir.unwrap_or_else(|| PathBuf::from("runs"));
        if configured_log_dir.as_os_str().is_empty() {
            return Err(KanbanError::InvalidInput(
                "dispatcher 必须提供 log_dir".to_owned(),
            ));
        }
        let log_dir = if configured_log_dir.is_absolute() {
            configured_log_dir
        } else {
            profile_dir.join(configured_log_dir)
        };
        tokio::fs::create_dir_all(&log_dir).await.map_err(|error| {
            KanbanError::Storage(format!(
                "无法创建 dispatcher 日志目录 {}：{error}",
                log_dir.display()
            ))
        })?;
        let log_dir = tokio::fs::canonicalize(&log_dir).await.map_err(|error| {
            KanbanError::Storage(format!(
                "无法解析 dispatcher 日志目录 {}：{error}",
                log_dir.display()
            ))
        })?;

        Ok(Self {
            board,
            command,
            poll_interval: Duration::from_millis(profile.poll_interval_ms),
            claim_ttl_ms: profile.claim_ttl_ms,
            heartbeat_interval: Duration::from_millis(
                u64::try_from(profile.heartbeat_interval_ms).map_err(|_| {
                    KanbanError::InvalidInput("dispatcher heartbeat_interval_ms 过大".to_owned())
                })?,
            ),
            on_success: profile.on_success,
            on_failure: profile.on_failure,
            log_dir,
        })
    }

    pub fn board(&self) -> &str {
        &self.board
    }

    pub fn log_dir(&self) -> &Path {
        &self.log_dir
    }
}

pub(crate) async fn run_dispatcher(
    state: AppState,
    config: DispatcherConfig,
    addr: SocketAddr,
    mut shutdown: watch::Receiver<ShutdownSignal>,
) -> Result<()> {
    loop {
        if *shutdown.borrow() != ShutdownSignal::Running {
            return Ok(());
        }

        if let Err(error) = state
            .application()
            .vector_worker_tick("vector-worker")
            .await
        {
            warn!(error = %error, "vector projection worker tick 失败；canonical 任务队列继续运行");
        }

        let reclaimed = state
            .application()
            .reclaim_expired(&config.board, DISPATCHER_ACTOR)
            .await?;
        if reclaimed > 0 {
            info!(reclaimed, "dispatcher 已回收过期任务 lease");
        }
        let Some(claim) = claim_next_ready(state.application(), &config).await? else {
            tokio::select! {
                _ = tokio::time::sleep(config.poll_interval) => {}
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() != ShutdownSignal::Running {
                        return Ok(());
                    }
                }
            }
            continue;
        };

        info!(
            task_id = %claim.task.id,
            run_id = %claim.run.id,
            "dispatcher 已 claim 任务"
        );
        run_claim(state.application(), &config, addr, &claim, &mut shutdown).await?;
    }
}

async fn claim_next_ready(
    application: &KanbanService,
    config: &DispatcherConfig,
) -> Result<Option<ClaimRecord>> {
    let page = application
        .list_tasks(
            &config.board,
            TaskListOptions {
                statuses: vec![TaskStatus::Ready],
                priorities: Vec::new(),
                labels: Vec::new(),
                plan_filters: Vec::new(),
                assignee: None,
                query: None,
                include_archived: false,
                limit: 1,
                offset: 0,
                sort: TaskListSort::Priority,
            },
        )
        .await?;
    let Some(task) = page.tasks.into_iter().next() else {
        return Ok(None);
    };
    match application
        .claim_task_with_run_log_dir(
            ClaimTaskCommand {
                task_id: task.id,
                actor: DISPATCHER_ACTOR.to_owned(),
                ttl_ms: config.claim_ttl_ms,
                worker_profile: Some(DISPATCHER_WORKER_PROFILE.to_owned()),
                metadata: serde_json::json!({"source": "dispatcher"}),
            },
            &config.log_dir,
        )
        .await
    {
        Ok(claim) => Ok(Some(claim)),
        Err(
            KanbanError::InvalidTransition(_)
            | KanbanError::ExecutionPlanRequired(_)
            | KanbanError::Conflict(_)
            | KanbanError::NotFound(_),
        ) => Ok(None),
        Err(error) => Err(error),
    }
}

async fn run_claim(
    application: &KanbanService,
    config: &DispatcherConfig,
    addr: SocketAddr,
    claim: &ClaimRecord,
    shutdown: &mut watch::Receiver<ShutdownSignal>,
) -> Result<()> {
    let log_path =
        claim.run.log_path.as_deref().ok_or_else(|| {
            KanbanError::Storage("dispatcher claim 未返回 run 日志路径".to_owned())
        })?;
    let (stdout, stderr) = match open_log(Path::new(log_path)) {
        Ok(files) => files,
        Err(error) => {
            finish_spawn_failure(application, config, claim, &error.to_string()).await?;
            return Ok(());
        }
    };
    let mut command = worker_command(&config.command);
    command
        .env("KB_BOARD_ID", &claim.task.board_id)
        .env("KB_BOARD_SLUG", &claim.task.board_slug)
        .env("KB_TASK_ID", &claim.task.id)
        .env("KB_TASK_SEQ", claim.task.seq.to_string())
        .env("KB_TASK_TITLE", &claim.task.title)
        .env("KB_CLAIM_TOKEN", &claim.claim_token)
        .env("KB_RUN_ID", &claim.run.id)
        .env("KB_ACTOR", DISPATCHER_ACTOR)
        .env("KANBAN_SERVER_URL", format!("http://{addr}"))
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .kill_on_drop(true);
    isolate_worker_from_dispatcher_interrupts(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            finish_spawn_failure(application, config, claim, &error.to_string()).await?;
            return Ok(());
        }
    };
    let status = wait_for_worker(application, config, claim, &mut child, shutdown).await?;
    finish_worker(application, config, claim, status).await
}

fn open_log(path: &Path) -> std::io::Result<(std::fs::File, std::fs::File)> {
    let stdout = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)?;
    let stderr = stdout.try_clone()?;
    Ok((stdout, stderr))
}

#[cfg(not(windows))]
fn worker_command(command: &str) -> Command {
    let mut worker = Command::new("sh");
    worker.arg("-c").arg(command);
    worker
}

#[cfg(windows)]
fn worker_command(command: &str) -> Command {
    let mut worker = Command::new("cmd.exe");
    worker.arg("/C").arg(command);
    worker
}

#[cfg(unix)]
fn isolate_worker_from_dispatcher_interrupts(command: &mut Command) {
    command.process_group(0);
}

#[cfg(not(unix))]
fn isolate_worker_from_dispatcher_interrupts(_command: &mut Command) {}

async fn wait_for_worker(
    application: &KanbanService,
    config: &DispatcherConfig,
    claim: &ClaimRecord,
    child: &mut Child,
    shutdown: &mut watch::Receiver<ShutdownSignal>,
) -> Result<ExitStatus> {
    let mut heartbeat = interval(config.heartbeat_interval);
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    heartbeat.tick().await;
    loop {
        tokio::select! {
            status = child.wait() => {
                return status.map_err(|error| {
                    KanbanError::Storage(format!("等待 dispatcher worker 失败：{error}"))
                });
            }
            _ = heartbeat.tick() => {
                if let Err(error) = application
                    .heartbeat_task(HeartbeatTaskCommand {
                        task_id: claim.task.id.clone(),
                        actor: DISPATCHER_ACTOR.to_owned(),
                        claim_token: claim.claim_token.clone(),
                        ttl_ms: config.claim_ttl_ms,
                        note: Some("dispatcher worker 心跳".to_owned()),
                    })
                    .await
                {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    let _ = application
                        .release_task(ReleaseTaskCommand {
                            task_id: claim.task.id.clone(),
                            actor: DISPATCHER_ACTOR.to_owned(),
                            claim_token: claim.claim_token.clone(),
                        })
                        .await;
                    return Err(error);
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() == ShutdownSignal::Force {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    return Err(KanbanError::Storage(
                        "dispatcher worker 被强制停止".to_owned(),
                    ));
                }
                // graceful shutdown 会有意让 active worker 继续运行。
            }
        }
    }
}

async fn finish_spawn_failure(
    application: &KanbanService,
    config: &DispatcherConfig,
    claim: &ClaimRecord,
    detail: &str,
) -> Result<()> {
    warn!(
        task_id = %claim.task.id,
        run_id = %claim.run.id,
        error = detail,
        "dispatcher 无法启动 worker"
    );
    match config.on_failure {
        FailurePolicy::Blocked => {
            application
                .block_task(BlockTaskCommand {
                    task_id: claim.task.id.clone(),
                    actor: DISPATCHER_ACTOR.to_owned(),
                    reason: format!("dispatcher 无法启动 worker：{detail}"),
                    claim_token: Some(claim.claim_token.clone()),
                    force: false,
                })
                .await?;
        }
        FailurePolicy::Ready => {
            application
                .release_task(ReleaseTaskCommand {
                    task_id: claim.task.id.clone(),
                    actor: DISPATCHER_ACTOR.to_owned(),
                    claim_token: claim.claim_token.clone(),
                })
                .await?;
        }
    }
    Ok(())
}

async fn finish_worker(
    application: &KanbanService,
    config: &DispatcherConfig,
    claim: &ClaimRecord,
    status: ExitStatus,
) -> Result<()> {
    let status_text = status.to_string();
    if status.success() {
        match config.on_success {
            SuccessPolicy::Done => {
                application
                    .complete_task(CompleteTaskCommand {
                        task_id: claim.task.id.clone(),
                        actor: DISPATCHER_ACTOR.to_owned(),
                        claim_token: Some(claim.claim_token.clone()),
                        force: false,
                        summary: Some(format!("dispatcher worker 已完成，状态为 {status_text}")),
                        result: Some(serde_json::json!({
                            "exit_code": status.code(),
                            "success": true,
                        })),
                    })
                    .await?;
            }
            SuccessPolicy::Review => {
                application
                    .submit_review_task(SubmitReviewTaskCommand {
                        task_id: claim.task.id.clone(),
                        actor: DISPATCHER_ACTOR.to_owned(),
                        claim_token: Some(claim.claim_token.clone()),
                        force: false,
                        summary: Some(format!("dispatcher worker 已完成，状态为 {status_text}")),
                    })
                    .await?;
            }
        }
    } else {
        match config.on_failure {
            FailurePolicy::Blocked => {
                application
                    .block_task(BlockTaskCommand {
                        task_id: claim.task.id.clone(),
                        actor: DISPATCHER_ACTOR.to_owned(),
                        reason: format!("dispatcher worker 失败，状态为 {status_text}"),
                        claim_token: Some(claim.claim_token.clone()),
                        force: false,
                    })
                    .await?;
            }
            FailurePolicy::Ready => {
                application
                    .release_task(ReleaseTaskCommand {
                        task_id: claim.task.id.clone(),
                        actor: DISPATCHER_ACTOR.to_owned(),
                        claim_token: claim.claim_token.clone(),
                    })
                    .await?;
            }
        }
    }
    info!(
        task_id = %claim.task.id,
        run_id = %claim.run.id,
        status = %status_text,
        "dispatcher worker 已结束"
    );
    Ok(())
}

fn default_board() -> String {
    "default".to_owned()
}

const fn default_poll_interval_ms() -> u64 {
    1_000
}

const fn default_claim_ttl_ms() -> i64 {
    300_000
}

const fn default_heartbeat_interval_ms() -> i64 {
    30_000
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, net::Ipv4Addr};

    use kanban_service::{
        ClaimTaskCommand, CreateTaskCommand, MarkExecutionPlanNotRequiredCommand,
        PromoteTaskCommand, SubmitReviewTaskCommand, VectorConfigureCommand,
    };

    use super::*;

    #[tokio::test]
    async fn profile_is_strict_and_resolves_log_dir_from_profile() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let profile = directory.path().join("dispatcher.toml");
        tokio::fs::write(
            &profile,
            r#"
command = "printf dispatcher"
poll_interval_ms = 5
claim_ttl_ms = 100
heartbeat_interval_ms = 10
on_success = "review"
on_failure = "ready"
log_dir = "worker-logs"
"#,
        )
        .await
        .expect("write profile");
        let config = DispatcherConfig::load(&profile)
            .await
            .expect("load profile");
        assert_eq!(config.board(), "default");
        assert_eq!(
            config.log_dir(),
            directory.path().join("worker-logs").as_path()
        );

        tokio::fs::write(
            &profile,
            r#"
command = "printf dispatcher"
concurrency = 2
"#,
        )
        .await
        .expect("write invalid profile");
        let error = DispatcherConfig::load(&profile)
            .await
            .expect_err("unknown fields must fail");
        assert!(matches!(
            error,
            KanbanError::InvalidInput(message) if message.contains("concurrency")
        ));
    }

    #[tokio::test]
    async fn profile_rejects_invalid_intervals_and_policy_shape() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let profile = directory.path().join("dispatcher.toml");
        tokio::fs::write(
            &profile,
            r#"
board = "default"
command = "true"
claim_ttl_ms = 10
heartbeat_interval_ms = 10
"#,
        )
        .await
        .expect("write profile");
        let error = DispatcherConfig::load(&profile)
            .await
            .expect_err("heartbeat equal to ttl must fail");
        assert!(matches!(
            error,
            KanbanError::InvalidInput(message) if message.contains("heartbeat_interval_ms")
        ));

        tokio::fs::write(
            &profile,
            r#"
command = "true"
on_success = "blocked"
"#,
        )
        .await
        .expect("write invalid policy");
        let error = DispatcherConfig::load(&profile)
            .await
            .expect_err("failure-only success policy must fail");
        assert!(matches!(
            error,
            KanbanError::InvalidInput(message) if message.contains("on_success")
        ));
    }

    #[tokio::test]
    async fn graceful_shutdown_finishes_current_ready_task_without_claiming_another_or_review() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = AppState::open(directory.path().join("kanban.db"), "test-host")
            .await
            .expect("open state");
        let application = state.application().clone();
        create_ready_task(&application, "t_dispatch_first", 0).await;
        create_ready_task(&application, "t_dispatch_second", 1).await;
        create_ready_task(&application, "t_dispatch_review", 2).await;
        let review_claim = application
            .claim_task(ClaimTaskCommand {
                task_id: "t_dispatch_review".to_owned(),
                actor: "review-worker".to_owned(),
                ttl_ms: 1_000,
                worker_profile: Some("manual".to_owned()),
                metadata: serde_json::json!({}),
            })
            .await
            .expect("claim review task");
        application
            .submit_review_task(SubmitReviewTaskCommand {
                task_id: "t_dispatch_review".to_owned(),
                actor: "review-worker".to_owned(),
                claim_token: Some(review_claim.claim_token),
                force: false,
                summary: None,
            })
            .await
            .expect("submit review task");

        let log_dir = directory.path().join("logs");
        tokio::fs::create_dir_all(&log_dir)
            .await
            .expect("create log directory");
        let config = DispatcherConfig {
            board: "default".to_owned(),
            command: "printf dispatcher-stdout; printf dispatcher-stderr >&2; sleep 0.15"
                .to_owned(),
            poll_interval: Duration::from_millis(5),
            claim_ttl_ms: 1_000,
            heartbeat_interval: Duration::from_millis(20),
            on_success: SuccessPolicy::Done,
            on_failure: FailurePolicy::Blocked,
            log_dir: log_dir.clone(),
        };
        let (shutdown_tx, shutdown_rx) = watch::channel(ShutdownSignal::Running);
        let dispatcher = tokio::spawn(run_dispatcher(
            state,
            config,
            SocketAddr::from((Ipv4Addr::LOCALHOST, 8721)),
            shutdown_rx,
        ));

        wait_for_status(&application, "t_dispatch_first", TaskStatus::Running).await;
        shutdown_tx
            .send(ShutdownSignal::Graceful)
            .expect("send graceful shutdown");
        tokio::time::timeout(Duration::from_secs(3), dispatcher)
            .await
            .expect("dispatcher shutdown timeout")
            .expect("dispatcher join")
            .expect("dispatcher result");

        let completed = application
            .get_task("t_dispatch_first")
            .await
            .expect("completed task");
        assert_eq!(completed.status, TaskStatus::Done);
        assert_eq!(
            application
                .get_task("t_dispatch_second")
                .await
                .expect("second task")
                .status,
            TaskStatus::Ready
        );
        assert_eq!(
            application
                .get_task("t_dispatch_review")
                .await
                .expect("review task")
                .status,
            TaskStatus::Review
        );

        let run_id = completed.current_run_id.expect("completed run id");
        let log = tokio::fs::read_to_string(log_dir.join(format!("{run_id}.log")))
            .await
            .expect("read worker log");
        assert!(log.contains("dispatcher-stdout"));
        assert!(log.contains("dispatcher-stderr"));
    }

    #[tokio::test]
    async fn dispatcher_tick_runs_vector_worker_and_publishes_ready_status() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = AppState::open(directory.path().join("kanban.db"), "test-host")
            .await
            .expect("open state");
        let application = state.application().clone();
        application
            .configure_vector(VectorConfigureCommand {
                provider: "ollama".to_owned(),
                endpoint: "http://127.0.0.1:1".to_owned(),
                model: "dispatcher-vector-model".to_owned(),
                dimensions: 2,
            })
            .await
            .expect("configure vector provider");
        let degraded = application
            .vector_status("default")
            .await
            .expect("vector status after configure");
        assert_eq!(degraded.dirty, Some(true));

        let config = test_config(directory.path());
        let (shutdown_tx, shutdown_rx) = watch::channel(ShutdownSignal::Running);
        let dispatcher = tokio::spawn(run_dispatcher(
            state,
            config,
            SocketAddr::from((Ipv4Addr::LOCALHOST, 8722)),
            shutdown_rx,
        ));

        wait_for_vector_ready(&application).await;
        shutdown_tx
            .send(ShutdownSignal::Graceful)
            .expect("send graceful shutdown");
        tokio::time::timeout(Duration::from_secs(3), dispatcher)
            .await
            .expect("dispatcher shutdown timeout")
            .expect("dispatcher join")
            .expect("dispatcher result");

        let status = application
            .vector_status("default")
            .await
            .expect("vector status");
        assert_eq!(status.dirty, Some(false));
        assert_eq!(status.pending_jobs, 0);
        assert_eq!(status.running_jobs, 0);
        assert_eq!(status.failed_jobs, 0);
    }

    #[tokio::test]
    async fn worker_finish_policies_reuse_review_block_and_release_commands() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = AppState::open(directory.path().join("kanban.db"), "test-host")
            .await
            .expect("open state");
        let application = state.application().clone();

        create_ready_task(&application, "t_dispatch_to_review", 0).await;
        let review_claim = dispatcher_claim(&application, "t_dispatch_to_review").await;
        let mut review_config = test_config(directory.path());
        review_config.on_success = SuccessPolicy::Review;
        finish_worker(
            &application,
            &review_config,
            &review_claim,
            worker_status("exit 0").await,
        )
        .await
        .expect("finish to review");
        assert_eq!(
            application
                .get_task("t_dispatch_to_review")
                .await
                .expect("review task")
                .status,
            TaskStatus::Review
        );

        create_ready_task(&application, "t_dispatch_to_blocked", 1).await;
        let blocked_claim = dispatcher_claim(&application, "t_dispatch_to_blocked").await;
        let mut blocked_config = test_config(directory.path());
        blocked_config.on_failure = FailurePolicy::Blocked;
        finish_worker(
            &application,
            &blocked_config,
            &blocked_claim,
            worker_status("exit 7").await,
        )
        .await
        .expect("finish to blocked");
        assert_eq!(
            application
                .get_task("t_dispatch_to_blocked")
                .await
                .expect("blocked task")
                .status,
            TaskStatus::Blocked
        );

        create_ready_task(&application, "t_dispatch_to_ready", 2).await;
        let ready_claim = dispatcher_claim(&application, "t_dispatch_to_ready").await;
        let mut ready_config = test_config(directory.path());
        ready_config.on_failure = FailurePolicy::Ready;
        finish_worker(
            &application,
            &ready_config,
            &ready_claim,
            worker_status("exit 9").await,
        )
        .await
        .expect("finish to ready");
        let ready = application
            .get_task("t_dispatch_to_ready")
            .await
            .expect("ready task");
        assert_eq!(ready.status, TaskStatus::Ready);
        assert_eq!(ready.current_run_id, None);
    }

    async fn dispatcher_claim(application: &KanbanService, task_id: &str) -> ClaimRecord {
        application
            .claim_task(ClaimTaskCommand {
                task_id: task_id.to_owned(),
                actor: DISPATCHER_ACTOR.to_owned(),
                ttl_ms: 1_000,
                worker_profile: Some(DISPATCHER_WORKER_PROFILE.to_owned()),
                metadata: serde_json::json!({"source": "dispatcher"}),
            })
            .await
            .expect("dispatcher claim")
    }

    fn test_config(directory: &Path) -> DispatcherConfig {
        DispatcherConfig {
            board: "default".to_owned(),
            command: "true".to_owned(),
            poll_interval: Duration::from_millis(5),
            claim_ttl_ms: 1_000,
            heartbeat_interval: Duration::from_millis(20),
            on_success: SuccessPolicy::Done,
            on_failure: FailurePolicy::Blocked,
            log_dir: directory.join("logs"),
        }
    }

    async fn worker_status(command: &str) -> ExitStatus {
        worker_command(command)
            .status()
            .await
            .expect("worker exit status")
    }

    async fn create_ready_task(application: &KanbanService, task_id: &str, priority: i64) {
        application
            .create_task(CreateTaskCommand {
                task_id: task_id.to_owned(),
                board: "default".to_owned(),
                idempotency_key: None,
                title: task_id.to_owned(),
                description: Some("dispatcher integration task".to_owned()),
                requested_status: None,
                assignee: None,
                priority,
                scheduled_at: None,
                due_at: None,
                max_retries: None,
                metadata: BTreeMap::new(),
                labels: Vec::new(),
                depends_on: Vec::new(),
                actor: "test".to_owned(),
            })
            .await
            .expect("create task");
        application
            .mark_execution_plan_not_required(MarkExecutionPlanNotRequiredCommand {
                task_id: task_id.to_owned(),
                reason: "test fixture".to_owned(),
                actor: "test".to_owned(),
            })
            .await
            .expect("mark plan not required");
        application
            .promote_task(PromoteTaskCommand {
                task_id: task_id.to_owned(),
                actor: "test".to_owned(),
            })
            .await
            .expect("promote task");
    }

    async fn wait_for_status(application: &KanbanService, task_id: &str, expected: TaskStatus) {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let status = application
                    .get_task(task_id)
                    .await
                    .expect("get task while polling")
                    .status;
                if status == expected {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("task status timeout");
    }

    async fn wait_for_vector_ready(application: &KanbanService) {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let status = application
                    .vector_status("default")
                    .await
                    .expect("get vector status while polling");
                if status.dirty == Some(false)
                    && status.pending_jobs == 0
                    && status.running_jobs == 0
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("vector status timeout");
    }
}
