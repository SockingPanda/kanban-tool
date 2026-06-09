use crate::common::*;

#[test]
fn dispatch_once_runs_ready_task_and_records_log() {
    let temp = TempDb::new("dispatch_once_runs_ready_task_and_records_log");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("跑 worker"),
    )
    .unwrap();
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
    )
    .unwrap();

    assert_eq!(result.claimed, 1);
    assert_eq!(
        get_task(&temp.path, "default", &task.id).unwrap().status,
        TaskStatus::Done
    );
    let runs = list_runs(&temp.path, "default", Some(&task.id)).unwrap();
    assert_eq!(runs[0].status, "succeeded");
    let log_path = runs[0].log_path.as_ref().expect("run log path");
    assert!(std::fs::read_to_string(log_path).unwrap().contains("task="));
}

#[test]
fn dispatch_once_rejects_untrusted_log_dir_before_claiming() {
    let temp = TempDb::new("dispatch_once_rejects_untrusted_log_dir_before_claiming");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("untrusted log dir"),
    )
    .unwrap();

    let err = dispatch_once(
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
    )
    .unwrap_err();

    assert!(
        err.to_string().contains("outside allowed run log roots"),
        "{err}"
    );
    assert_eq!(
        get_task(&temp.path, "default", &task.id).unwrap().status,
        TaskStatus::Ready
    );
    assert!(
        list_runs(&temp.path, "default", Some(&task.id))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn dispatch_once_does_not_claim_review_or_dependency_blocked_tasks() {
    let temp = TempDb::new("dispatch_once_does_not_claim_review_or_dependency_blocked_tasks");
    init_database(&temp.path, "tester").unwrap();
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
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let review_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("review 不可 claim"),
    )
    .unwrap();
    let review_claim =
        claim_task(&temp.path, "default", "worker", &review_task.id, 300_000).unwrap();
    kanban_sqlite::submit_review_task(
        &temp.path,
        "default",
        "worker",
        &review_task.id,
        Some(&review_claim.claim_token),
        false,
    )
    .unwrap();
    let blocked_by_dependency = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("依赖未完成但快照被修回 ready"),
    )
    .unwrap();
    add_dependency(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        &blocked_by_dependency.id,
    )
    .unwrap();
    connect_file(&temp.path)
        .unwrap()
        .execute(
            "UPDATE tasks SET status='ready' WHERE id=?1",
            [&blocked_by_dependency.id],
        )
        .unwrap();

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
    )
    .unwrap();

    assert_eq!(result.claimed, 0);
    assert_eq!(
        get_task(&temp.path, "default", &review_task.id)
            .unwrap()
            .status,
        TaskStatus::Review
    );
    assert_eq!(
        get_task(&temp.path, "default", &blocked_by_dependency.id)
            .unwrap()
            .status,
        TaskStatus::Ready
    );
    assert!(
        list_runs(&temp.path, "default", Some(&blocked_by_dependency.id))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn claim_actor_with_quotes_and_control_chars_writes_valid_event_json() {
    let temp = TempDb::new("claim_actor_with_quotes_and_control_chars_writes_valid_event_json");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("claim JSON escaping"),
    )
    .unwrap();
    let actor = "bad\"actor\nwith\tcontrol";

    let claim = claim_task(&temp.path, "default", actor, &task.id, 300_000).unwrap();

    let fresh = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(fresh.status, TaskStatus::Running);
    assert_eq!(fresh.claim_owner.as_deref(), Some(actor));
    assert_eq!(fresh.current_run_id.as_deref(), Some(claim.run_id.as_str()));
    let events = list_events(&temp.path, "default", Some(&task.id)).unwrap();
    let event = events
        .iter()
        .find(|event| event.kind == "task.claimed")
        .expect("task.claimed event");
    assert_eq!(event.actor.as_deref(), Some(actor));
    assert_eq!(event.run_id.as_deref(), Some(claim.run_id.as_str()));
    let payload: serde_json::Value = serde_json::from_str(&event.payload_json).unwrap();
    assert_eq!(payload["claim_owner"], actor);
}

