use crate::connect_file;

use super::{
    ClaimResult, TaskRecord, board_id, ensure_board_active, get_task_by_id, insert_event,
    json_valid, query_tasks, resolve_task, storage, with_immediate_tx,
};

use std::path::Path;

use kanban_core::{
    Clock, KanbanError, ReadinessFacts, Result, SystemClock, TaskStatus, can_complete_from,
    can_finish_to, can_promote_from, completed_at_for_finish, is_claimable_task, new_run_id,
    new_typed_id, recompute_ready_status as core_recompute_ready_status, retry_decision,
    running_claim_is_present,
};

use rusqlite::{Connection, OptionalExtension, params};

use serde_json::json;

pub fn promote_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        let task = resolve_task(&conn, &board_id, task_ref)?;
        if !can_promote_from(task.status) {
            return Err(KanbanError::InvalidTransition(format!(
                "cannot promote from {}",
                task.status.as_str()
            )));
        }
        if task.status == TaskStatus::Scheduled && task.scheduled_at.is_some_and(|t| t > now) {
            return Err(KanbanError::InvalidTransition(
                "scheduled_at is in the future".into(),
            ));
        }
        let target = recompute_ready_status(&conn, &task, now)?;
        if target != TaskStatus::Ready {
            return Err(KanbanError::InvalidTransition(match target {
                TaskStatus::Todo => "dependency blocked".into(),
                TaskStatus::Scheduled => "scheduled_at is in the future".into(),
                TaskStatus::Triage => "task spec is incomplete".into(),
                _ => format!("cannot promote to {}", target.as_str()),
            }));
        }
        guarded_set_status(
            &conn,
            &board_id,
            &task,
            TaskStatus::Ready,
            actor,
            "task.promoted",
            now,
        )?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn claim_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    ttl_ms: i64,
) -> Result<ClaimResult> {
    ensure_positive_ttl(ttl_ms)?;
    claim_task_with_profile(path, board, actor, task_ref, ttl_ms, "manual")
}

pub fn claim_task_with_profile(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    ttl_ms: i64,
    worker_profile: &str,
) -> Result<ClaimResult> {
    ensure_positive_ttl(ttl_ms)?;
    claim_task_with_profile_and_metadata(path, board, actor, task_ref, ttl_ms, worker_profile, "{}")
}

