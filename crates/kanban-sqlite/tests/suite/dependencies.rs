use crate::common::*;

#[test]
fn claim_complete_and_dependencies_promote_children() -> anyhow::Result<()> {
    let temp = TempDb::new("claim_complete_and_dependencies_promote_children")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(&temp.path, "default", "tester", CreateTask::ready("父任务"))?;
    let child = create_task(&temp.path, "default", "tester", CreateTask::ready("子任务"))?;

    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id)?;
    assert_eq!(
        get_task(&temp.path, "default", &child.id)?.status,
        TaskStatus::Todo
    );

    let claim = claim_task(&temp.path, "default", "worker", &parent.id, 300_000)?;
    assert_eq!(claim.task.status, TaskStatus::Running);
    assert!(!claim.claim_token.is_empty());
    assert!(claim.task.current_run_id.is_some());
    let heartbeat = kanban_sqlite::heartbeat_task(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        &claim.claim_token,
        600_000,
    )?;
    assert!(heartbeat.claim_expires_at > claim.task.claim_expires_at);

    complete_task(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        Some(&claim.claim_token),
        false,
    )?;
    assert_eq!(
        get_task(&temp.path, "default", &parent.id)?.status,
        TaskStatus::Done
    );
    assert_eq!(
        get_task(&temp.path, "default", &child.id)?.status,
        TaskStatus::Ready
    );
    assert_eq!(
        list_runs(&temp.path, "default", Some(&parent.id))?[0].status,
        "succeeded"
    );
    Ok(())
}

#[test]
fn block_unblock_recomputes_target_and_cycle_detection_rejects_cycles() -> anyhow::Result<()> {
    let temp = TempDb::new("block_unblock_recomputes_target_and_cycle_detection_rejects_cycles")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(&temp.path, "default", "tester", CreateTask::ready("父任务"))?;
    let child = create_task(&temp.path, "default", "tester", CreateTask::ready("子任务"))?;
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id)?;

    let err = result_err(add_dependency(
        &temp.path, "default", "tester", &child.id, &parent.id,
    ))?;
    assert!(err.to_string().contains("cycle"));

    block_task(
        &temp.path,
        "default",
        "tester",
        &child.id,
        "等待输入",
        None,
        false,
    )?;
    let unblocked = unblock_task(&temp.path, "default", "tester", &child.id)?;
    assert_eq!(unblocked.status, TaskStatus::Todo);

    let claim = claim_task(&temp.path, "default", "worker", &parent.id, 300_000)?;
    complete_task(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        Some(&claim.claim_token),
        false,
    )?;
    assert_eq!(
        get_task(&temp.path, "default", &child.id)?.status,
        TaskStatus::Ready
    );
    Ok(())
}

#[test]
fn add_dependency_rolls_back_edge_and_status_when_event_insert_fails() -> anyhow::Result<()> {
    let temp = TempDb::new("add_dependency_rolls_back_edge_and_status_when_event_insert_fails")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "incomplete parent".into(),
            description: None,
            status: Some(TaskStatus::Triage),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )?;
    let child = create_task(&temp.path, "default", "tester", CreateTask::ready("child"))?;
    connect_file(&temp.path)
        ?
        .execute(
            "CREATE TRIGGER fail_dependency_added_event BEFORE INSERT ON task_events WHEN NEW.kind='dependency.added' BEGIN SELECT RAISE(ABORT, 'forced dependency.added event failure'); END",
            [],
        )
        ?;

    let err = result_err(add_dependency(
        &temp.path, "default", "tester", &parent.id, &child.id,
    ))?;

    assert!(
        err.to_string()
            .contains("forced dependency.added event failure"),
        "err: {err}"
    );
    assert_eq!(
        get_task(&temp.path, "default", &child.id)?.status,
        TaskStatus::Ready
    );
    assert!(list_dependencies(&temp.path, "default", &child.id)?.is_empty());
    Ok(())
}

#[test]
fn remove_dependency_recomputes_child_to_ready_when_unblocked() -> anyhow::Result<()> {
    let temp = TempDb::new("remove_dependency_recomputes_child_to_ready_when_unblocked")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "unfinished parent".into(),
            description: Some("spec".into()),
            status: Some(TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )?;
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("child should unblock"),
    )?;

    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id)?;
    assert_eq!(
        get_task(&temp.path, "default", &child.id)?.status,
        TaskStatus::Todo
    );

    kanban_sqlite::remove_dependency(&temp.path, "default", "tester", &parent.id, &child.id)?;

    let child = get_task(&temp.path, "default", &child.id)?;
    assert_eq!(child.status, TaskStatus::Ready);
    assert!(
        list_events(&temp.path, "default", Some(&child.id))?
            .iter()
            .any(|event| event.kind == "task.promoted")
    );
    Ok(())
}

