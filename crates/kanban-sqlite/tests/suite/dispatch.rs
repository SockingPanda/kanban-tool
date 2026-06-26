use crate::common::*;

#[test]
fn dispatch_once_runs_ready_task_and_records_log() -> anyhow::Result<()> {
    let temp = TempDb::new("dispatch_once_runs_ready_task_and_records_log")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("跑 worker"),
    )?;
    mark_execution_plan_not_required(&temp.path, "default", "tester", &task.id, "small task")?;
    let log_dir = temp.dir.join("logs");

    let result = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "sh -c 'echo task=$KB_TASK_ID; test -n \"$KB_CLAIM_TOKEN\"'".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: log_dir.clone(),
        },
    )?;

    assert_eq!(result.claimed, 1);
    assert_eq!(
        get_task(&temp.path, "default", &task.id)?.status,
        TaskStatus::Done
    );
    let runs = list_runs(&temp.path, "default", Some(&task.id))?;
    assert_eq!(runs[0].status, "succeeded");
    let log_path = runs[0]
        .log_path
        .as_ref()
        .ok_or_else(|| test_error("expected run log path"))?;
    assert!(std::fs::read_to_string(log_path)?.contains("task="));
    Ok(())
}

#[test]
fn dispatch_once_rejects_untrusted_log_dir_before_claiming() -> anyhow::Result<()> {
    let temp = TempDb::new("dispatch_once_rejects_untrusted_log_dir_before_claiming")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("untrusted log dir"),
    )?;
    mark_execution_plan_not_required(&temp.path, "default", "tester", &task.id, "small task")?;

    let err = result_err(dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "printf should-not-run".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: temp.dir.join("custom-logs"),
        },
    ))?;

    assert!(
        err.to_string().contains("outside allowed run log roots"),
        "{err}"
    );
    assert_eq!(
        get_task(&temp.path, "default", &task.id)?.status,
        TaskStatus::Ready
    );
    assert!(list_runs(&temp.path, "default", Some(&task.id))?.is_empty());
    Ok(())
}

#[test]
fn dispatch_once_recovers_post_claim_log_dir_setup_failure() -> anyhow::Result<()> {
    let temp = TempDb::new("dispatch_once_recovers_post_claim_log_dir_setup_failure")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("log dir setup fails after claim"),
    )?;
    mark_execution_plan_not_required(&temp.path, "default", "tester", &task.id, "small task")?;
    set_retry_policy(&temp.path, &task.id, 2)?;
    let log_dir = temp.dir.join("logs");
    std::fs::write(&log_dir, "not a directory")?;

    let err = result_err(dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "printf should-not-run".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir,
        },
    ))?;

    assert!(err.to_string().contains("storage error"), "{err}");
    let fresh = get_task(&temp.path, "default", &task.id)?;
    assert_eq!(fresh.status, TaskStatus::Ready);
    assert_eq!(fresh.retry_count, 1);
    assert!(fresh.claim_token.is_none());
    assert!(fresh.current_run_id.is_some());
    let runs = list_runs(&temp.path, "default", Some(&task.id))?;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, "failed");
    assert_eq!(runs[0].error.as_deref(), Some("dispatcher setup failed"));
    assert!(runs[0].finished_at.is_some());
    Ok(())
}

