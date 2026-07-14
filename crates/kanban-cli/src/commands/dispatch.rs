use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result};
use kanban_contract::cli_operator::{
    CliDispatchLoopResult, CliDispatchRunResult, CliDispatchStopReason,
};
use kanban_sqlite::api::{DispatchOptions, FinishPolicy, dispatch_once};

use crate::args::{DispatchArgs, DispatchLoopSummary, WorkerProfileConfig};
use crate::commands::common::invalid_input;

pub(crate) fn dispatch_options(args: &DispatchArgs, actor: String) -> Result<DispatchOptions> {
    let profile = args
        .profile_config
        .as_ref()
        .map(|path| load_worker_profile(path, &args.worker_profile))
        .transpose()?;
    let log_dir = profile
        .as_ref()
        .and_then(|profile| profile.log_dir.clone())
        .or_else(|| args.log_dir.clone())
        .unwrap_or_else(|| kanban_local::default_log_dir().join("runs"));
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

pub(crate) async fn dispatch_loop(
    db_path: &PathBuf,
    board: &str,
    options: DispatchOptions,
    poll_interval_ms: u64,
    max_iterations: Option<usize>,
) -> Result<DispatchLoopSummary> {
    let mut iterations = 0;
    let mut claimed = 0;
    let mut runs = Vec::new();
    let mut stop_reason = None;
    let interrupt_count = install_dispatch_interrupt_listener();
    'dispatch: loop {
        iterations += 1;
        let result = dispatch_once(db_path, board, options.clone())?;
        claimed += result.claimed;
        runs.push(result);
        if interrupt_count.load(Ordering::SeqCst) > 0 {
            stop_reason = Some("interrupted".to_owned());
            break;
        }
        if max_iterations.is_some_and(|max| iterations >= max) {
            break;
        }
        let mut remaining = Duration::from_millis(poll_interval_ms);
        while !remaining.is_zero() {
            if interrupt_count.load(Ordering::SeqCst) > 0 {
                stop_reason = Some("interrupted".to_owned());
                break 'dispatch;
            }
            let step = remaining.min(Duration::from_millis(50));
            tokio::time::sleep(step).await;
            remaining = remaining.saturating_sub(step);
        }
    }
    Ok(DispatchLoopSummary {
        iterations,
        claimed,
        runs,
        stop_reason,
    })
}

pub(crate) fn cli_dispatch_run_result(
    result: &kanban_sqlite::api::DispatchResult,
) -> CliDispatchRunResult {
    CliDispatchRunResult {
        claimed: result.claimed,
        task_id: result.task_id.clone(),
        run_id: result.run_id.clone(),
        exit_code: result.exit_code,
    }
}

pub(crate) fn cli_dispatch_loop_result(
    summary: &DispatchLoopSummary,
) -> Result<CliDispatchLoopResult> {
    let stop_reason = match summary.stop_reason.as_deref() {
        None => None,
        Some("interrupted") => Some(CliDispatchStopReason::Interrupted),
        Some(value) => anyhow::bail!("dispatch output has invalid stop reason {value}"),
    };
    Ok(CliDispatchLoopResult {
        iterations: summary.iterations,
        claimed: summary.claimed,
        runs: summary.runs.iter().map(cli_dispatch_run_result).collect(),
        stop_reason,
    })
}

fn install_dispatch_interrupt_listener() -> Arc<AtomicU8> {
    let interrupt_count = Arc::new(AtomicU8::new(0));
    let listener_count = Arc::clone(&interrupt_count);
    tokio::spawn(async move {
        while tokio::signal::ctrl_c().await.is_ok() {
            let previous = listener_count.fetch_add(1, Ordering::SeqCst);
            if previous == 0 {
                eprintln!("received Ctrl-C; stopping dispatch loop after current iteration");
            } else {
                std::process::exit(130);
            }
        }
    });
    interrupt_count
}

pub(crate) fn absolute_path(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()
            .context("failed to resolve current directory")?
            .join(path))
    }
}

pub(crate) fn load_worker_profile(path: &Path, profile_name: &str) -> Result<WorkerProfileConfig> {
    let document = kanban_local::read_worker_profiles(path)
        .with_context(|| format!("failed to read worker profile config {}", path.display()))?;
    let profile = document.workers.get(profile_name).ok_or_else(|| {
        invalid_input(format!(
            "worker profile {profile_name} not found in {}",
            path.display()
        ))
    })?;
    Ok(WorkerProfileConfig {
        command: profile.command.clone(),
        claim_ttl_ms: profile.claim_ttl_ms,
        heartbeat_interval_ms: profile.heartbeat_interval_ms,
        on_success: profile.on_success.map(finish_policy),
        on_failure: profile.on_failure.map(finish_policy),
        log_dir: profile.log_dir.clone(),
    })
}

fn finish_policy(value: kanban_local::WorkerFinishPolicy) -> FinishPolicy {
    match value {
        kanban_local::WorkerFinishPolicy::Done => FinishPolicy::Done,
        kanban_local::WorkerFinishPolicy::Review => FinishPolicy::Review,
        kanban_local::WorkerFinishPolicy::Blocked => FinishPolicy::Blocked,
        kanban_local::WorkerFinishPolicy::Ready => FinishPolicy::Ready,
    }
}
