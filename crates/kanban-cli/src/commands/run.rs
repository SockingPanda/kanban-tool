use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use kanban_sqlite::api::{get_run_by_id_global, resolve_run_log_path};

use crate::args::RunCommand;
use crate::output::{api_run_from_record, print_contract_or_human, print_human};

pub(crate) fn handle_run(command: RunCommand, db_path: &PathBuf, json: bool) -> Result<()> {
    match command {
        RunCommand::Show { run_id } => {
            let run = get_run_by_id_global(db_path, &run_id)?;
            if json {
                let output = kanban_contract::CliRunShowOutput::new(api_run_from_record(&run)?);
                print_contract_or_human(true, &output, String::new)?;
            } else {
                print_human(|| {
                    format!(
                        "{} [{}] task={} exit={:?}",
                        run.id, run.status, run.task_id, run.exit_code
                    )
                })?;
            }
        }
        RunCommand::Logs { run_id, tail_bytes } => {
            let log = read_run_log(db_path, &run_id, tail_bytes)?;
            let output = kanban_contract::CliRunLogsOutput::new(log.clone());
            print_contract_or_human(json, &output, || log.content.clone())?;
        }
    }
    Ok(())
}

fn read_run_log(
    db_path: &PathBuf,
    run_id: &str,
    tail_bytes: Option<usize>,
) -> Result<kanban_contract::CliRunLog> {
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
    Ok(kanban_contract::CliRunLog {
        run_id: run_id.to_owned(),
        content: String::from_utf8_lossy(&bytes[start..]).into_owned(),
        truncated,
        tail_bytes: tail_bytes
            .map(u64::try_from)
            .transpose()
            .map_err(|_| anyhow::anyhow!("tail byte count exceeds u64"))?,
    })
}
