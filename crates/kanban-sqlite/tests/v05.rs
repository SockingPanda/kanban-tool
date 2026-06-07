use std::{
    path::Path,
    sync::{Arc, Barrier},
    thread,
    time::Duration,
};

use kanban_core::TaskStatus;
use kanban_sqlite::{
    CreateTask, DispatchOptions, FinishPolicy, TaskPatch, add_dependency, archive_task, block_task,
    claim_task, complete_task, connect_file, create_task, dispatch_once, get_task, init_database,
    list_dependencies, list_events, list_runs, list_tasks, unblock_task, update_task,
};
use rusqlite::params;

#[test]
fn task_crud_writes_events_and_hides_archived_by_default() {
    let temp = TempDb::new("task_crud_writes_events_and_hides_archived_by_default");
    init_database(&temp.path, "tester").unwrap();

    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "实现 Task CRUD".into(),
            description: Some("规格".into()),
            status: None,
            assignee: None,
            priority: 10,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();

    assert_eq!(task.seq, 1);
    assert_eq!(task.status, TaskStatus::Ready);
    assert_eq!(
        list_events(&temp.path, "default", Some(&task.id)).unwrap()[0].kind,
        "task.created"
    );

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            title: Some("实现 Task CRUD v0.5".into()),
            description: None,
            assignee: Some(Some("worker-a".into())),
            priority: Some(20),
            scheduled_at: None,
            due_at: None,
            metadata_json: None,
            expected_lock_version: Some(task.lock_version),
        },
    )
    .unwrap();
    assert_eq!(updated.title, "实现 Task CRUD v0.5");
    assert_eq!(updated.lock_version, task.lock_version + 1);

    archive_task(&temp.path, "default", "tester", &task.id, false).unwrap();
    assert!(
        list_tasks(&temp.path, "default", &[], false)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        list_tasks(&temp.path, "default", &[], true).unwrap().len(),
        1
    );
}

#[test]
fn explicit_ready_create_requires_ready_prerequisites() {
    let temp = TempDb::new("explicit_ready_create_requires_ready_prerequisites");
    init_database(&temp.path, "tester").unwrap();

    let missing_spec = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "not ready".into(),
            description: None,
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap_err();

    assert!(
        missing_spec
            .to_string()
            .contains("ready requires description"),
        "err: {missing_spec}"
    );

    let future_ready = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "future ready".into(),
            description: Some("spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: Some(now_ms() + 60_000),
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap_err();

    assert!(
        future_ready
            .to_string()
            .contains("ready requires scheduled_at to be due"),
        "err: {future_ready}"
    );
}

#[test]
fn force_archive_running_task_closes_active_run() {
    let temp = TempDb::new("force_archive_running_task_closes_active_run");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("archive running"),
    )
    .unwrap();
    let claim = claim_task(&temp.path, "default", "worker", &task.id, 300_000).unwrap();

    let archived = archive_task(&temp.path, "default", "tester", &task.id, true).unwrap();

    assert_eq!(archived.status, TaskStatus::Archived);
    assert!(archived.claim_token.is_none());
    assert!(archived.claim_owner.is_none());
    assert!(archived.claim_expires_at.is_none());
    let runs = list_runs(&temp.path, "default", Some(&task.id)).unwrap();
    let run = runs.iter().find(|run| run.id == claim.run_id).unwrap();
    assert_eq!(run.status, "canceled");
    assert!(run.finished_at.is_some());
    assert!(runs.iter().all(|run| run.status != "running"));
    assert!(
        list_events(&temp.path, "default", Some(&task.id))
            .unwrap()
            .iter()
            .any(|event| event.kind == "task.archived"
                && event.run_id.as_deref() == Some(&claim.run_id))
    );
}

#[test]
fn block_reason_with_control_chars_writes_valid_event_json() {
    let temp = TempDb::new("block_reason_with_control_chars_writes_valid_event_json");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("block control chars"),
    )
    .unwrap();
    let reason = "line one\nline two\tquote \" slash \\ control \u{0008}";

    let blocked = block_task(
        &temp.path, "default", "tester", &task.id, reason, None, false,
    )
    .unwrap();

    assert_eq!(blocked.status, TaskStatus::Blocked);
    assert_eq!(blocked.status_reason.as_deref(), Some(reason));
    let events = list_events(&temp.path, "default", Some(&task.id)).unwrap();
    let event = events
        .iter()
        .find(|event| event.kind == "task.blocked")
        .unwrap();
    let payload: serde_json::Value = serde_json::from_str(&event.payload_json).unwrap();
    assert_eq!(payload["reason"], reason);
}