#[cfg(unix)]
#[test]
fn dispatch_once_recovers_post_claim_worker_start_failure() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDb::new("dispatch_once_recovers_post_claim_worker_start_failure")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("worker start fails after claim"),
    )?;
    mark_execution_plan_not_required(&temp.path, "default", "tester", &task.id, "small task")?;
    set_retry_policy(&temp.path, &task.id, 2)?;
    let log_dir = temp.dir.join("logs");
    std::fs::create_dir_all(&log_dir)?;
    std::fs::set_permissions(&log_dir, std::fs::Permissions::from_mode(0o500))?;

    let err = result_err(dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "printf\0should-not-run".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: log_dir.clone(),
        },
    ));
    std::fs::set_permissions(&log_dir, std::fs::Permissions::from_mode(0o700))?;
    let err = err?;

    assert!(err.to_string().contains("storage error"), "{err}");
    let fresh = get_task(&temp.path, "default", &task.id)?;
    assert_eq!(fresh.status, TaskStatus::Ready);
    assert_eq!(fresh.retry_count, 1);
    assert!(fresh.claim_token.is_none());
    assert!(fresh.current_run_id.is_some());
    let runs = list_runs(&temp.path, "default", Some(&task.id))?;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, "failed");
    assert_eq!(runs[0].error.as_deref(), Some("dispatcher worker failed"));
    assert!(runs[0].finished_at.is_some());
    assert!(runs[0].log_path.is_some());
    Ok(())
}

#[test]
fn dispatch_once_does_not_claim_review_or_dependency_blocked_tasks() -> anyhow::Result<()> {
    let temp = TempDb::new("dispatch_once_does_not_claim_review_or_dependency_blocked_tasks")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "未完成父任务".into(),
            description: Some(String::new()),
            status: Some(TaskStatus::Triage),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    let review_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("review 不可 claim"),
    )?;
    mark_execution_plan_not_required(
        &temp.path,
        "default",
        "tester",
        &review_task.id,
        "small task",
    )?;
    let review_claim = claim_task(&temp.path, "default", "worker", &review_task.id, 300_000)?;
    kanban_sqlite::submit_review_task(
        &temp.path,
        "default",
        "worker",
        &review_task.id,
        Some(&review_claim.claim_token),
        false,
    )?;
    let blocked_by_dependency = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("依赖未完成但快照被修回 ready"),
    )?;
    add_dependency(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        &blocked_by_dependency.id,
    )?;
    connect_file(&temp.path)?.execute(
        "UPDATE tasks SET status='ready' WHERE id=?1",
        [&blocked_by_dependency.id],
    )?;

    let result = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "true".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: temp.dir.join("logs"),
        },
    )?;

    assert_eq!(result.claimed, 0);
    assert_eq!(
        get_task(&temp.path, "default", &review_task.id)?.status,
        TaskStatus::Review
    );
    assert_eq!(
        get_task(&temp.path, "default", &blocked_by_dependency.id)?.status,
        TaskStatus::Ready
    );
    assert!(list_runs(&temp.path, "default", Some(&blocked_by_dependency.id))?.is_empty());
    Ok(())
}

#[test]
fn dispatch_once_claims_ready_task_with_only_archived_parent() -> anyhow::Result<()> {
    let temp = TempDb::new("dispatch_once_claims_ready_task_with_only_archived_parent")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("archived parent"),
    )?;
    archive_task(&temp.path, "default", "tester", &parent.id, false)?;
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("ready child"),
    )?;
    mark_execution_plan_not_required(&temp.path, "default", "tester", &child.id, "small task")?;
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id)?;
    connect_file(&temp.path)?
        .execute("UPDATE tasks SET status='ready' WHERE id=?1", [&child.id])?;

    let result = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "true".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: temp.dir.join("logs"),
        },
    )?;

    assert_eq!(result.claimed, 1);
    assert_eq!(result.task_id.as_deref(), Some(child.id.as_str()));
    assert_eq!(
        get_task(&temp.path, "default", &child.id)?.status,
        TaskStatus::Done
    );
    Ok(())
}

#[test]
fn dispatch_once_does_not_auto_promote_unblocked_todo() -> anyhow::Result<()> {
    let temp = TempDb::new("dispatch_once_does_not_auto_promote_unblocked_todo")?;
    init_database(&temp.path, "tester")?;
    let todo = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "manual ready intent required".into(),
            description: Some("spec".into()),
            status: Some(TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;

    let result = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "true".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: temp.dir.join("logs"),
        },
    )?;

    assert_eq!(result.claimed, 0);
    let fresh = get_task(&temp.path, "default", &todo.id)?;
    assert_eq!(fresh.status, TaskStatus::Todo);
    assert!(!fresh.dependency_blocked);
    assert!(list_runs(&temp.path, "default", Some(&todo.id))?.is_empty());
    Ok(())
}

