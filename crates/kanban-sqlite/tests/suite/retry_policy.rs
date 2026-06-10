use crate::common::*;

#[test]
fn failed_ready_retry_policy_increments_retry_count_and_blocks_at_max_retries() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("failed_ready_retry_policy_increments_retry_count_and_blocks_at_max_retries")?;
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("retry worker"),
    )
    .unwrap();
    set_retry_policy(&temp.path, &task.id, 2);

    for expected_retry_count in [1, 2] {
        let result = dispatch_once(
            &temp.path,
            "default",
            DispatchOptions {
                actor: "dispatcher".into(),
                command: "exit 7".into(),
                worker_profile: "default".into(),
                claim_ttl_ms: 300_000,
                heartbeat_interval_ms: 30_000,
                on_success: FinishPolicy::Done,
                on_failure: FinishPolicy::Ready,
                log_dir: temp.dir.join("logs"),
            },
        )
        .unwrap();
        assert_eq!(result.claimed, 1);
        assert_eq!(result.exit_code, Some(7));
        let fresh = get_task(&temp.path, "default", &task.id).unwrap();
        assert_eq!(fresh.retry_count, expected_retry_count);
        if expected_retry_count == 1 {
            assert_eq!(fresh.status, TaskStatus::Ready);
        } else {
            assert_eq!(fresh.status, TaskStatus::Blocked);
        }
    }
    Ok(())
}

#[test]
fn reclaim_expired_increments_retry_count_and_blocks_at_max_retries() -> anyhow::Result<()> {
    let temp = TempDb::new("reclaim_expired_increments_retry_count_and_blocks_at_max_retries")?;
    init_database(&temp.path, "tester").unwrap();
    let retrying = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("retry reclaim"),
    )
    .unwrap();
    set_retry_policy(&temp.path, &retrying.id, 2);
    let blocking = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("blocking reclaim"),
    )
    .unwrap();
    set_retry_policy(&temp.path, &blocking.id, 1);

    for task in [&retrying, &blocking] {
        claim_task(&temp.path, "default", "worker", &task.id, 1).unwrap();
    }
    thread::sleep(Duration::from_millis(5));

    let reclaimed = kanban_sqlite::reclaim_expired(&temp.path, "default", "dispatcher").unwrap();

    assert_eq!(reclaimed, 2);
    let retrying = get_task(&temp.path, "default", &retrying.id).unwrap();
    assert_eq!(retrying.retry_count, 1);
    assert_eq!(retrying.status, TaskStatus::Ready);
    let blocking = get_task(&temp.path, "default", &blocking.id).unwrap();
    assert_eq!(blocking.retry_count, 1);
    assert_eq!(blocking.status, TaskStatus::Blocked);
    assert!(
        list_events(&temp.path, "default", Some(&blocking.id))
            .unwrap()
            .iter()
            .any(|event| event.kind == "task.reclaimed")
    );
    Ok(())
}

#[test]
fn reclaim_expired_skips_task_heartbeated_after_scan_before_claim_tx() -> anyhow::Result<()> {
    let temp = TempDb::new("reclaim_expired_skips_task_heartbeated_after_scan_before_claim_tx")?;
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("heartbeat race"),
    )
    .unwrap();
    let claim = claim_task(&temp.path, "default", "worker", &task.id, 1).unwrap();
    thread::sleep(Duration::from_millis(5));

    let db_path = temp.path.clone();
    let task_id = task.id.clone();
    let run_id = claim.run_id.clone();
    let claim_token = claim.claim_token.clone();
    let heartbeat_started = Arc::new(Barrier::new(2));
    let release_heartbeat = Arc::new(Barrier::new(2));
    let worker_started = Arc::clone(&heartbeat_started);
    let worker_release = Arc::clone(&release_heartbeat);
    let handle = thread::spawn(move || {
        let conn = connect_file(&db_path).unwrap();
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        let extended = now_ms() + 300_000;
        conn.execute(
            "UPDATE tasks SET claim_expires_at=?1, last_heartbeat_at=?2 WHERE id=?3 AND status='running' AND claim_token=?4",
            params![extended, extended, task_id, claim_token],
        )
        .unwrap();
        conn.execute(
            "UPDATE task_runs SET claim_expires_at=?1, last_heartbeat_at=?2 WHERE id=?3",
            params![extended, extended, run_id],
        )
        .unwrap();
        worker_started.wait();
        worker_release.wait();
        conn.execute_batch("COMMIT").unwrap();
    });

    heartbeat_started.wait();
    let reclaiming = thread::spawn({
        let db_path = temp.path.clone();
        move || kanban_sqlite::reclaim_expired(&db_path, "default", "dispatcher")
    });
    thread::sleep(Duration::from_millis(50));
    release_heartbeat.wait();

    let reclaimed = reclaiming.join().unwrap().unwrap();
    handle.join().unwrap();

    assert_eq!(reclaimed, 0);
    let fresh = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(fresh.status, TaskStatus::Running);
    assert!(
        fresh
            .claim_expires_at
            .is_some_and(|expires| expires > now_ms())
    );
    Ok(())
}