#[test]
fn block_rolls_back_task_state_when_event_insert_fails() {
    let temp = TempDb::new("block_rolls_back_task_state_when_event_insert_fails");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("block rollback"),
    )
    .unwrap();
    connect_file(&temp.path)
        .unwrap()
        .execute(
            "CREATE TRIGGER fail_block_event BEFORE INSERT ON task_events WHEN NEW.kind='task.blocked' BEGIN SELECT RAISE(ABORT, 'forced task.blocked event failure'); END",
            [],
        )
        .unwrap();

    let err = block_task(
        &temp.path, "default", "tester", &task.id, "blocked", None, false,
    )
    .unwrap_err();

    assert!(
        err.to_string()
            .contains("forced task.blocked event failure")
    );
    let fresh = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(fresh.status, TaskStatus::Ready);
    assert!(fresh.status_reason.is_none());
}

#[test]
fn update_rolls_back_task_state_when_event_insert_fails() {
    let temp = TempDb::new("update_rolls_back_task_state_when_event_insert_fails");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("original title"),
    )
    .unwrap();
    connect_file(&temp.path)
        .unwrap()
        .execute(
            "CREATE TRIGGER fail_update_event BEFORE INSERT ON task_events WHEN NEW.kind='task.updated' BEGIN SELECT RAISE(ABORT, 'forced task.updated event failure'); END",
            [],
        )
        .unwrap();

    let err = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            title: Some("changed title".into()),
            priority: Some(99),
            ..TaskPatch::default()
        },
    )
    .unwrap_err();

    assert!(
        err.to_string()
            .contains("forced task.updated event failure"),
        "err: {err}"
    );
    let fresh = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(fresh.title, "original title");
    assert_eq!(fresh.priority, task.priority);
    assert_eq!(fresh.lock_version, task.lock_version);
}

#[test]
fn updating_ready_task_to_future_schedule_makes_it_unclaimable_until_due() {
    let temp = TempDb::new("updating_ready_task_to_future_schedule_makes_it_unclaimable_until_due");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("future scheduled update"),
    )
    .unwrap();

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            scheduled_at: Some(Some(now_ms() + 3_600_000)),
            ..TaskPatch::default()
        },
    )
    .unwrap();

    assert_eq!(updated.status, TaskStatus::Scheduled);
    assert!(claim_task(&temp.path, "default", "worker", &task.id, 300_000).is_err());
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
        TaskStatus::Scheduled
    );
}

#[test]
fn clearing_schedule_recomputes_complete_task_without_dependencies_to_ready() {
    let temp =
        TempDb::new("clearing_schedule_recomputes_complete_task_without_dependencies_to_ready");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "scheduled complete".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Scheduled),
            assignee: None,
            priority: 0,
            scheduled_at: Some(now_ms() + 3_600_000),
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            scheduled_at: Some(None),
            ..TaskPatch::default()
        },
    )
    .unwrap();

    assert_eq!(updated.scheduled_at, None);
    assert_eq!(updated.status, TaskStatus::Ready);
}

#[test]
fn clearing_schedule_recomputes_incomplete_dependencies_to_todo() {
    let temp = TempDb::new("clearing_schedule_recomputes_incomplete_dependencies_to_todo");
    init_database(&temp.path, "tester").unwrap();
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("incomplete parent"),
    )
    .unwrap();
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "scheduled child".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Scheduled),
            assignee: None,
            priority: 0,
            scheduled_at: Some(now_ms() + 3_600_000),
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id).unwrap();

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &child.id,
        TaskPatch {
            scheduled_at: Some(None),
            ..TaskPatch::default()
        },
    )
    .unwrap();

    assert_eq!(updated.status, TaskStatus::Todo);
}

#[test]
fn clearing_schedule_recomputes_missing_description_to_triage() {
    let temp = TempDb::new("clearing_schedule_recomputes_missing_description_to_triage");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "scheduled missing spec".into(),
            description: None,
            status: Some(TaskStatus::Scheduled),
            assignee: None,
            priority: 0,
            scheduled_at: Some(now_ms() + 3_600_000),
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            scheduled_at: Some(None),
            ..TaskPatch::default()
        },
    )
    .unwrap();

    assert_eq!(updated.status, TaskStatus::Triage);
}