pub fn claim_task_with_profile_and_metadata(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    ttl_ms: i64,
    worker_profile: &str,
    metadata_json: &str,
) -> Result<ClaimResult> {
    ensure_positive_ttl(ttl_ms)?;
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    if !json_valid(&conn, metadata_json)? {
        return Err(KanbanError::InvalidInput(
            "metadata_json must be valid JSON".into(),
        ));
    }
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    claim_task_conn(
        &conn,
        &board_id,
        actor,
        &task.id,
        ttl_ms,
        worker_profile,
        metadata_json,
        now,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn claim_task_conn(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    task_id: &str,
    ttl_ms: i64,
    profile: &str,
    metadata_json: &str,
    now: i64,
) -> Result<ClaimResult> {
    ensure_positive_ttl(ttl_ms)?;
    conn.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
    match claim_task_in_current_tx(
        conn,
        board_id,
        actor,
        task_id,
        ttl_ms,
        profile,
        metadata_json,
        now,
    ) {
        Ok(claim) => {
            conn.execute_batch("COMMIT").map_err(storage)?;
            Ok(claim)
        }
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

pub(crate) fn claim_next_ready_conn(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    worker_profile: &str,
    ttl_ms: i64,
    now: i64,
) -> Result<Option<ClaimResult>> {
    ensure_positive_ttl(ttl_ms)?;
    conn.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
    if let Err(err) = ensure_board_active(conn, board_id) {
        let _ = conn.execute_batch("ROLLBACK");
        return Err(err);
    }
    let selected = conn
        .query_row(
            "SELECT id FROM tasks WHERE board_id=?1 AND status='ready' AND claim_token IS NULL AND (assignee IS NULL OR assignee=?2) AND NOT EXISTS (SELECT 1 FROM task_dependencies d JOIN tasks p ON p.id=d.parent_task_id WHERE d.child_task_id=tasks.id AND p.status != 'done') ORDER BY priority ASC, created_at ASC LIMIT 1",
            params![board_id, worker_profile],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage);
    let result = match selected {
        Ok(Some(task_id)) => claim_task_in_current_tx(
            conn,
            board_id,
            actor,
            &task_id,
            ttl_ms,
            worker_profile,
            "{}",
            now,
        )
        .map(Some),
        Ok(None) => Ok(None),
        Err(err) => Err(err),
    };
    match result {
        Ok(claim) => {
            conn.execute_batch("COMMIT").map_err(storage)?;
            Ok(claim)
        }
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn claim_task_in_current_tx(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    task_id: &str,
    ttl_ms: i64,
    profile: &str,
    metadata_json: &str,
    now: i64,
) -> Result<ClaimResult> {
    ensure_positive_ttl(ttl_ms)?;
    ensure_board_active(conn, board_id)?;
    let task = get_task_by_id(conn, board_id, task_id)?;
    if !is_claimable_task(task.status, task.claim_token.is_some()) {
        return Err(KanbanError::InvalidTransition(
            "task is not claimable".into(),
        ));
    }
    ensure_dependencies_done(conn, task_id)?;
    let token = new_typed_id("claim");
    let run_id = new_run_id();
    let expires = now + ttl_ms;
    let changed = conn.execute(
        "UPDATE tasks SET status='running', claim_token=?1, claim_owner=?2, claim_expires_at=?3, last_heartbeat_at=?4, started_at=COALESCE(started_at, ?4), updated_at=?4, lock_version=lock_version+1 WHERE id=?5 AND board_id=?6 AND status='ready' AND claim_token IS NULL AND NOT EXISTS (SELECT 1 FROM task_dependencies d JOIN tasks p ON p.id=d.parent_task_id WHERE d.child_task_id=tasks.id AND p.status != 'done')",
        params![token, actor, expires, now, task_id, board_id],
    ).map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition("claim conflict".into()));
    }
    conn.execute(
        "INSERT INTO task_runs(id, board_id, task_id, status, worker_profile, claim_token, claim_owner, claim_expires_at, started_at, last_heartbeat_at, metadata_json) VALUES (?1, ?2, ?3, 'running', ?4, ?5, ?6, ?7, ?8, ?8, ?9)",
        params![run_id, board_id, task_id, profile, token, actor, expires, now, metadata_json],
    ).map_err(storage)?;
    conn.execute(
        "UPDATE tasks SET current_run_id=?1 WHERE id=?2",
        params![run_id, task_id],
    )
    .map_err(storage)?;
    insert_event(
        conn,
        board_id,
        Some(task_id),
        Some(&run_id),
        "task.claimed",
        actor,
        &json!({
            "claim_owner": actor,
            "metadata": serde_json::from_str::<serde_json::Value>(metadata_json)
                .unwrap_or_else(|_| json!({})),
        })
        .to_string(),
        now,
    )?;
    Ok(ClaimResult {
        task: get_task_by_id(conn, board_id, task_id)?,
        claim_token: token,
        run_id,
    })
}

pub fn heartbeat_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: &str,
    ttl_ms: i64,
) -> Result<TaskRecord> {
    ensure_positive_ttl(ttl_ms)?;
    heartbeat_task_with_note(path, board, actor, task_ref, token, ttl_ms, None)
}

pub fn heartbeat_task_with_note(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: &str,
    ttl_ms: i64,
    note: Option<&str>,
) -> Result<TaskRecord> {
    ensure_positive_ttl(ttl_ms)?;
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        heartbeat_task_conn(&conn, &board_id, actor, &task, token, ttl_ms, note, now)?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn heartbeat_task_conn(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    task: &TaskRecord,
    token: &str,
    ttl_ms: i64,
    note: Option<&str>,
    now: i64,
) -> Result<()> {
    ensure_positive_ttl(ttl_ms)?;
    if task.status != TaskStatus::Running || task.claim_token.as_deref() != Some(token) {
        return Err(KanbanError::InvalidTransition(
            "heartbeat requires matching running claim".into(),
        ));
    }
    let expires = now + ttl_ms;
    let changed = conn
        .execute(
            "UPDATE tasks SET claim_expires_at=?1, last_heartbeat_at=?2, updated_at=?2, lock_version=lock_version+1 WHERE id=?3 AND board_id=?4 AND status='running' AND claim_token=?5 AND current_run_id IS ?6",
            params![expires, now, task.id, board_id, token, task.current_run_id],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition(
            "heartbeat requires matching running claim".into(),
        ));
    }
    if let Some(run_id) = &task.current_run_id {
        let changed = conn
            .execute(
                "UPDATE task_runs SET claim_expires_at=?1, last_heartbeat_at=?2 WHERE id=?3 AND board_id=?4 AND task_id=?5 AND status='running' AND claim_token=?6",
                params![expires, now, run_id, board_id, task.id, token],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(KanbanError::InvalidTransition(
                "heartbeat requires matching running run".into(),
            ));
        }
        insert_event(
            conn,
            board_id,
            Some(&task.id),
            Some(run_id),
            "task.heartbeat",
            actor,
            &json!({ "note": note }).to_string(),
            now,
        )?;
    }
    Ok(())
}

fn ensure_positive_ttl(ttl_ms: i64) -> Result<()> {
    if ttl_ms <= 0 {
        return Err(KanbanError::InvalidInput("ttl_ms must be positive".into()));
    }
    Ok(())
}

pub fn complete_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: Option<&str>,
    force: bool,
) -> Result<TaskRecord> {
    complete_task_with_summary_and_result(path, board, actor, task_ref, token, force, None, None)
}

pub fn complete_task_with_summary(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: Option<&str>,
    force: bool,
    summary: Option<&str>,
) -> Result<TaskRecord> {
    complete_task_with_summary_and_result(path, board, actor, task_ref, token, force, summary, None)
}

#[allow(clippy::too_many_arguments)]
pub fn complete_task_with_summary_and_result(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: Option<&str>,
    force: bool,
    summary: Option<&str>,
    result_json: Option<&str>,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    let result_json_is_invalid = match result_json {
        Some(value) => !json_valid(&conn, value)?,
        None => false,
    };
    if result_json_is_invalid {
        return Err(KanbanError::InvalidInput(
            "result_json must be valid JSON".into(),
        ));
    }
    if task.status == TaskStatus::Running && !force && task.claim_token.as_deref() != token {
        return Err(KanbanError::InvalidTransition(
            "claim token mismatch".into(),
        ));
    }
    if !can_complete_from(task.status) {
        return Err(KanbanError::InvalidTransition(
            "complete requires running or review".into(),
        ));
    }
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        finish_running(
            &conn,
            &board_id,
            &task,
            TaskStatus::Done,
            actor,
            "task.completed",
            "succeeded",
            0,
            None,
            None,
            summary,
            result_json,
            now,
        )?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn submit_review_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: Option<&str>,
    force: bool,
) -> Result<TaskRecord> {
    submit_review_task_with_summary(path, board, actor, task_ref, token, force, None)
}

pub fn submit_review_task_with_summary(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    token: Option<&str>,
    force: bool,
    summary: Option<&str>,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    if task.status != TaskStatus::Running {
        return Err(KanbanError::InvalidTransition(
            "review requires running".into(),
        ));
    }
    if !force && task.claim_token.as_deref() != token {
        return Err(KanbanError::InvalidTransition(
            "claim token mismatch".into(),
        ));
    }
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        finish_running(
            &conn,
            &board_id,
            &task,
            TaskStatus::Review,
            actor,
            "task.submitted_for_review",
            "succeeded",
            0,
            None,
            None,
            summary,
            None,
            now,
        )?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn block_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    reason: &str,
    token: Option<&str>,
    force: bool,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    if reason.trim().is_empty() {
        return Err(KanbanError::InvalidInput("block reason is required".into()));
    }
    if task.status == TaskStatus::Running && !force && task.claim_token.as_deref() != token {
        return Err(KanbanError::InvalidTransition(
            "claim token mismatch".into(),
        ));
    }
    if !matches!(
        task.status,
        TaskStatus::Triage
            | TaskStatus::Todo
            | TaskStatus::Scheduled
            | TaskStatus::Ready
            | TaskStatus::Running
            | TaskStatus::Review
    ) {
        return Err(KanbanError::InvalidTransition("cannot block task".into()));
    }
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        if task.status == TaskStatus::Running {
            finish_running(
                &conn,
                &board_id,
                &task,
                TaskStatus::Blocked,
                actor,
                "task.blocked",
                "failed",
                1,
                Some(reason),
                None,
                None,
                None,
                now,
            )?;
        } else {
            let changed = conn
                .execute(
                    "UPDATE tasks SET status='blocked', status_reason=?1, updated_at=?2, lock_version=lock_version+1 WHERE id=?3 AND board_id=?4 AND status=?5",
                    params![reason, now, task.id, board_id, task.status.as_str()],
                )
                .map_err(storage)?;
            if changed != 1 {
                return Err(KanbanError::InvalidTransition("cannot block task".into()));
            }
            let payload = json!({ "reason": reason }).to_string();
            insert_event(
                &conn,
                &board_id,
                Some(&task.id),
                None,
                "task.blocked",
                actor,
                &payload,
                now,
            )?;
        }
        Ok(())
    })?;
    get_task_by_id(&conn, &board_id, &task.id)
}