#[test]
fn claim_actor_with_quotes_and_control_chars_writes_valid_event_json() -> anyhow::Result<()> {
    let temp = TempDb::new("claim_actor_with_quotes_and_control_chars_writes_valid_event_json")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("claim JSON escaping"),
    )?;
    mark_execution_plan_not_required(&temp.path, "default", "tester", &task.id, "small task")?;
    let actor = "bad\"actor\nwith\tcontrol";

    let claim = claim_task(&temp.path, "default", actor, &task.id, 300_000)?;

    let fresh = get_task(&temp.path, "default", &task.id)?;
    assert_eq!(fresh.status, TaskStatus::Running);
    assert_eq!(fresh.claim_owner.as_deref(), Some(actor));
    assert_eq!(fresh.current_run_id.as_deref(), Some(claim.run_id.as_str()));
    let events = list_events(&temp.path, "default", Some(&task.id))?;
    let event = events
        .iter()
        .find(|event| event.kind == "task.claimed")
        .ok_or_else(|| test_error("expected task.claimed event"))?;
    assert_eq!(event.actor.as_deref(), Some(actor));
    assert_eq!(event.run_id.as_deref(), Some(claim.run_id.as_str()));
    let payload: serde_json::Value = serde_json::from_str(&event.payload_json)?;
    assert_eq!(payload["claim_owner"], actor);
    Ok(())
}

#[test]
fn dispatch_once_does_not_promote_scheduled_or_todo_before_claiming() -> anyhow::Result<()> {
    let temp = TempDb::new("dispatch_once_does_not_promote_scheduled_or_todo_before_claiming")?;
    init_database(&temp.path, "tester")?;
    let now = now_ms();
    let scheduled = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "due scheduled".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Scheduled),
            assignee: None,
            priority: 0,
            scheduled_at: Some(now - 1_000),
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    let todo = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "eligible todo".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Todo),
            assignee: None,
            priority: 1,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;

    let result = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "true".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: temp.dir.join("logs"),
        },
    )?;

    assert_eq!(result.claimed, 0);
    assert_eq!(
        get_task(&temp.path, "default", &scheduled.id)?.status,
        TaskStatus::Scheduled
    );
    assert_eq!(
        get_task(&temp.path, "default", &todo.id)?.status,
        TaskStatus::Todo
    );
    for task_id in [&scheduled.id, &todo.id] {
        assert!(
            !list_events(&temp.path, "default", Some(task_id))?
                .iter()
                .any(|event| event.kind == "task.promoted"),
            "unexpected task.promoted for {task_id}"
        );
    }
    Ok(())
}

