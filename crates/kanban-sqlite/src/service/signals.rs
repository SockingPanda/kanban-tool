use crate::db::connect_file;

use super::{
    CommentRecord, MAX_TASK_LIST_LIMIT, SignalLifecycle, SignalListOptions,
    SignalObservationRecord, SignalRecord, SignalRecordInput, SignalRecordResult,
    SignalReviewInput, SignalStatus, all_values, board_id, ensure_changed_one, exec, insert_event,
    required_row, resolve_task, with_immediate_tx,
};

use std::{collections::HashSet, path::Path, str::FromStr};

use kanban_core::{Clock, KanbanError, Result, SystemClock, new_typed_id};
use rusqlite::{Connection, OptionalExtension, Row, params, types::Value};
use serde_json::json;

const REVIEW_SIGNAL_STATUSES: &[SignalStatus] = &[SignalStatus::Open, SignalStatus::Confirmed];

pub fn record_signal(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    input: SignalRecordInput,
) -> Result<SignalRecordResult> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let actor = normalize_required(input.actor.as_deref().unwrap_or(actor), "actor")?;
        let kind = normalize_required(&input.kind, "signal kind")?;
        let title = normalize_required(&input.title, "signal title")?;
        let summary = normalize_required(&input.summary, "signal summary")?;
        let severity =
            normalize_optional(input.severity.as_deref()).unwrap_or_else(|| "info".to_owned());
        let evidence_json = normalize_json_object(input.evidence.as_ref(), "evidence")?;

        let task = match input.task_ref.as_deref().or(input.task_id.as_deref()) {
            Some(task_ref) => Some(resolve_task(&conn, &board_id, task_ref)?),
            None => None,
        };
        if let Some(run_id) = input.run_id.as_deref() {
            ensure_board_row(&conn, "task_runs", run_id, &board_id, "run")?;
        }
        if let Some(comment_id) = input.comment_id.as_deref() {
            ensure_board_row(&conn, "task_comments", comment_id, &board_id, "comment")?;
        }

        let observation_id = new_typed_id("obs");
        let signal_id = new_typed_id("sig");
        let task_ref_snapshot = task.as_ref().map(|task| task.task_ref.clone());
        exec(
            &conn,
            "INSERT INTO signal_observations(id, board_id, task_id, task_ref_snapshot, run_id, comment_id, actor, agent_type, source, evidence_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                observation_id,
                board_id,
                task.as_ref().map(|task| task.id.as_str()),
                task_ref_snapshot,
                input.run_id,
                input.comment_id,
                actor,
                normalize_optional(input.agent_type.as_deref()),
                normalize_optional(input.source.as_deref()),
                evidence_json,
                now
            ],
        )?;
        exec(
            &conn,
            "INSERT INTO signals(id, board_id, observation_id, kind, title, summary, severity, status, dedupe_key, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'open', ?8, ?9, ?9)",
            params![
                signal_id,
                board_id,
                observation_id,
                kind,
                title,
                summary,
                severity,
                normalize_optional(input.dedupe_key.as_deref()),
                now
            ],
        )?;

        let backlink_comment = if let Some(task) = task.as_ref() {
            let body = input
                .comment
                .and_then(|comment| normalize_optional(comment.body.as_deref()))
                .unwrap_or_else(|| format!("Signal: {title}"));
            let metadata = json!({
                "type": "signal_link",
                "signal_id": signal_id,
                "observation_id": observation_id,
                "signal_kind": kind,
                "signal_status": "open"
            })
            .to_string();
            Some(insert_signal_comment_in_tx(
                &conn,
                SignalCommentInsert {
                    board_id: &board_id,
                    task_id: &task.id,
                    author: actor.as_str(),
                    agent_type: input.agent_type.as_deref(),
                    body: &body,
                    metadata_json: &metadata,
                    now,
                },
            )?)
        } else {
            None
        };

        insert_event(
            &conn,
            &board_id,
            task.as_ref().map(|task| task.id.as_str()),
            input.run_id.as_deref(),
            "signal.recorded",
            actor.as_str(),
            &json!({"signal_id": signal_id, "observation_id": observation_id, "kind": kind, "status": "open"}).to_string(),
            now,
        )?;
        get_signal_result_in_tx(&conn, &signal_id, backlink_comment)
    })
}

pub fn list_signals(
    path: impl AsRef<Path>,
    board: &str,
    options: SignalListOptions,
) -> Result<Vec<SignalRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    list_signals_conn(&conn, &board_id, options)
}

pub fn get_signal(path: impl AsRef<Path>, board: &str, signal_id: &str) -> Result<SignalRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    get_signal_for_board_in_tx(&conn, &board_id, signal_id)
}

pub fn get_signal_by_id(path: impl AsRef<Path>, signal_id: &str) -> Result<SignalRecord> {
    let conn = connect_file(path.as_ref())?;
    get_signal_by_id_in_tx(&conn, signal_id)
}