pub fn unblock_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        let task = resolve_task(&conn, &board_id, task_ref)?;
        if task.status != TaskStatus::Blocked {
            return Err(KanbanError::InvalidTransition(
                "unblock requires blocked".into(),
            ));
        }
        let target = recompute_ready_status(&conn, &task, now)?;
        guarded_set_status_with_reason(
            &conn,
            &board_id,
            &task,
            StatusUpdate {
                status: target,
                status_reason: None,
                actor,
                event: "task.unblocked",
                now,
            },
        )?;
        get_task_by_id(&conn, &board_id, &task.id)
    })
}

pub fn reclaim_expired(path: impl AsRef<Path>, board: &str, actor: &str) -> Result<usize> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let expired: Vec<TaskRecord> = query_tasks(&conn, &board_id)?
        .into_iter()
        .filter(|t| t.status == TaskStatus::Running && t.claim_expires_at.is_some_and(|x| x <= now))
        .collect();
    let mut count = 0;
    for task in expired {
        let reclaimed = with_immediate_tx(&conn, || {
            ensure_board_active(&conn, &board_id)?;
            let fresh = get_task_by_id(&conn, &board_id, &task.id)?;
            let tx_now = SystemClock.now_ms();
            if fresh.status != TaskStatus::Running
                || fresh
                    .claim_expires_at
                    .is_none_or(|expires| expires > tx_now)
            {
                return Ok(false);
            }
            retry_running_task(
                &conn,
                &board_id,
                &fresh,
                actor,
                "expired",
                None,
                "claim expired",
                tx_now,
                Some(tx_now),
            )?;
            Ok(true)
        })?;
        if reclaimed {
            count += 1;
        }
    }
    Ok(count)
}

