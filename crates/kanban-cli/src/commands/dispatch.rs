use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result};
use kanban_sqlite::{DispatchOptions, FinishPolicy, dispatch_once};

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

pub(crate) fn load_worker_profile(
    path: &PathBuf,
    profile_name: &str,
) -> Result<WorkerProfileConfig> {
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
            return Err(invalid_input(format!(
                "invalid worker profile line: {raw_line}"
            )));
        };
        let key = key.trim();
        let value = unquote(value.trim());
        match key {
            "command" => profile.command = Some(value.to_owned()),
            "claim_ttl_ms" => {
                profile.claim_ttl_ms = Some(value.parse().map_err(|_| {
                    invalid_input(format!(
                        "worker profile claim_ttl_ms must be an integer: {value}"
                    ))
                })?)
            }
            "heartbeat_interval_ms" => {
                profile.heartbeat_interval_ms = Some(value.parse().map_err(|_| {
                    invalid_input(format!(
                        "worker profile heartbeat_interval_ms must be an integer: {value}"
                    ))
                })?)
            }
            "on_success" => profile.on_success = Some(parse_finish_policy(value)?),
            "on_failure" => profile.on_failure = Some(parse_finish_policy(value)?),
            "log_dir" => profile.log_dir = Some(PathBuf::from(value)),
            _ => {
                return Err(invalid_input(format!(
                    "unsupported worker profile key: {key}"
                )));
            }
        }
    }
    if !found {
        return Err(invalid_input(format!(
            "worker profile {profile_name} not found in {}",
            path.display()
        )));
    }
    Ok(profile)
}

pub(crate) fn unquote(value: &str) -> &str {
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
        _ => Err(invalid_input(format!("unsupported finish policy: {value}"))),
    }
}