pub fn review_signals(
    path: impl AsRef<Path>,
    board: &str,
    mut options: SignalListOptions,
) -> Result<Vec<SignalRecord>> {
    options.include_all = false;
    list_signals(path, board, options)
}

pub fn update_signal_status(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    input: SignalReviewInput,
) -> Result<Vec<SignalRecord>> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        if input.signal_ids.is_empty() {
            return Err(KanbanError::InvalidInput(
                "at least one signal id is required".into(),
            ));
        }
        let actor = normalize_required(actor, "actor")?;
        let reason = normalize_required(&input.reason, "reason")?;
        if matches!(input.lifecycle, SignalLifecycle::Supersede)
            && input.replacement_signal_id.is_none()
        {
            return Err(KanbanError::InvalidInput(
                "supersede requires replacement signal id".into(),
            ));
        }
        let mut out = Vec::new();
        for signal_id in &input.signal_ids {
            let current = get_signal_for_board_in_tx(&conn, &board_id, signal_id)?;
            let target = target_status(input.lifecycle);
            validate_signal_transition(&current, target)?;
            let replacement = if let Some(replacement) = input.replacement_signal_id.as_deref() {
                let replacement_signal = get_signal_for_board_in_tx(&conn, &board_id, replacement)?;
                if replacement_signal.board_id != current.board_id {
                    return Err(KanbanError::InvalidInput(
                        "replacement signal must be on the same board".into(),
                    ));
                }
                if replacement_signal.id == current.id
                    || supersede_path_contains(&conn, replacement, &current.id)?
                {
                    return Err(KanbanError::InvalidInput(
                        "signal supersede cycle detected".into(),
                    ));
                }
                Some(replacement.to_owned())
            } else {
                None
            };
            ensure_changed_one(
                conn.execute(
                    "UPDATE signals SET status=?1, superseded_by_signal_id=?2, reviewed_by=?3, reviewed_at=?4, review_reason=?5, updated_at=?4 WHERE id=?6 AND board_id=?7",
                    params![target.to_string(), replacement, actor, now, reason, signal_id, board_id],
                )
                .map_err(|err| KanbanError::Storage(err.to_string()))?,
                || KanbanError::NotFound(format!("signal not found: {signal_id}")),
            )?;
            insert_event(
                &conn,
                &current.board_id,
                current.observation.task_id.as_deref(),
                current.observation.run_id.as_deref(),
                "signal.reviewed",
                actor.as_str(),
                &json!({"signal_id": signal_id, "status": target, "reason": reason}).to_string(),
                now,
            )?;
            out.push(get_signal_for_board_in_tx(&conn, &board_id, signal_id)?);
        }
        Ok(out)
    })
}

struct SignalCommentInsert<'a> {
    board_id: &'a str,
    task_id: &'a str,
    author: &'a str,
    agent_type: Option<&'a str>,
    body: &'a str,
    metadata_json: &'a str,
    now: i64,
}

fn insert_signal_comment_in_tx(
    conn: &Connection,
    input: SignalCommentInsert<'_>,
) -> Result<CommentRecord> {
    let id = new_typed_id("c");
    exec(
        conn,
        "INSERT INTO task_comments(id, board_id, task_id, author, author_type, agent_type, body, kind, metadata_json, created_at) VALUES (?1, ?2, ?3, ?4, 'agent', ?5, ?6, 'signal', ?7, ?8)",
        params![
            id,
            input.board_id,
            input.task_id,
            input.author,
            normalize_optional(input.agent_type),
            input.body,
            input.metadata_json,
            input.now
        ],
    )?;
    insert_event(
        conn,
        input.board_id,
        Some(input.task_id),
        None,
        "task.comment.created",
        input.author,
        &json!({"comment_id": id, "kind": "signal", "author_type": "agent", "agent_type": input.agent_type}).to_string(),
        input.now,
    )?;
    Ok(CommentRecord {
        id,
        board_id: input.board_id.to_owned(),
        task_id: input.task_id.to_owned(),
        author: input.author.to_owned(),
        author_type: "agent".to_owned(),
        agent_type: input.agent_type.map(str::to_owned),
        body: input.body.to_owned(),
        kind: "signal".to_owned(),
        metadata_json: input.metadata_json.to_owned(),
        created_at: input.now,
    })
}

fn get_signal_result_in_tx(
    conn: &Connection,
    signal_id: &str,
    backlink_comment: Option<CommentRecord>,
) -> Result<SignalRecordResult> {
    Ok(SignalRecordResult {
        signal: get_signal_by_id_in_tx(conn, signal_id)?,
        backlink_comment,
    })
}

fn get_signal_by_id_in_tx(conn: &Connection, signal_id: &str) -> Result<SignalRecord> {
    required_row(
        conn,
        &signal_select_sql("WHERE s.id=?1"),
        params![signal_id],
        signal_from_row,
        || KanbanError::NotFound(format!("signal not found: {signal_id}")),
    )
}

