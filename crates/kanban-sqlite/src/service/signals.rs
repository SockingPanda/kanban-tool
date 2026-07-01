use crate::connect_file;

use super::{
    CommentRecord, SignalLifecycle, SignalListOptions, SignalObservationRecord, SignalRecord,
    SignalRecordInput, SignalRecordResult, SignalReviewInput, SignalStatus, all, board_id,
    ensure_changed_one, exec, insert_event, resolve_task, with_immediate_tx,
};

use std::{collections::HashSet, path::Path, str::FromStr};

use kanban_core::{Clock, KanbanError, Result, SystemClock, new_typed_id};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::json;

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
    let mut filters = vec!["s.board_id=?".to_owned()];
    let mut values: Vec<String> = vec![board_id];
    if !options.include_all && options.statuses.is_empty() {
        filters.push("s.status IN ('open','confirmed')".to_owned());
    }
    if !options.statuses.is_empty() {
        filters.push(format!(
            "s.status IN ({})",
            vec!["?"; options.statuses.len()].join(",")
        ));
        values.extend(options.statuses.iter().map(ToString::to_string));
    }
    if !options.kinds.is_empty() {
        filters.push(format!(
            "s.kind IN ({})",
            vec!["?"; options.kinds.len()].join(",")
        ));
        values.extend(options.kinds.iter().cloned());
    }
    if let Some(task_ref) = options.task_ref.as_deref() {
        let task = resolve_task(&conn, values.first().expect("board id"), task_ref)?;
        filters.push("o.task_id=?".to_owned());
        values.push(task.id);
    }
    let limit = options.limit.clamp(1, 1000).to_string();
    let sql = format!(
        "SELECT s.id,s.board_id,s.observation_id,s.kind,s.title,s.summary,s.severity,s.status,s.dedupe_key,s.superseded_by_signal_id,s.reviewed_by,s.reviewed_at,s.review_reason,s.created_at,s.updated_at,o.id,o.task_id,o.task_ref_snapshot,o.run_id,o.comment_id,o.actor,o.agent_type,o.source,o.evidence_json,o.created_at FROM signals s JOIN signal_observations o ON o.id=s.observation_id WHERE {} ORDER BY s.created_at DESC, s.id ASC LIMIT {limit}",
        filters.join(" AND ")
    );
    let refs = values.iter().map(String::as_str).collect::<Vec<_>>();
    all(
        &conn,
        &sql,
        rusqlite::params_from_iter(refs),
        signal_from_row,
    )
}

pub fn get_signal(path: impl AsRef<Path>, signal_id: &str) -> Result<SignalRecord> {
    let conn = connect_file(path.as_ref())?;
    get_signal_in_tx(&conn, signal_id)
}

pub fn review_signals(
    path: impl AsRef<Path>,
    board: &str,
    mut options: SignalListOptions,
) -> Result<Vec<SignalRecord>> {
    options.include_all = false;
    if options.statuses.is_empty() {
        options.statuses = vec![SignalStatus::Open, SignalStatus::Confirmed];
    }
    list_signals(path, board, options)
}

pub fn update_signal_status(
    path: impl AsRef<Path>,
    actor: &str,
    input: SignalReviewInput,
) -> Result<Vec<SignalRecord>> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
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
            let current = get_signal_in_tx(&conn, signal_id)?;
            let target = target_status(input.lifecycle);
            validate_signal_transition(&current, target)?;
            let replacement = if let Some(replacement) = input.replacement_signal_id.as_deref() {
                let replacement_signal = get_signal_in_tx(&conn, replacement)?;
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
                    "UPDATE signals SET status=?1, superseded_by_signal_id=?2, reviewed_by=?3, reviewed_at=?4, review_reason=?5, updated_at=?4 WHERE id=?6",
                    params![target.to_string(), replacement, actor, now, reason, signal_id],
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
            out.push(get_signal_in_tx(&conn, signal_id)?);
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
        signal: get_signal_in_tx(conn, signal_id)?,
        backlink_comment,
    })
}

fn get_signal_in_tx(conn: &Connection, signal_id: &str) -> Result<SignalRecord> {
    conn.query_row(
        "SELECT s.id,s.board_id,s.observation_id,s.kind,s.title,s.summary,s.severity,s.status,s.dedupe_key,s.superseded_by_signal_id,s.reviewed_by,s.reviewed_at,s.review_reason,s.created_at,s.updated_at,o.id,o.task_id,o.task_ref_snapshot,o.run_id,o.comment_id,o.actor,o.agent_type,o.source,o.evidence_json,o.created_at FROM signals s JOIN signal_observations o ON o.id=s.observation_id WHERE s.id=?1",
        [signal_id],
        signal_from_row,
    )
    .optional()
    .map_err(|err| KanbanError::Storage(err.to_string()))?
    .ok_or_else(|| KanbanError::NotFound(format!("signal not found: {signal_id}")))
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
            task_id: row.get(16)?,
            task_ref_snapshot: row.get(17)?,
            run_id: row.get(18)?,
            comment_id: row.get(19)?,
            actor: row.get(20)?,
            agent_type: row.get(21)?,
            source: row.get(22)?,
            evidence_json: row.get(23)?,
            created_at: row.get(24)?,
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