#[test]
fn claim_complete_and_dependencies_promote_children() {
    let temp = TempDb::new("claim_complete_and_dependencies_promote_children");
    init_database(&temp.path, "tester").unwrap();
    let parent = create_task(&temp.path, "default", "tester", CreateTask::ready("父任务")).unwrap();
    let child = create_task(&temp.path, "default", "tester", CreateTask::ready("子任务")).unwrap();

    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id).unwrap();
    assert_eq!(
        get_task(&temp.path, "default", &child.id).unwrap().status,
        TaskStatus::Todo
    );

    let claim = claim_task(&temp.path, "default", "worker", &parent.id, 300_000).unwrap();
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
    )
    .unwrap();
    assert!(heartbeat.claim_expires_at > claim.task.claim_expires_at);

    complete_task(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        Some(&claim.claim_token),
        false,
    )
    .unwrap();
    assert_eq!(
        get_task(&temp.path, "default", &parent.id).unwrap().status,
        TaskStatus::Done
    );
    assert_eq!(
        get_task(&temp.path, "default", &child.id).unwrap().status,
        TaskStatus::Ready
    );
    assert_eq!(
        list_runs(&temp.path, "default", Some(&parent.id)).unwrap()[0].status,
        "succeeded"
    );
}

#[test]
fn block_unblock_recomputes_target_and_cycle_detection_rejects_cycles() {
    let temp = TempDb::new("block_unblock_recomputes_target_and_cycle_detection_rejects_cycles");
    init_database(&temp.path, "tester").unwrap();
    let parent = create_task(&temp.path, "default", "tester", CreateTask::ready("父任务")).unwrap();
    let child = create_task(&temp.path, "default", "tester", CreateTask::ready("子任务")).unwrap();
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id).unwrap();

    let err = add_dependency(&temp.path, "default", "tester", &child.id, &parent.id).unwrap_err();
    assert!(err.to_string().contains("cycle"));

    block_task(
        &temp.path,
        "default",
        "tester",
        &child.id,
        "等待输入",
        None,
        false,
    )
    .unwrap();
    let unblocked = unblock_task(&temp.path, "default", "tester", &child.id).unwrap();
    assert_eq!(unblocked.status, TaskStatus::Todo);

    let claim = claim_task(&temp.path, "default", "worker", &parent.id, 300_000).unwrap();
    complete_task(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        Some(&claim.claim_token),
        false,
    )
    .unwrap();
    assert_eq!(
        get_task(&temp.path, "default", &child.id).unwrap().status,
        TaskStatus::Ready
    );
}

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
fn concurrent_claim_attempts_on_one_ready_task_have_exactly_one_success() {
    let temp = TempDb::new("concurrent_claim_attempts_on_one_ready_task_have_exactly_one_success");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("只允许一个 claim"),
    )
    .unwrap();

    let path = Arc::new(temp.path.clone());
    let task_id = Arc::new(task.id.clone());
    let barrier = Arc::new(Barrier::new(2));
    let mut handles = Vec::new();
    for actor in ["worker-a", "worker-b"] {
        let path = Arc::clone(&path);
        let task_id = Arc::clone(&task_id);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            claim_task(&*path, "default", actor, &task_id, 300_000)
        }));
    }

    let results = handles
        .into_iter()
        .map(|handle| handle.join().expect("claim thread should not panic"))
        .collect::<Vec<_>>();
    let successes = results.iter().filter(|result| result.is_ok()).count();
    let failures = results.iter().filter(|result| result.is_err()).count();
    assert_eq!(successes, 1, "results: {results:?}");
    assert_eq!(failures, 1, "results: {results:?}");

    let claimed = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(claimed.status, TaskStatus::Running);
    assert!(claimed.claim_token.is_some());
    assert_eq!(
        list_runs(&temp.path, "default", Some(&task.id))
            .unwrap()
            .iter()
            .filter(|run| run.status == "running")
            .count(),
        1
    );
}