pub fn reclaim_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    force: bool,
) -> Result<TaskRecord> {
    reclaim_task_to(path, board, actor, task_ref, force, TaskStatus::Ready, None)
}

pub fn reclaim_task_to(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    force: bool,
    to_status: TaskStatus,
    reason: Option<&str>,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    if !matches!(to_status, TaskStatus::Ready | TaskStatus::Blocked) {
        return Err(KanbanError::InvalidInput(
            "reclaim to_status must be ready or blocked".into(),
        ));
    }
    if to_status == TaskStatus::Blocked && reason.is_none_or(|value| value.trim().is_empty()) {
        return Err(KanbanError::InvalidInput(
            "reclaim reason is required when to_status is blocked".into(),
        ));
    }
    let task = resolve_task(&conn, &board_id, task_ref)?;
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        let fresh = get_task_by_id(&conn, &board_id, &task.id)?;
        let tx_now = SystemClock.now_ms();
        if fresh.status != TaskStatus::Running {
            return Err(KanbanError::InvalidTransition(
                "reclaim requires running".into(),
            ));
        }
        if !force
            && fresh
                .claim_expires_at
                .is_none_or(|expires| expires > tx_now)
        {
            return Err(KanbanError::InvalidTransition(
                "reclaim requires expired claim or force".into(),
            ));
        }
        let decision = retry_decision(fresh.retry_count, fresh.max_retries, to_status);
        let default_reason = if decision.max_retries_reached {
            "max retries reached"
        } else if force {
            "force reclaimed"
        } else {
            "claim expired"
        };
        let effective_reason = reason.unwrap_or(default_reason);
        reclaim_running_task(
            &conn,
            &board_id,
            &fresh,
            actor,
            if force { "canceled" } else { "expired" },
            effective_reason,
            decision.status,
            tx_now,
            (!force).then_some(tx_now),
        )?;
        get_task_by_id(&conn, &board_id, &fresh.id)
    })
}

