use crate::common::*;

#[test]
fn doctor_resolves_legacy_relative_run_log_paths_against_database_dir() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_resolves_legacy_relative_run_log_paths_against_database_dir")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("legacy log path"),
    )?;
    let log_dir = temp.dir.join("logs");
    dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "printf legacy".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir,
        },
    )?;
    let run = list_runs(&temp.path, "default", Some(&task.id))?[0].clone();
    let absolute_log_path = Path::new(
        run.log_path
            .as_ref()
            .ok_or_else(|| test_error("expected run log path"))?,
    );
    let relative_log_path = absolute_log_path
        .strip_prefix(&temp.dir)?
        .to_string_lossy()
        .to_string();
    connect_file(&temp.path)?.execute(
        "UPDATE task_runs SET log_path=?1 WHERE id=?2",
        params![relative_log_path, run.id],
    )?;

    let report = doctor_database(&temp.path)?;

    assert_eq!(report.missing_run_logs, 0);
    assert!(report.ok);
    Ok(())
}

#[test]
fn doctor_counts_suspicious_run_log_paths_separately_from_missing_allowed_logs()
-> anyhow::Result<()> {
    let temp = TempDb::new("doctor_counts_suspicious_run_log_paths_separately")?;
    init_database(&temp.path, "tester")?;
    let suspicious_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("suspicious log path"),
    )?;
    let missing_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("missing log path"),
    )?;
    let log_dir = temp.dir.join("logs");
    let suspicious = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "printf suspicious".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: log_dir.clone(),
        },
    )?;
    let missing = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "printf missing".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir,
        },
    )?;
    assert_eq!(
        suspicious.task_id.as_deref(),
        Some(suspicious_task.id.as_str())
    );
    assert_eq!(missing.task_id.as_deref(), Some(missing_task.id.as_str()));
    let suspicious_run_id = suspicious
        .run_id
        .ok_or_else(|| test_error("expected suspicious run id"))?;
    let missing_run_id = missing
        .run_id
        .ok_or_else(|| test_error("expected missing run id"))?;
    let missing_log_path = get_run_by_id_global(&temp.path, &missing_run_id)?
        .log_path
        .ok_or_else(|| test_error("expected missing run log path"))?;
    std::fs::remove_file(missing_log_path)?;
    connect_file(&temp.path)?.execute(
        "UPDATE task_runs SET log_path=?1 WHERE id=?2",
        params!["/etc/passwd", suspicious_run_id],
    )?;

    let report = doctor_database(&temp.path)?;

    assert!(!report.ok);
    assert_eq!(report.missing_run_logs, 1);
    assert_eq!(report.suspicious_run_log_paths, 1);
    Ok(())
}

#[test]
fn doctor_reports_partially_initialized_database_without_bailing() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_reports_partially_initialized_database_without_bailing")?;
    connect_file(&temp.path)
        ?
        .execute(
            "CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL DEFAULT '', applied_at INTEGER NOT NULL)",
            [],
        )
        ?;

    let report = doctor_database(&temp.path)?;

    assert!(!report.ok);
    assert_eq!(report.migration_version, None);
    assert_eq!(report.user_version, 0);
    Ok(())
}

#[test]
fn doctor_reports_missing_knowledge_substrate_tables_unhealthy() -> anyhow::Result<()> {
    for table in ["index_outbox", "derived_store_state"] {
        let temp = TempDb::new(&format!(
            "doctor_reports_missing_knowledge_substrate_tables_unhealthy_{table}"
        ))?;
        init_database(&temp.path, "tester")?;
        connect_file(&temp.path)?.execute_batch(&format!("DROP TABLE {table};"))?;

        let report = doctor_database(&temp.path)?;

        assert_eq!(report.migration_version, Some(2));
        assert_eq!(report.user_version, 2);
        assert!(!report.ok, "{table} missing should make doctor unhealthy");
    }
    Ok(())
}

#[test]
fn doctor_reports_executable_status_invariant_violations() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_reports_executable_status_invariant_violations")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("unfinished parent"),
    )?;
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("invalid ready child"),
    )?;
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id)?;
    let missing_spec = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("invalid missing spec"),
    )?;
    let future_scheduled = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("invalid future schedule"),
    )?;
    let conn = connect_file(&temp.path)?;
    conn.execute("UPDATE tasks SET status='ready' WHERE id=?1", [&child.id])?;
    conn.execute(
        "UPDATE tasks SET description=NULL WHERE id=?1",
        [&missing_spec.id],
    )?;
    conn.execute(
        "UPDATE tasks SET scheduled_at=?1 WHERE id=?2",
        params![4_102_444_800_000_i64, future_scheduled.id],
    )?;

    let report = doctor_database(&temp.path)?;

    assert!(!report.ok);
    assert_eq!(report.executable_dependency_violations, 1);
    assert_eq!(report.executable_spec_violations, 1);
    assert_eq!(report.executable_schedule_violations, 1);
    Ok(())
}

#[test]
fn doctor_counts_each_dependency_cycle_once() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_counts_each_dependency_cycle_once")?;
    init_database(&temp.path, "tester")?;
    let a = create_task(&temp.path, "default", "tester", CreateTask::ready("a"))?;
    let b = create_task(&temp.path, "default", "tester", CreateTask::ready("b"))?;
    let c = create_task(&temp.path, "default", "tester", CreateTask::ready("c"))?;
    add_dependency(&temp.path, "default", "tester", &a.id, &b.id)?;
    add_dependency(&temp.path, "default", "tester", &b.id, &c.id)?;
    connect_file(&temp.path)?.execute(
        "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) \
             VALUES (?1, ?2, ?3, 1)",
        params![a.board_id, c.id, a.id],
    )?;

    let report = doctor_database(&temp.path)?;

    assert!(!report.ok);
    assert_eq!(report.dependency_cycles, 1);
    Ok(())
}

#[test]
fn database_replace_is_rejected_while_runtime_lock_is_held() -> anyhow::Result<()> {
    let temp = TempDb::new("database_replace_is_rejected_while_runtime_lock_is_held")?;
    init_database(&temp.path, "tester")?;
    let _runtime_guard = begin_database_runtime(&temp.path)?;

    let err = result_err(begin_database_replace(&temp.path))?;

    assert!(
        err.to_string().contains("running")
            || err.to_string().contains("runtime")
            || err.to_string().contains("serve/dispatch"),
        "err: {err}"
    );
    Ok(())
}