#[test]
fn dispatch_once_promotes_eligible_scheduled_and_todo_before_claiming() {
    let temp = TempDb::new("dispatch_once_promotes_eligible_scheduled_and_todo_before_claiming");
    init_database(&temp.path, "tester").unwrap();
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
            priority: 10,
            scheduled_at: Some(now - 1_000),
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
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
            metadata_json: "{}".into(),
        },
    )
    .unwrap();

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
    )
    .unwrap();

    assert_eq!(result.claimed, 1);
    assert_eq!(
        get_task(&temp.path, "default", &scheduled.id)
            .unwrap()
            .status,
        TaskStatus::Done
    );
    assert_eq!(
        get_task(&temp.path, "default", &todo.id).unwrap().status,
        TaskStatus::Ready
    );
    for task_id in [&scheduled.id, &todo.id] {
        assert!(
            list_events(&temp.path, "default", Some(task_id))
                .unwrap()
                .iter()
                .any(|event| event.kind == "task.promoted"),
            "missing task.promoted for {task_id}"
        );
    }
}

#[test]
fn dispatch_once_heartbeats_while_long_running_command_blocks() {
    let temp = TempDb::new("dispatch_once_heartbeats_while_long_running_command_blocks");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("long worker"),
    )
    .unwrap();

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
    )
    .unwrap();

    let runs = list_runs(&temp.path, "default", Some(&task.id)).unwrap();
    assert_eq!(runs[0].status, "succeeded");
    let (run_claim_expires_at, run_last_heartbeat_at): (i64, i64) = connect_file(&temp.path)
        .unwrap()
        .query_row(
            "SELECT claim_expires_at, last_heartbeat_at FROM task_runs WHERE id=?1",
            [&runs[0].id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(run_last_heartbeat_at > runs[0].started_at);
    assert!(run_claim_expires_at > runs[0].started_at + 100);
    let events = list_events(&temp.path, "default", Some(&task.id)).unwrap();
    let claimed_at = events
        .iter()
        .find(|event| event.kind == "task.claimed")
        .expect("claimed event")
        .created_at;
    let heartbeat = events
        .iter()
        .find(|event| event.kind == "task.heartbeat")
        .expect("heartbeat event");
    assert!(heartbeat.created_at > claimed_at, "events: {events:?}");
}

#[test]
fn manual_block_during_dispatch_is_not_overwritten_to_done() {
    let temp = TempDb::new("manual_block_during_dispatch_is_not_overwritten_to_done");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("manual block race"),
    )
    .unwrap();
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
                log_dir: dispatch_release.parent().unwrap().join("logs"),
            },
        )
    });

    for _ in 0..200 {
        if get_task(&temp.path, "default", &task.id).unwrap().status == TaskStatus::Running {
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
    )
    .unwrap();
    std::fs::write(&release, "go").unwrap();
    let result = handle.join().unwrap();
    assert!(result.is_err(), "dispatcher should report finish conflict");

    let fresh = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(fresh.status, TaskStatus::Blocked);
    assert_eq!(fresh.status_reason.as_deref(), Some("manual block"));
}

#[test]
fn todo_without_description_is_not_promoted_or_claimed_by_dispatch() {
    let temp = TempDb::new("todo_without_description_is_not_promoted_or_claimed_by_dispatch");
    init_database(&temp.path, "tester").unwrap();
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
            metadata_json: "{}".into(),
        },
    )
    .unwrap();

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
    )
    .unwrap();

    assert_eq!(result.claimed, 0);
    assert_eq!(
        get_task(&temp.path, "default", &task.id).unwrap().status,
        TaskStatus::Todo
    );
}

#[test]
fn worker_large_output_does_not_deadlock_under_heartbeat_wrapper() {
    let temp = TempDb::new("worker_large_output_does_not_deadlock_under_heartbeat_wrapper");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("large output"),
    )
    .unwrap();

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
    )
    .unwrap();

    assert_eq!(result.claimed, 1);
    assert_eq!(
        get_task(&temp.path, "default", &task.id).unwrap().status,
        TaskStatus::Done
    );
}

#[test]
fn dispatch_rejects_heartbeat_interval_not_less_than_claim_ttl() {
    let temp = TempDb::new("dispatch_rejects_heartbeat_interval_not_less_than_claim_ttl");
    init_database(&temp.path, "tester").unwrap();
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("bad interval"),
    )
    .unwrap();

    let err = dispatch_once(
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
    )
    .unwrap_err();

    assert!(err.to_string().contains("heartbeat_interval_ms"));
}
