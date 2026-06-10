use super::*;

pub fn dispatch_once(
    path: impl AsRef<Path>,
    board: &str,
    options: DispatchOptions,
) -> Result<DispatchResult> {
    validate_dispatch_options(&options)?;
    let path = path.as_ref();
    validate_dispatch_log_dir(path.parent(), &options.log_dir)?;
    reclaim_expired(path, board, &options.actor)?;
    let conn = connect_file(path)?;
    let board_id = board_id(&conn, board)?;
    let now = SystemClock.now_ms();
    promote_due_tasks(&conn, &board_id, &options.actor, now)?;
    let Some(claim) = claim_next_ready_conn(
        &conn,
        &board_id,
        &options.actor,
        &options.worker_profile,
        options.claim_ttl_ms,
        now,
    )?
    else {
        return Ok(DispatchResult {
            claimed: 0,
            task_id: None,
            run_id: None,
            exit_code: None,
        });
    };
    std::fs::create_dir_all(&options.log_dir).map_err(|e| KanbanError::Storage(e.to_string()))?;
    let log_path = options.log_dir.join(format!("{}.log", claim.run_id));
    let output = run_worker_with_heartbeat(path, board, &options, &claim, &log_path)?;
    let exit = output.status.code().unwrap_or(1);
    let fresh = get_task_by_id(&conn, &board_id, &claim.task.id)?;
    let target = if output.status.success() {
        options.on_success
    } else {
        options.on_failure
    };
    with_immediate_tx(&conn, || {
        ensure_board_active(&conn, &board_id)?;
        match target {
            FinishPolicy::Done => {
                finish_running(
                    &conn,
                    &board_id,
                    &fresh,
                    TaskStatus::Done,
                    &options.actor,
                    "task.completed",
                    "succeeded",
                    exit,
                    None,
                    Some(&log_path),
                    None,
                    None,
                    SystemClock.now_ms(),
                )?;
                promote_children(
                    &conn,
                    &board_id,
                    &options.actor,
                    &fresh.id,
                    SystemClock.now_ms(),
                )?;
            }
            FinishPolicy::Review => {
                finish_running(
                    &conn,
                    &board_id,
                    &fresh,
                    TaskStatus::Review,
                    &options.actor,
                    "task.submitted_for_review",
                    "succeeded",
                    exit,
                    None,
                    Some(&log_path),
                    None,
                    None,
                    SystemClock.now_ms(),
                )?;
            }
            FinishPolicy::Blocked => {
                finish_running(
                    &conn,
                    &board_id,
                    &fresh,
                    TaskStatus::Blocked,
                    &options.actor,
                    "task.blocked",
                    "failed",
                    exit,
                    Some("worker failed"),
                    Some(&log_path),
                    None,
                    None,
                    SystemClock.now_ms(),
                )?;
            }
            FinishPolicy::Ready => {
                retry_running_task(
                    &conn,
                    &board_id,
                    &fresh,
                    &options.actor,
                    "failed",
                    Some(exit),
                    "worker failed",
                    SystemClock.now_ms(),
                    None,
                )?;
                conn.execute(
                    "UPDATE task_runs SET log_path=?1 WHERE id=?2",
                    params![log_path.to_string_lossy(), claim.run_id],
                )
                .map_err(storage)?;
            }
        }
        Ok(())
    })?;
    Ok(DispatchResult {
        claimed: 1,
        task_id: Some(claim.task.id),
        run_id: Some(claim.run_id),
        exit_code: Some(exit),
    })
}

pub(crate) fn promote_due_tasks(
    conn: &Connection,
    board_id: &str,
    actor: &str,
    now: i64,
) -> Result<usize> {
    let candidates = query_tasks(conn, board_id)?
        .into_iter()
        .filter(|task| matches!(task.status, TaskStatus::Todo | TaskStatus::Scheduled))
        .collect::<Vec<_>>();
    let mut promoted = 0;
    for task in candidates {
        let was_promoted = with_immediate_tx(conn, || {
            ensure_board_active(conn, board_id)?;
            let fresh = get_task_by_id(conn, board_id, &task.id)?;
            if !matches!(fresh.status, TaskStatus::Todo | TaskStatus::Scheduled) {
                return Ok(false);
            }
            if recompute_ready_status(conn, &fresh, now)? != TaskStatus::Ready {
                return Ok(false);
            }
            guarded_set_status(
                conn,
                board_id,
                &fresh,
                TaskStatus::Ready,
                actor,
                "task.promoted",
                now,
            )?;
            Ok(true)
        })?;
        if was_promoted {
            promoted += 1;
        }
    }
    Ok(promoted)
}