#[test]
fn adding_incomplete_parent_to_running_child_is_rejected_without_force() -> anyhow::Result<()> {
    let temp = TempDb::new("adding_incomplete_parent_to_running_child_is_rejected_without_force")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "incomplete parent".into(),
            description: None,
            status: Some(TaskStatus::Triage),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )?;
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("running child"),
    )?;
    claim_task(&temp.path, "default", "worker", &child.id, 300_000)?;

    let err = result_err(add_dependency(
        &temp.path, "default", "tester", &parent.id, &child.id,
    ))?;

    assert!(
        err.to_string().contains("running") && err.to_string().contains("dependency"),
        "err: {err}"
    );
    assert_eq!(
        get_task(&temp.path, "default", &child.id)?.status,
        TaskStatus::Running
    );
    assert!(list_dependencies(&temp.path, "default", &child.id)?.is_empty());
    Ok(())
}

#[test]
fn add_dependency_reloads_child_inside_transaction_before_demoting_ready() -> anyhow::Result<()> {
    let temp =
        TempDb::new("add_dependency_reloads_child_inside_transaction_before_demoting_ready")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "incomplete parent".into(),
            description: None,
            status: Some(TaskStatus::Triage),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )?;
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("claimed child"),
    )?;

    let conn = connect_file(&temp.path)?;
    conn.execute_batch("BEGIN IMMEDIATE")?;
    mark_task_running_in_current_tx(&conn, &child.id)?;
    let adding = thread::spawn({
        let db_path = temp.path.clone();
        let parent_id = parent.id.clone();
        let child_id = child.id.clone();
        move || add_dependency(&db_path, "default", "tester", &parent_id, &child_id)
    });
    thread::sleep(Duration::from_millis(50));
    conn.execute_batch("COMMIT")?;

    let err = result_err(join_thread(adding)?)?;
    assert!(
        err.to_string().contains("running") && err.to_string().contains("dependency"),
        "err: {err}"
    );
    let fresh = get_task(&temp.path, "default", &child.id)?;
    assert_eq!(fresh.status, TaskStatus::Running);
    assert!(fresh.current_run_id.is_some());
    assert!(list_dependencies(&temp.path, "default", &child.id)?.is_empty());
    Ok(())
}

#[test]
fn promote_task_reloads_dependencies_inside_transaction() -> anyhow::Result<()> {
    let temp = TempDb::new("promote_task_reloads_dependencies_inside_transaction")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "unfinished parent".into(),
            description: Some("spec".into()),
            status: Some(TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )?;
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "manual promote race".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )?;

    let conn = connect_file(&temp.path)?;
    conn.execute_batch("BEGIN IMMEDIATE")?;
    conn.execute(
        "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![child.board_id, parent.id, child.id, now_ms()],
    )
    ?;
    let promoting = thread::spawn({
        let db_path = temp.path.clone();
        let child_id = child.id.clone();
        move || promote_task(&db_path, "default", "tester", &child_id)
    });
    thread::sleep(Duration::from_millis(50));
    conn.execute_batch("COMMIT")?;

    let err = result_err(join_thread(promoting)?)?;
    assert!(err.to_string().contains("dependency"), "err: {err}");
    assert_eq!(
        get_task(&temp.path, "default", &child.id)?.status,
        TaskStatus::Todo
    );
    Ok(())
}

#[test]
fn unblock_task_reloads_status_inside_transaction_before_recomputing() -> anyhow::Result<()> {
    let temp = TempDb::new("unblock_task_reloads_status_inside_transaction_before_recomputing")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("blocked archive race"),
    )?;
    block_task(
        &temp.path, "default", "tester", &task.id, "waiting", None, false,
    )?;

    let conn = connect_file(&temp.path)?;
    conn.execute_batch("BEGIN IMMEDIATE")?;
    conn.execute(
        "UPDATE tasks SET status='archived', archived_at=?1, updated_at=?1, lock_version=lock_version+1 WHERE id=?2",
        params![now_ms(), task.id],
    )
    ?;
    let unblocking = thread::spawn({
        let db_path = temp.path.clone();
        let task_id = task.id.clone();
        move || unblock_task(&db_path, "default", "tester", &task_id)
    });
    thread::sleep(Duration::from_millis(50));
    conn.execute_batch("COMMIT")?;

    let err = result_err(join_thread(unblocking)?)?;
    assert!(err.to_string().contains("unblock"), "err: {err}");
    assert_eq!(
        get_task(&temp.path, "default", &task.id)?.status,
        TaskStatus::Archived
    );
    Ok(())
}
