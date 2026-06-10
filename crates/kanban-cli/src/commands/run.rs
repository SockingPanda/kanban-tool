use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use kanban_sqlite::{get_run_by_id_global, resolve_run_log_path};

use crate::args::RunCommand;
use crate::output::print_or_json;

pub(crate) fn handle_run(command: RunCommand, db_path: &PathBuf, json: bool) -> Result<()> {
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
    let log_path = run
        .log_path
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("run log not found for {run_id}"))?;
    let path = resolve_run_log_path(db_path, run_id, log_path)?;
    let bytes =
        fs::read(&path).with_context(|| format!("failed to read run log {}", path.display()))?;
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