pub(crate) struct WorkerOutput {
    status: ExitStatus,
}

pub(crate) fn validate_dispatch_options(options: &DispatchOptions) -> Result<()> {
    if options.claim_ttl_ms <= 0 {
        return Err(KanbanError::InvalidInput(
            "claim_ttl_ms must be positive".into(),
        ));
    }
    if options.heartbeat_interval_ms <= 0 {
        return Err(KanbanError::InvalidInput(
            "heartbeat_interval_ms must be positive".into(),
        ));
    }
    if options.heartbeat_interval_ms >= options.claim_ttl_ms {
        return Err(KanbanError::InvalidInput(
            "heartbeat_interval_ms must be less than claim_ttl_ms".into(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_dispatch_log_dir(db_dir: Option<&Path>, log_dir: &Path) -> Result<()> {
    let Some(db_dir) = db_dir else {
        return Err(KanbanError::InvalidInput(
            "database path has no parent directory".into(),
        ));
    };
    let normalized_log_dir = normalize_existing_aware(log_dir);
    let allowed = allowed_run_log_roots(db_dir)
        .iter()
        .map(|root| normalize_existing_aware(root))
        .any(|root| normalized_log_dir.starts_with(root));
    if !allowed {
        return Err(KanbanError::InvalidInput(
            "dispatch log_dir is outside allowed run log roots".into(),
        ));
    }
    Ok(())
}

pub(crate) fn run_worker_with_heartbeat(
    path: &Path,
    board: &str,
    options: &DispatchOptions,
    claim: &ClaimResult,
    log_path: &Path,
) -> Result<WorkerOutput> {
    let stdout = File::create(log_path).map_err(|e| KanbanError::Storage(e.to_string()))?;
    let stderr = stdout
        .try_clone()
        .map_err(|e| KanbanError::Storage(e.to_string()))?;
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&options.command)
        .env("KB_DB_PATH", path)
        .env("KB_BOARD_ID", &claim.task.board_id)
        .env("KB_BOARD_SLUG", board)
        .env("KB_TASK_ID", &claim.task.id)
        .env("KB_TASK_SEQ", claim.task.seq.to_string())
        .env("KB_TASK_TITLE", &claim.task.title)
        .env("KB_CLAIM_TOKEN", &claim.claim_token)
        .env("KB_RUN_ID", &claim.run_id)
        .env("KB_ACTOR", &options.actor)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|e| KanbanError::Storage(e.to_string()))?;

    let heartbeat_interval = Duration::from_millis(options.heartbeat_interval_ms as u64);
    let poll_interval = heartbeat_interval.min(Duration::from_millis(10));
    let mut elapsed_since_heartbeat = Duration::ZERO;
    loop {
        match child
            .try_wait()
            .map_err(|e| KanbanError::Storage(e.to_string()))?
        {
            Some(status) => return Ok(WorkerOutput { status }),
            None => {
                thread::sleep(poll_interval);
                elapsed_since_heartbeat += poll_interval;
                if elapsed_since_heartbeat < heartbeat_interval {
                    continue;
                }
                elapsed_since_heartbeat = Duration::ZERO;
                let conn = connect_file(path)?;
                let board_id = board_id(&conn, board)?;
                let task = get_task_by_id(&conn, &board_id, &claim.task.id)?;
                if let Err(err) = with_immediate_tx(&conn, || {
                    ensure_board_active(&conn, &board_id)?;
                    heartbeat_task_conn(
                        &conn,
                        &board_id,
                        &options.actor,
                        &task,
                        &claim.claim_token,
                        options.claim_ttl_ms,
                        None,
                        SystemClock.now_ms(),
                    )
                }) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(err);
                }
            }
        }
    }
}