pub fn archive_task(
    path: impl AsRef<Path>,
    board: &str,
    actor: &str,
    task_ref: &str,
    force: bool,
) -> Result<TaskRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let board_id = board_id(&conn, board)?;
    let task = resolve_task(&conn, &board_id, task_ref)?;
    if task.status == TaskStatus::Running && !force {
        return Err(KanbanError::InvalidTransition(
            "cannot archive running without force".into(),
        ));
    }
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        if task.status == TaskStatus::Running {
            let run_id = task.current_run_id.as_deref().ok_or_else(|| {
                KanbanError::InvalidTransition("force archive requires active run".into())
            })?;
            let changed = conn
            .execute(
                "UPDATE task_runs SET status='canceled', finished_at=?1, error=COALESCE(error, ?2) WHERE id=?3 AND board_id=?4 AND task_id=?5 AND status='running'",
                params![now, "force archived", run_id, board_id, task.id],
            )
            .map_err(storage)?;
            if changed != 1 {
                return Err(KanbanError::InvalidTransition(
                    "force archive requires active running run".into(),
                ));
            }
        }
        let changed = conn
            .execute(
                "UPDATE tasks SET status='archived', archived_at=?1, claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, updated_at=?1, lock_version=lock_version+1 WHERE id=?2 AND board_id=?3 AND status=?4",
                params![now, task.id, board_id, task.status.as_str()],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(KanbanError::InvalidTransition("cannot archive task".into()));
        }
        insert_event(
            &conn,
            &board_id,
            Some(&task.id),
            task.current_run_id.as_deref(),
            "task.archived",
            actor,
            "{}",
            now,
        )?;
        Ok(())
    })?;
    get_task_by_id(&conn, &board_id, &task.id)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn reclaim_running_task(
    conn: &Connection,
    board_id: &str,
    task: &TaskRecord,
    actor: &str,
    run_status: &str,
    reason: &str,
    target: TaskStatus,
    now: i64,
    expiry_guard: Option<i64>,
) -> Result<()> {
    if !running_claim_is_present(
        task.status,
        task.claim_token.is_some(),
        task.current_run_id.is_some(),
    ) {
        return Err(KanbanError::InvalidTransition(
            "reclaim requires matching running claim".into(),
        ));
    }
    if let (Some(run_id), Some(token)) = (&task.current_run_id, &task.claim_token) {
        let changed = conn
            .execute(
                "UPDATE task_runs SET status=?1, finished_at=?2, error=?3 WHERE id=?4 AND board_id=?5 AND task_id=?6 AND status='running' AND claim_token=?7",
                params![run_status, now, reason, run_id, board_id, task.id, token],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(KanbanError::InvalidTransition(
                "reclaim requires matching running run".into(),
            ));
        }
    }
    let changed = conn
        .execute(
            "UPDATE tasks SET status=?1, status_reason=?2, claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, retry_count=retry_count+1, updated_at=?3, lock_version=lock_version+1 WHERE id=?4 AND board_id=?5 AND status='running' AND claim_token=?6 AND current_run_id=?7 AND (?8 IS NULL OR claim_expires_at <= ?8)",
            params![target.as_str(), (target == TaskStatus::Blocked).then_some(reason), now, task.id, board_id, task.claim_token, task.current_run_id, expiry_guard],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition(
            "reclaim requires matching running claim".into(),
        ));
    }
    let payload = json!({
        "retry_count": task.retry_count + 1,
        "max_retries": task.max_retries,
        "to_status": target.as_str(),
        "reason": reason,
    })
    .to_string();
    insert_event(
        conn,
        board_id,
        Some(&task.id),
        task.current_run_id.as_deref(),
        "task.reclaimed",
        actor,
        &payload,
        now,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn retry_running_task(
    conn: &Connection,
    board_id: &str,
    task: &TaskRecord,
    actor: &str,
    run_status: &str,
    exit_code: Option<i32>,
    reason: &str,
    now: i64,
    expiry_guard: Option<i64>,
) -> Result<()> {
    if !running_claim_is_present(
        task.status,
        task.claim_token.is_some(),
        task.current_run_id.is_some(),
    ) {
        return Err(KanbanError::InvalidTransition(
            "retry requires matching running claim".into(),
        ));
    }
    let decision = retry_decision(task.retry_count, task.max_retries, TaskStatus::Ready);
    if let (Some(run_id), Some(token)) = (&task.current_run_id, &task.claim_token) {
        let changed = conn
            .execute(
                "UPDATE task_runs SET status=?1, finished_at=?2, exit_code=?3, error=?4 WHERE id=?5 AND board_id=?6 AND task_id=?7 AND status='running' AND claim_token=?8",
                params![run_status, now, exit_code, reason, run_id, board_id, task.id, token],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(KanbanError::InvalidTransition(
                "retry requires matching running run".into(),
            ));
        }
    }
    let changed = conn
        .execute(
            "UPDATE tasks SET status=?1, status_reason=?2, claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, retry_count=?3, updated_at=?4, lock_version=lock_version+1 WHERE id=?5 AND board_id=?6 AND status='running' AND claim_token=?7 AND current_run_id=?8 AND (?9 IS NULL OR claim_expires_at <= ?9)",
            params![decision.status.as_str(), if decision.max_retries_reached { Some(reason) } else { None }, decision.retry_count, now, task.id, board_id, task.claim_token, task.current_run_id, expiry_guard],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition(
            "retry requires matching running claim".into(),
        ));
    }
    let payload = json!({
        "retry_count": decision.retry_count,
        "max_retries": task.max_retries,
    })
    .to_string();
    insert_event(
        conn,
        board_id,
        Some(&task.id),
        task.current_run_id.as_deref(),
        if decision.max_retries_reached {
            "task.blocked"
        } else {
            "task.reclaimed"
        },
        actor,
        &payload,
        now,
    )?;
    if decision.max_retries_reached && reason == "claim expired" {
        insert_event(
            conn,
            board_id,
            Some(&task.id),
            task.current_run_id.as_deref(),
            "task.reclaimed",
            actor,
            &payload,
            now,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn finish_running(
    conn: &Connection,
    board_id: &str,
    task: &TaskRecord,
    target: TaskStatus,
    actor: &str,
    event: &str,
    run_status: &str,
    exit_code: i32,
    reason: Option<&str>,
    log_path: Option<&Path>,
    summary: Option<&str>,
    result_json: Option<&str>,
    now: i64,
) -> Result<()> {
    let completed = completed_at_for_finish(target, now, task.completed_at);
    if !can_finish_to(task.status, target) {
        return Err(KanbanError::InvalidTransition(
            "finish requires matching running claim".into(),
        ));
    }
    let changed = if task.status == TaskStatus::Running {
        if !running_claim_is_present(
            task.status,
            task.claim_token.is_some(),
            task.current_run_id.is_some(),
        ) {
            return Err(KanbanError::InvalidTransition(
                "finish requires matching running claim".into(),
            ));
        }
        conn.execute(
            "UPDATE tasks SET status=?1, status_reason=?2, completed_at=?3, result_summary=COALESCE(?4, result_summary), result_json=COALESCE(?5, result_json), claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, updated_at=?6, lock_version=lock_version+1 WHERE id=?7 AND board_id=?8 AND status='running' AND claim_token=?9 AND current_run_id=?10",
            params![target.as_str(), reason, completed, summary, result_json, now, task.id, board_id, task.claim_token, task.current_run_id],
        )
    } else {
        conn.execute(
            "UPDATE tasks SET status=?1, status_reason=?2, completed_at=?3, result_summary=COALESCE(?4, result_summary), result_json=COALESCE(?5, result_json), claim_token=NULL, claim_owner=NULL, claim_expires_at=NULL, last_heartbeat_at=NULL, updated_at=?6, lock_version=lock_version+1 WHERE id=?7 AND board_id=?8 AND status='review'",
            params![target.as_str(), reason, completed, summary, result_json, now, task.id, board_id],
        )
    }
    .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition(
            "finish requires matching running claim".into(),
        ));
    }
    let event_payload = json!({
        "result": result_json.and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok()),
    })
    .to_string();
    if let Some(run_id) = &task.current_run_id {
        let changed = conn.execute(
            "UPDATE task_runs SET status=?1, finished_at=?2, exit_code=?3, error=?4, log_path=COALESCE(?5, log_path), summary=COALESCE(?6, summary) WHERE id=?7 AND board_id=?8 AND task_id=?9 AND status='running' AND claim_token IS ?10",
            params![run_status, now, exit_code, reason, log_path.map(|p| p.to_string_lossy().to_string()), summary, run_id, board_id, task.id, task.claim_token],
        ).map_err(storage)?;
        if task.status == TaskStatus::Running && changed != 1 {
            return Err(KanbanError::InvalidTransition(
                "finish requires matching running run".into(),
            ));
        }
        insert_event(
            conn,
            board_id,
            Some(&task.id),
            Some(run_id),
            event,
            actor,
            &event_payload,
            now,
        )?;
    } else {
        insert_event(
            conn,
            board_id,
            Some(&task.id),
            None,
            event,
            actor,
            &event_payload,
            now,
        )?;
    }
    Ok(())
}