#[test]
fn dispatch_once_heartbeats_while_long_running_command_blocks() -> anyhow::Result<()> {
    let temp = TempDb::new("dispatch_once_heartbeats_while_long_running_command_blocks")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("long worker"),
    )?;
    mark_execution_plan_not_required(&temp.path, "default", "tester", &task.id, "small task")?;

    dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "sleep 0.25".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 100,
            heartbeat_interval_ms: 25,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: temp.dir.join("logs"),
        },
    )?;

    let runs = list_runs(&temp.path, "default", Some(&task.id))?;
    assert_eq!(runs[0].status, "succeeded");
    let (run_claim_expires_at, run_last_heartbeat_at): (i64, i64) = connect_file(&temp.path)?
        .query_row(
            "SELECT claim_expires_at, last_heartbeat_at FROM task_runs WHERE id=?1",
            [&runs[0].id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
    assert!(run_last_heartbeat_at > runs[0].started_at);
    assert!(run_claim_expires_at > runs[0].started_at + 100);
    let events = list_events(&temp.path, "default", Some(&task.id))?;
    let claimed_at = events
        .iter()
        .find(|event| event.kind == "task.claimed")
        .ok_or_else(|| test_error("expected task.claimed event"))?
        .created_at;
    let heartbeat = events
        .iter()
        .find(|event| event.kind == "task.heartbeat")
        .ok_or_else(|| test_error("expected task.heartbeat event"))?;
    assert!(heartbeat.created_at > claimed_at, "events: {events:?}");
    Ok(())
}

#[test]
fn manual_block_during_dispatch_is_not_overwritten_to_done() -> anyhow::Result<()> {
    let temp = TempDb::new("manual_block_during_dispatch_is_not_overwritten_to_done")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("manual block race"),
    )?;
    let release = temp.dir.join("release");

    let dispatch_path = temp.path.clone();
    let dispatch_release = release.clone();
    let handle = thread::spawn(move || {
        dispatch_once(
            &dispatch_path,
            "default",
            DispatchOptions {
                actor: "dispatcher".into(),
                command: format!(
                    "while [ ! -f '{}' ]; do sleep 0.01; done; true",
                    dispatch_release.display()
                ),
                worker_profile: "default".into(),
                claim_ttl_ms: 300_000,
                heartbeat_interval_ms: 300_000,
                on_success: FinishPolicy::Done,
                on_failure: FinishPolicy::Blocked,
                log_dir: dispatch_release
                    .parent()
                    .ok_or_else(|| KanbanError::InvalidInput("release path has no parent".into()))?
                    .join("logs"),
            },
        )
    });

    for _ in 0..200 {
        if get_task(&temp.path, "default", &task.id)?.status == TaskStatus::Running {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    block_task(
        &temp.path,
        "default",
        "human",
        &task.id,
        "manual block",
        None,
        true,
    )?;
    std::fs::write(&release, "go")?;
    let result = join_thread(handle)?;
    assert!(result.is_err(), "dispatcher should report finish conflict");

    let fresh = get_task(&temp.path, "default", &task.id)?;
    assert_eq!(fresh.status, TaskStatus::Blocked);
    assert_eq!(fresh.status_reason.as_deref(), Some("manual block"));
    Ok(())
}

#[test]
fn todo_without_description_is_not_promoted_or_claimed_by_dispatch() -> anyhow::Result<()> {
    let temp = TempDb::new("todo_without_description_is_not_promoted_or_claimed_by_dispatch")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "needs spec".into(),
            description: None,
            status: Some(TaskStatus::Todo),
            assignee: None,
            priority: 1,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;

    let result = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "true".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: temp.dir.join("logs"),
        },
    )?;

    assert_eq!(result.claimed, 0);
    assert_eq!(
        get_task(&temp.path, "default", &task.id)?.status,
        TaskStatus::Todo
    );
    Ok(())
}

#[test]
fn worker_large_output_does_not_deadlock_under_heartbeat_wrapper() -> anyhow::Result<()> {
    let temp = TempDb::new("worker_large_output_does_not_deadlock_under_heartbeat_wrapper")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("large output"),
    )?;
    mark_execution_plan_not_required(&temp.path, "default", "tester", &task.id, "small task")?;

    let result = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "python3 -c 'import sys; sys.stdout.write(\"x\" * 2000000)'".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 100,
            heartbeat_interval_ms: 25,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: temp.dir.join("logs"),
        },
    )?;

    assert_eq!(result.claimed, 1);
    assert_eq!(
        get_task(&temp.path, "default", &task.id)?.status,
        TaskStatus::Done
    );
    Ok(())
}

#[test]
fn dispatch_rejects_heartbeat_interval_not_less_than_claim_ttl() -> anyhow::Result<()> {
    let temp = TempDb::new("dispatch_rejects_heartbeat_interval_not_less_than_claim_ttl")?;
    init_database(&temp.path, "tester")?;
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("bad interval"),
    )?;

    let err = result_err(dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "true".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 100,
            heartbeat_interval_ms: 100,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: temp.dir.join("logs"),
        },
    ))?;

    assert!(err.to_string().contains("heartbeat_interval_ms"));
    Ok(())
}