#[test]
fn claimed_task_has_current_run_running_run_and_claimed_event() {
    let temp = TempDb::new("claimed_task_has_current_run_running_run_and_claimed_event");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("claim invariant"),
    )
    .unwrap();

    let claim = claim_task(&temp.path, "default", "worker", &task.id, 300_000).unwrap();
    let claimed = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(claimed.status, TaskStatus::Running);
    assert_eq!(
        claimed.current_run_id.as_deref(),
        Some(claim.run_id.as_str())
    );

    let running_runs = list_runs(&temp.path, "default", Some(&task.id))
        .unwrap()
        .into_iter()
        .filter(|run| run.status == "running")
        .collect::<Vec<_>>();
    assert_eq!(running_runs.len(), 1);
    assert_eq!(running_runs[0].id, claim.run_id);

    let events = list_events(&temp.path, "default", Some(&task.id)).unwrap();
    assert!(
        events.iter().any(|event| event.kind == "task.claimed"
            && event.run_id.as_deref() == claimed.current_run_id.as_deref()),
        "events: {events:?}"
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
fn add_dependency_rolls_back_edge_and_status_when_event_insert_fails() {
    let temp = TempDb::new("add_dependency_rolls_back_edge_and_status_when_event_insert_fails");
    init_database(&temp.path, "tester").unwrap();
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
    )
    .unwrap();
    let child = create_task(&temp.path, "default", "tester", CreateTask::ready("child")).unwrap();
    connect_file(&temp.path)
        .unwrap()
        .execute(
            "CREATE TRIGGER fail_dependency_added_event BEFORE INSERT ON task_events WHEN NEW.kind='dependency.added' BEGIN SELECT RAISE(ABORT, 'forced dependency.added event failure'); END",
            [],
        )
        .unwrap();

    let err = add_dependency(&temp.path, "default", "tester", &parent.id, &child.id).unwrap_err();

    assert!(
        err.to_string()
            .contains("forced dependency.added event failure"),
        "err: {err}"
    );
    assert_eq!(
        get_task(&temp.path, "default", &child.id).unwrap().status,
        TaskStatus::Ready
    );
    assert!(
        list_dependencies(&temp.path, "default", &child.id)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn remove_dependency_recomputes_child_to_ready_when_unblocked() {
    let temp = TempDb::new("remove_dependency_recomputes_child_to_ready_when_unblocked");
    init_database(&temp.path, "tester").unwrap();
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
    )
    .unwrap();
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("child should unblock"),
    )
    .unwrap();

    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id).unwrap();
    assert_eq!(
        get_task(&temp.path, "default", &child.id).unwrap().status,
        TaskStatus::Todo
    );

    kanban_sqlite::remove_dependency(&temp.path, "default", "tester", &parent.id, &child.id)
        .unwrap();

    let child = get_task(&temp.path, "default", &child.id).unwrap();
    assert_eq!(child.status, TaskStatus::Ready);
    assert!(
        list_events(&temp.path, "default", Some(&child.id))
            .unwrap()
            .iter()
            .any(|event| event.kind == "task.promoted")
    );
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
fn heartbeat_against_stale_non_running_claim_fails_without_touching_heartbeat_fields() {
    let temp = TempDb::new(
        "heartbeat_against_stale_non_running_claim_fails_without_touching_heartbeat_fields",
    );
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("stale hb"),
    )
    .unwrap();
    let claim = claim_task(&temp.path, "default", "worker", &task.id, 300_000).unwrap();
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
    let blocked = get_task(&temp.path, "default", &task.id).unwrap();

    let err = kanban_sqlite::heartbeat_task(
        &temp.path,
        "default",
        "worker",
        &task.id,
        &claim.claim_token,
        600_000,
    )
    .unwrap_err();
    assert!(err.to_string().contains("matching running claim"));

    let after = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(after.status, TaskStatus::Blocked);
    assert_eq!(after.claim_expires_at, blocked.claim_expires_at);
    assert_eq!(after.last_heartbeat_at, blocked.last_heartbeat_at);
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

#[test]
fn adding_incomplete_parent_to_running_child_is_rejected_without_force() {
    let temp = TempDb::new("adding_incomplete_parent_to_running_child_is_rejected_without_force");
    init_database(&temp.path, "tester").unwrap();
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
    )
    .unwrap();
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("running child"),
    )
    .unwrap();
    claim_task(&temp.path, "default", "worker", &child.id, 300_000).unwrap();

    let err = add_dependency(&temp.path, "default", "tester", &parent.id, &child.id).unwrap_err();

    assert!(
        err.to_string().contains("running") && err.to_string().contains("dependency"),
        "err: {err}"
    );
    assert_eq!(
        get_task(&temp.path, "default", &child.id).unwrap().status,
        TaskStatus::Running
    );
    assert!(
        list_dependencies(&temp.path, "default", &child.id)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn failed_ready_retry_policy_increments_retry_count_and_blocks_at_max_retries() {
    let temp =
        TempDb::new("failed_ready_retry_policy_increments_retry_count_and_blocks_at_max_retries");
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
}

#[test]
fn reclaim_expired_increments_retry_count_and_blocks_at_max_retries() {
    let temp = TempDb::new("reclaim_expired_increments_retry_count_and_blocks_at_max_retries");
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
}

#[test]
fn reclaim_expired_skips_task_heartbeated_after_scan_before_claim_tx() {
    let temp = TempDb::new("reclaim_expired_skips_task_heartbeated_after_scan_before_claim_tx");
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
}

struct TempDb {
    dir: std::path::PathBuf,
    path: std::path::PathBuf,
}

impl TempDb {
    fn new(name: &str) -> Self {
        let mut dir = std::env::temp_dir();
        dir.push(format!("kb-v05-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("kb.db");
        Self { dir, path }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        if Path::new(&self.dir).exists() {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn set_retry_policy(path: &Path, task_id: &str, max_retries: i64) {
    connect_file(path)
        .unwrap()
        .execute(
            "UPDATE tasks SET max_retries=?1 WHERE id=?2",
            rusqlite::params![max_retries, task_id],
        )
        .unwrap();
}
