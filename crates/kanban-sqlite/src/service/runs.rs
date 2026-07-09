use crate::db::connect_file;

use super::{RunRecord, all, board_id_any, required_row, resolve_task_any};

use std::path::Path;

use kanban_core::{KanbanError, Result};

use rusqlite::{Row, params};

pub fn list_runs(
    path: impl AsRef<Path>,
    board: &str,
    task_ref: Option<&str>,
) -> Result<Vec<RunRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id_any(&conn, board)?;
    let task_id = task_ref
        .map(|r| resolve_task_any(&conn, &board_id, r).map(|t| t.id))
        .transpose()?;
    let sql = if task_id.is_some() {
        "SELECT id,task_id,status,worker_profile,worker_pid,claim_token,claim_owner,started_at,finished_at,exit_code,summary,error,log_path,metadata_json FROM task_runs WHERE board_id=?1 AND task_id=?2 ORDER BY started_at DESC"
    } else {
        "SELECT id,task_id,status,worker_profile,worker_pid,claim_token,claim_owner,started_at,finished_at,exit_code,summary,error,log_path,metadata_json FROM task_runs WHERE board_id=?1 ORDER BY started_at DESC"
    };
    if let Some(task_id) = task_id {
        all(&conn, sql, params![board_id, task_id], run_from_row)
    } else {
        all(&conn, sql, params![board_id], run_from_row)
    }
}

pub fn get_run_by_id_global(path: impl AsRef<Path>, run_id: &str) -> Result<RunRecord> {
    let conn = connect_file(path.as_ref())?;
    required_row(
        &conn,
        "SELECT id,task_id,status,worker_profile,worker_pid,claim_token,claim_owner,started_at,finished_at,exit_code,summary,error,log_path,metadata_json FROM task_runs WHERE id=?1",
        [run_id],
        run_from_row,
        || KanbanError::NotFound(format!("run {run_id}")),
    )
}

pub(crate) fn run_from_row(row: &Row<'_>) -> rusqlite::Result<RunRecord> {
    Ok(RunRecord {
        id: row.get(0)?,
        task_id: row.get(1)?,
        status: row.get(2)?,
        worker_profile: row.get(3)?,
        worker_pid: row.get(4)?,
        claim_token: row.get(5)?,
        claim_owner: row.get(6)?,
        started_at: row.get(7)?,
        finished_at: row.get(8)?,
        exit_code: row.get(9)?,
        summary: row.get(10)?,
        error: row.get(11)?,
        log_path: row.get(12)?,
        metadata_json: row.get(13)?,
    })
}
