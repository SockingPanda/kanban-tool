use crate::connect_file;

use super::{
    MAX_TASK_LIST_LIMIT, SignalListOptions, SignalObservationRecord, SignalRecord, all_values,
    board_id, required_row, resolve_task,
};

use std::path::Path;

use kanban_core::{KanbanError, Result};
use rusqlite::{Connection, Row, params, types::Value};

const REVIEW_SIGNAL_STATUSES: &[&str] = &["open", "confirmed"];
const SIGNAL_STATUSES: &[&str] = &["open", "confirmed", "rejected", "superseded", "resolved"];

pub fn list_signals(
    path: impl AsRef<Path>,
    board: &str,
    options: SignalListOptions,
) -> Result<Vec<SignalRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    list_signals_conn(&conn, &board_id, options)
}

pub fn review_signals(
    path: impl AsRef<Path>,
    board: &str,
    options: SignalListOptions,
) -> Result<Vec<SignalRecord>> {
    list_signals(path, board, options)
}

pub fn get_signal(path: impl AsRef<Path>, signal_id: &str) -> Result<SignalRecord> {
    let conn = connect_file(path.as_ref())?;
    required_row(
        &conn,
        &signal_select_sql("WHERE s.id=?1"),
        params![signal_id],
        signal_from_row,
        || KanbanError::NotFound(format!("signal {signal_id}")),
    )
}

fn list_signals_conn(
    conn: &Connection,
    board_id: &str,
    options: SignalListOptions,
) -> Result<Vec<SignalRecord>> {
    validate_signal_limit(options.limit)?;

    let statuses = normalized_statuses(&options.statuses, options.include_all)?;
    let kinds = normalized_kinds(&options.kinds);
    let task_id = match options
        .task_ref
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(task_ref) => {
            let task = resolve_task(conn, board_id, task_ref)?;
            if task.board_id != board_id {
                return Err(KanbanError::InvalidInput(format!(
                    "task {task_ref} is not on the requested board"
                )));
            }
            Some(task.id)
        }
        None => None,
    };

    let mut conditions = vec!["s.board_id=?".to_owned()];
    let mut values = vec![Value::Text(board_id.to_owned())];

    if !statuses.is_empty() {
        conditions.push(format!("s.status IN ({})", placeholders(statuses.len())));
        values.extend(statuses.into_iter().map(Value::Text));
    }
    if !kinds.is_empty() {
        conditions.push(format!("s.kind IN ({})", placeholders(kinds.len())));
        values.extend(kinds.into_iter().map(Value::Text));
    }
    if let Some(task_id) = task_id {
        conditions.push("o.task_id=?".to_owned());
        values.push(Value::Text(task_id));
    }

    values.push(Value::Integer(options.limit as i64));
    let sql = format!(
        "{} WHERE {} ORDER BY s.created_at DESC, s.id DESC LIMIT ?",
        signal_select_sql(""),
        conditions.join(" AND ")
    );
    all_values(conn, &sql, &values, signal_from_row)
}

fn normalized_statuses(statuses: &[String], include_all: bool) -> Result<Vec<String>> {
    if statuses.is_empty() {
        return Ok(if include_all {
            Vec::new()
        } else {
            REVIEW_SIGNAL_STATUSES
                .iter()
                .map(|status| (*status).to_owned())
                .collect()
        });
    }

    let mut normalized = Vec::new();
    for status in statuses {
        let status = status.trim();
        if status.is_empty() {
            continue;
        }
        if !SIGNAL_STATUSES.contains(&status) {
            return Err(KanbanError::InvalidInput(format!(
                "invalid signal status: {status}"
            )));
        }
        if !normalized.iter().any(|existing| existing == status) {
            normalized.push(status.to_owned());
        }
    }
    Ok(normalized)
}

fn normalized_kinds(kinds: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for kind in kinds {
        let kind = kind.trim();
        if kind.is_empty() {
            continue;
        }
        if !normalized.iter().any(|existing| existing == kind) {
            normalized.push(kind.to_owned());
        }
    }
    normalized
}

fn validate_signal_limit(limit: usize) -> Result<()> {
    if limit > MAX_TASK_LIST_LIMIT {
        return Err(KanbanError::InvalidInput(format!(
            "limit must be <= {MAX_TASK_LIST_LIMIT}"
        )));
    }
    Ok(())
}

fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(",")
}

fn signal_select_sql(where_sql: &str) -> String {
    format!(
        "SELECT \
            s.id,s.board_id,s.observation_id,s.kind,s.title,s.summary,s.severity,s.status,\
            s.dedupe_key,s.superseded_by_signal_id,s.reviewed_by,s.reviewed_at,s.review_reason,\
            s.created_at,s.updated_at,\
            o.id,o.board_id,o.task_id,o.task_ref_snapshot,o.run_id,o.comment_id,o.actor,\
            o.agent_type,o.source,o.evidence_json,o.created_at \
         FROM signals s \
         JOIN signal_observations o ON o.id=s.observation_id AND o.board_id=s.board_id \
         {where_sql}"
    )
}

fn signal_from_row(row: &Row<'_>) -> rusqlite::Result<SignalRecord> {
    Ok(SignalRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        observation_id: row.get(2)?,
        kind: row.get(3)?,
        title: row.get(4)?,
        summary: row.get(5)?,
        severity: row.get(6)?,
        status: row.get(7)?,
        dedupe_key: row.get(8)?,
        superseded_by_signal_id: row.get(9)?,
        reviewed_by: row.get(10)?,
        reviewed_at: row.get(11)?,
        review_reason: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        observation: SignalObservationRecord {
            id: row.get(15)?,
            board_id: row.get(16)?,
            task_id: row.get(17)?,
            task_ref_snapshot: row.get(18)?,
            run_id: row.get(19)?,
            comment_id: row.get(20)?,
            actor: row.get(21)?,
            agent_type: row.get(22)?,
            source: row.get(23)?,
            evidence_json: row.get(24)?,
            created_at: row.get(25)?,
        },
    })
}