pub(crate) fn guarded_set_status(
    conn: &Connection,
    board_id: &str,
    task: &TaskRecord,
    status: TaskStatus,
    actor: &str,
    event: &str,
    now: i64,
) -> Result<()> {
    guarded_set_status_with_reason(
        conn,
        board_id,
        task,
        StatusUpdate {
            status,
            status_reason: None,
            actor,
            event,
            now,
        },
    )
}

pub(crate) struct StatusUpdate<'a> {
    status: TaskStatus,
    status_reason: Option<&'a str>,
    actor: &'a str,
    event: &'a str,
    now: i64,
}

pub(crate) fn guarded_set_status_with_reason(
    conn: &Connection,
    board_id: &str,
    task: &TaskRecord,
    update: StatusUpdate<'_>,
) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE tasks SET status=?1, status_reason=?2, updated_at=?3, lock_version=lock_version+1 WHERE id=?4 AND board_id=?5 AND status=?6 AND lock_version=?7",
            params![
                update.status.as_str(),
                update.status_reason,
                update.now,
                task.id,
                board_id,
                task.status.as_str(),
                task.lock_version
            ],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(KanbanError::InvalidTransition(
            "status update requires matching fresh task".into(),
        ));
    }
    let payload = json!({ "to_status": update.status.as_str() }).to_string();
    insert_event(
        conn,
        board_id,
        Some(&task.id),
        None,
        update.event,
        update.actor,
        &payload,
        update.now,
    )
}

pub(crate) fn recompute_ready_status(
    conn: &Connection,
    task: &TaskRecord,
    now: i64,
) -> Result<TaskStatus> {
    Ok(core_recompute_ready_status(
        ReadinessFacts {
            title: &task.title,
            description: task.description.as_deref(),
            scheduled_at: task.scheduled_at,
            dependencies_done: dependencies_done(conn, &task.id)?,
        },
        now,
    ))
}

pub(crate) fn ensure_dependencies_done(conn: &Connection, task_id: &str) -> Result<()> {
    if dependencies_done(conn, task_id)? {
        Ok(())
    } else {
        Err(KanbanError::InvalidTransition("dependency blocked".into()))
    }
}

pub(crate) fn dependencies_done(conn: &Connection, task_id: &str) -> Result<bool> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM task_dependencies d JOIN tasks p ON p.id=d.parent_task_id WHERE d.child_task_id=?1 AND p.status != 'done'", [task_id], |r| r.get(0)).map_err(storage)?;
    Ok(count == 0)
}