fn get_signal_for_board_in_tx(
    conn: &Connection,
    board_id: &str,
    signal_id: &str,
) -> Result<SignalRecord> {
    required_row(
        conn,
        &signal_select_sql("WHERE s.id=?1 AND s.board_id=?2"),
        params![signal_id, board_id],
        signal_from_row,
        || KanbanError::NotFound(format!("signal not found on board: {signal_id}")),
    )
}

fn list_signals_conn(
    conn: &Connection,
    board_id: &str,
    options: SignalListOptions,
) -> Result<Vec<SignalRecord>> {
    let limit = normalize_signal_limit(options.limit)?;
    let statuses = normalized_statuses(&options.statuses, options.include_all);
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

    values.push(Value::Integer(limit as i64));
    let sql = format!(
        "{} WHERE {} ORDER BY s.created_at DESC, s.id DESC LIMIT ?",
        signal_select_sql(""),
        conditions.join(" AND ")
    );
    all_values(conn, &sql, &values, signal_from_row)
}

fn normalized_statuses(statuses: &[SignalStatus], include_all: bool) -> Vec<String> {
    if statuses.is_empty() {
        return if include_all {
            Vec::new()
        } else {
            REVIEW_SIGNAL_STATUSES
                .iter()
                .map(ToString::to_string)
                .collect()
        };
    }

    let mut normalized = Vec::new();
    for status in statuses {
        let status = status.to_string();
        if !normalized.iter().any(|existing| existing == &status) {
            normalized.push(status);
        }
    }
    normalized
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

fn normalize_signal_limit(limit: usize) -> Result<usize> {
    if limit > MAX_TASK_LIST_LIMIT {
        return Err(KanbanError::InvalidInput(format!(
            "limit must be <= {MAX_TASK_LIST_LIMIT}"
        )));
    }
    Ok(limit.clamp(1, MAX_TASK_LIST_LIMIT))
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

fn ensure_board_row(
    conn: &Connection,
    table: &str,
    id: &str,
    board_id: &str,
    label: &str,
) -> Result<()> {
    let found: Option<String> = conn
        .query_row(
            &format!("SELECT board_id FROM {table} WHERE id=?1"),
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|err| KanbanError::Storage(err.to_string()))?;
    match found {
        Some(found) if found == board_id => Ok(()),
        Some(_) => Err(KanbanError::InvalidInput(format!(
            "{label} belongs to a different board"
        ))),
        None => Err(KanbanError::NotFound(format!("{label} not found: {id}"))),
    }
}

fn normalize_required(value: &str, field: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(KanbanError::InvalidInput(format!("{field} is required")));
    }
    Ok(value.to_owned())
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn normalize_json_object(value: Option<&serde_json::Value>, field: &str) -> Result<String> {
    match value {
        None | Some(serde_json::Value::Null) => Ok("{}".to_owned()),
        Some(value @ serde_json::Value::Object(_)) => Ok(value.to_string()),
        Some(_) => Err(KanbanError::InvalidInput(format!(
            "{field} must be a JSON object"
        ))),
    }
}

fn target_status(lifecycle: SignalLifecycle) -> SignalStatus {
    match lifecycle {
        SignalLifecycle::Confirm => SignalStatus::Confirmed,
        SignalLifecycle::Reject => SignalStatus::Rejected,
        SignalLifecycle::Resolve => SignalStatus::Resolved,
        SignalLifecycle::Supersede => SignalStatus::Superseded,
    }
}

fn validate_signal_transition(current: &SignalRecord, target: SignalStatus) -> Result<()> {
    let current_status = SignalStatus::from_str(&current.status)?;
    let ok = matches!(
        (current_status, target),
        (SignalStatus::Open, SignalStatus::Confirmed)
            | (SignalStatus::Open, SignalStatus::Rejected)
            | (SignalStatus::Open, SignalStatus::Superseded)
            | (SignalStatus::Open, SignalStatus::Resolved)
            | (SignalStatus::Confirmed, SignalStatus::Resolved)
    );
    if ok {
        Ok(())
    } else {
        Err(KanbanError::InvalidInput(format!(
            "invalid signal transition: {} -> {target}",
            current.status
        )))
    }
}

fn supersede_path_contains(conn: &Connection, start: &str, needle: &str) -> Result<bool> {
    let mut seen = HashSet::new();
    let mut current = Some(start.to_owned());
    while let Some(id) = current {
        if id == needle {
            return Ok(true);
        }
        if !seen.insert(id.clone()) {
            return Ok(true);
        }
        current = conn
            .query_row(
                "SELECT superseded_by_signal_id FROM signals WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| KanbanError::Storage(err.to_string()))?
            .flatten();
    }
    Ok(false)
}
