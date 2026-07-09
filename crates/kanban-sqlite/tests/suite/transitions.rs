use crate::common::*;

#[test]
fn explicit_ready_create_requires_ready_prerequisites() -> anyhow::Result<()> {
    let temp = TempDb::new("explicit_ready_create_requires_ready_prerequisites")?;
    init_database(&temp.path, "tester")?;

    let missing_spec = result_err(create_task(
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
            max_retries: None,
            metadata_json: "{}".into(),
        },
    ))?;

    assert!(
        missing_spec
            .to_string()
            .contains("ready requires description"),
        "err: {missing_spec}"
    );

    let future_ready = result_err(create_task(
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
            max_retries: None,
            metadata_json: "{}".into(),
        },
    ))?;

    assert!(
        future_ready
            .to_string()
            .contains("ready requires scheduled_at to be due"),
        "err: {future_ready}"
    );
    Ok(())
}

#[test]
fn claim_rejects_nonpositive_ttl_without_mutating_task() -> anyhow::Result<()> {
    let temp = TempDb::new("claim_rejects_nonpositive_ttl_without_mutating_task")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("claim ttl validation"),
    )?;

    for ttl_ms in [0, -1] {
        let err = result_err(claim_task(
            &temp.path, "default", "worker", &task.id, ttl_ms,
        ))?;
        assert!(err.to_string().contains("ttl_ms must be positive"));
    }

    let unchanged = get_task(&temp.path, "default", &task.id)?;
    assert_eq!(unchanged.status, TaskStatus::Todo);
    assert!(unchanged.claim_token.is_none());
    assert!(unchanged.claim_owner.is_none());
    assert!(unchanged.claim_expires_at.is_none());
    assert!(list_runs(&temp.path, "default", Some(&task.id))?.is_empty());
    assert!(
        list_events(&temp.path, "default", Some(&task.id))?
            .iter()
            .all(|event| event.kind != "task.claimed")
    );
    Ok(())
}

#[test]
fn heartbeat_rejects_nonpositive_ttl_without_shortening_claim() -> anyhow::Result<()> {
    let temp = TempDb::new("heartbeat_rejects_nonpositive_ttl_without_shortening_claim")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("heartbeat ttl validation"),
    )?;
    mark_execution_plan_not_required(&temp.path, "default", "tester", &task.id, "small task")?;
    let claim = claim_task(&temp.path, "default", "worker", &task.id, 300_000)?;

    for ttl_ms in [0, -1] {
        let err = result_err(kanban_sqlite::api::heartbeat_task(
            &temp.path,
            "default",
            "worker",
            &task.id,
            &claim.claim_token,
            ttl_ms,
        ))?;
        assert!(err.to_string().contains("ttl_ms must be positive"));
    }

    let unchanged = get_task(&temp.path, "default", &task.id)?;
    assert_eq!(unchanged.status, TaskStatus::Running);
    assert_eq!(
        unchanged.claim_expires_at, claim.task.claim_expires_at,
        "invalid heartbeat must not shorten or extend the active lease"
    );
    assert_eq!(unchanged.last_heartbeat_at, claim.task.last_heartbeat_at);
    let heartbeat_events = list_events(&temp.path, "default", Some(&task.id))?
        .into_iter()
        .filter(|event| event.kind == "task.heartbeat")
        .count();
    assert_eq!(heartbeat_events, 0);
    Ok(())
}

#[test]
fn running_task_scoped_activity_event_renews_claim_without_heartbeat_event() -> anyhow::Result<()> {
    let temp = TempDb::new("running_task_scoped_activity_event_renews_claim")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("implicit activity lease"),
    )?;
    mark_execution_plan_not_required(&temp.path, "default", "tester", &task.id, "small task")?;
    let claim = claim_task(&temp.path, "default", "worker", &task.id, 60_000)?;
    let before = get_task(&temp.path, "default", &task.id)?;
    let before_run_heartbeat = run_last_heartbeat(&temp.path, &claim.run_id)?;

    thread::sleep(Duration::from_millis(20));
    create_comment(&temp.path, &task.id, "operator", "progress activity", None)?;

    let after = get_task(&temp.path, "default", &task.id)?;
    assert_eq!(after.status, TaskStatus::Running);
    assert_eq!(after.claim_token, before.claim_token);
    assert!(
        after.claim_expires_at > before.claim_expires_at,
        "activity should extend the task claim: before={:?} after={:?}",
        before.claim_expires_at,
        after.claim_expires_at
    );
    assert!(
        after.last_heartbeat_at > before.last_heartbeat_at,
        "activity should refresh task heartbeat: before={:?} after={:?}",
        before.last_heartbeat_at,
        after.last_heartbeat_at
    );
    assert!(
        run_last_heartbeat(&temp.path, &claim.run_id)? > before_run_heartbeat,
        "activity should refresh active run heartbeat"
    );
    let heartbeat_events = list_events(&temp.path, "default", Some(&task.id))?
        .into_iter()
        .filter(|event| event.kind == "task.heartbeat")
        .count();
    assert_eq!(heartbeat_events, 0);
    Ok(())
}

#[test]
fn non_running_task_scoped_activity_event_does_not_touch_claim_fields() -> anyhow::Result<()> {
    let temp = TempDb::new("non_running_activity_event_does_not_touch_claim")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("non running activity"),
    )?;
    let before = get_task(&temp.path, "default", &task.id)?;

    thread::sleep(Duration::from_millis(20));
    create_comment(&temp.path, &task.id, "operator", "not running", None)?;

    let after = get_task(&temp.path, "default", &task.id)?;
    assert_eq!(after.status, before.status);
    assert_eq!(after.claim_expires_at, before.claim_expires_at);
    assert_eq!(after.last_heartbeat_at, before.last_heartbeat_at);
    Ok(())
}

#[test]
fn board_level_event_does_not_renew_running_task_claim() -> anyhow::Result<()> {
    let temp = TempDb::new("board_level_event_does_not_renew_running_claim")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("board activity should not renew"),
    )?;
    mark_execution_plan_not_required(&temp.path, "default", "tester", &task.id, "small task")?;
    claim_task(&temp.path, "default", "worker", &task.id, 60_000)?;
    let before = get_task(&temp.path, "default", &task.id)?;

    thread::sleep(Duration::from_millis(20));
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "board-activity".into(),
            color: None,
        },
    )?;

    let after = get_task(&temp.path, "default", &task.id)?;
    assert_eq!(after.claim_expires_at, before.claim_expires_at);
    assert_eq!(after.last_heartbeat_at, before.last_heartbeat_at);
    Ok(())
}

#[test]
fn force_archive_running_task_closes_active_run() -> anyhow::Result<()> {
    let temp = TempDb::new("force_archive_running_task_closes_active_run")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("archive running"),
    )?;
    mark_execution_plan_not_required(&temp.path, "default", "tester", &task.id, "small task")?;
    let claim = claim_task(&temp.path, "default", "worker", &task.id, 300_000)?;

    let archived = archive_task(&temp.path, "default", "tester", &task.id, true)?;

    assert_eq!(archived.status, TaskStatus::Archived);
    assert!(archived.claim_token.is_none());
    assert!(archived.claim_owner.is_none());
    assert!(archived.claim_expires_at.is_none());
    let runs = list_runs(&temp.path, "default", Some(&task.id))?;
    let run = runs
        .iter()
        .find(|run| run.id == claim.run_id)
        .ok_or_else(|| test_error("expected active run to be canceled"))?;
    assert_eq!(run.status, "canceled");
    assert!(run.finished_at.is_some());
    assert!(runs.iter().all(|run| run.status != "running"));
    assert!(
        list_events(&temp.path, "default", Some(&task.id))?
            .iter()
            .any(|event| event.kind == "task.archived"
                && event.run_id.as_deref() == Some(&claim.run_id))
    );
    Ok(())
}

#[test]
fn block_reason_with_control_chars_writes_valid_event_json() -> anyhow::Result<()> {
    let temp = TempDb::new("block_reason_with_control_chars_writes_valid_event_json")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("block control chars"),
    )?;
    let reason = "line one\nline two\tquote \" slash \\ control \u{0008}";

    let blocked = block_task(
        &temp.path, "default", "tester", &task.id, reason, None, false,
    )?;

    assert_eq!(blocked.status, TaskStatus::Blocked);
    assert_eq!(blocked.status_reason.as_deref(), Some(reason));
    let events = list_events(&temp.path, "default", Some(&task.id))?;
    let event = events
        .iter()
        .find(|event| event.kind == "task.blocked")
        .ok_or_else(|| test_error("expected task.blocked event"))?;
    let payload: serde_json::Value = serde_json::from_str(&event.payload_json)?;
    assert_eq!(payload["reason"], reason);
    Ok(())
}

#[test]
fn reopen_done_task_recomputes_target_and_preserves_result() -> anyhow::Result<()> {
    let temp = TempDb::new("reopen_done_task_recomputes_target_and_preserves_result")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(&temp.path, "default", "tester", CreateTask::ready("parent"))?;
    let child = create_task(&temp.path, "default", "tester", CreateTask::ready("child"))?;
    mark_plan_not_required_for_test(&temp.path, "default", "tester", &parent.id)?;
    mark_plan_not_required_for_test(&temp.path, "default", "tester", &child.id)?;
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id)?;

    let claim = claim_task(&temp.path, "default", "worker", &parent.id, 300_000)?;
    let completed = kanban_sqlite::api::complete_task_with_summary_and_result(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        Some(&claim.claim_token),
        false,
        Some("finished once"),
        Some(r#"{"ok":true}"#),
    )?;
    assert_eq!(completed.status, TaskStatus::Done);
    let original_completed_at = completed
        .completed_at
        .ok_or_else(|| test_error("expected completed_at"))?;
    let child = promote_task(&temp.path, "default", "tester", &child.id)?;
    assert_eq!(child.status, TaskStatus::Ready);

    let reopened = reopen_task(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        "retry with fix",
    )?;

    assert_eq!(reopened.status, TaskStatus::Ready);
    assert!(reopened.completed_at.is_none());
    assert_eq!(reopened.result_summary.as_deref(), Some("finished once"));
    assert_eq!(reopened.result_json.as_deref(), Some(r#"{"ok":true}"#));
    let child = get_task(&temp.path, "default", &child.id)?;
    assert_eq!(child.status, TaskStatus::Todo);
    assert!(child.dependency_blocked);
    let events = list_events(&temp.path, "default", Some(&parent.id))?;
    let event = events
        .iter()
        .find(|event| event.kind == "task.reopened")
        .ok_or_else(|| test_error("expected task.reopened event"))?;
    let payload: serde_json::Value = serde_json::from_str(&event.payload_json)?;
    assert_eq!(payload["from"], "done");
    assert_eq!(payload["to"], "ready");
    assert_eq!(payload["reason"], "retry with fix");
    assert_eq!(payload["original_completed_at"], original_completed_at);
    Ok(())
}

#[test]
fn reopen_skips_running_blocked_review_done_and_archived_children() -> anyhow::Result<()> {
    let temp = TempDb::new("reopen_skips_running_blocked_review_done_and_archived_children")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(&temp.path, "default", "tester", CreateTask::ready("parent"))?;
    mark_plan_not_required_for_test(&temp.path, "default", "tester", &parent.id)?;
    let parent_claim = claim_task(&temp.path, "default", "worker", &parent.id, 300_000)?;
    complete_task(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        Some(&parent_claim.claim_token),
        false,
    )?;

    let running = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("running child"),
    )?;
    let blocked = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("blocked child"),
    )?;
    let review = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("review child"),
    )?;
    let done = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("done child"),
    )?;
    let archived = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("archived child"),
    )?;
    for child in [&running, &blocked, &review, &done, &archived] {
        mark_plan_not_required_for_test(&temp.path, "default", "tester", &child.id)?;
        add_dependency(&temp.path, "default", "tester", &parent.id, &child.id)?;
    }
    claim_task(&temp.path, "default", "worker", &running.id, 300_000)?;
    block_task(
        &temp.path,
        "default",
        "tester",
        &blocked.id,
        "waiting",
        None,
        false,
    )?;
    let review_claim = claim_task(&temp.path, "default", "worker", &review.id, 300_000)?;
    submit_review_task(
        &temp.path,
        "default",
        "worker",
        &review.id,
        Some(&review_claim.claim_token),
        false,
    )?;
    let done_claim = claim_task(&temp.path, "default", "worker", &done.id, 300_000)?;
    complete_task(
        &temp.path,
        "default",
        "worker",
        &done.id,
        Some(&done_claim.claim_token),
        false,
    )?;
    archive_task(&temp.path, "default", "tester", &archived.id, false)?;

    reopen_task(&temp.path, "default", "tester", &parent.id, "retry parent")?;

    assert_eq!(
        get_task(&temp.path, "default", &running.id)?.status,
        TaskStatus::Running
    );
    assert_eq!(
        get_task(&temp.path, "default", &blocked.id)?.status,
        TaskStatus::Blocked
    );
    assert_eq!(
        get_task(&temp.path, "default", &review.id)?.status,
        TaskStatus::Review
    );
    assert_eq!(
        get_task(&temp.path, "default", &done.id)?.status,
        TaskStatus::Done
    );
    assert_eq!(
        get_task(&temp.path, "default", &archived.id)?.status,
        TaskStatus::Archived
    );
    Ok(())
}

#[test]
fn reopen_rejects_non_done_and_blank_reason_without_mutation() -> anyhow::Result<()> {
    let temp = TempDb::new("reopen_rejects_non_done_and_blank_reason_without_mutation")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("not done"),
    )?;

    let non_done = result_err(reopen_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        "try anyway",
    ))?;
    assert!(non_done.to_string().contains("reopen requires done"));

    mark_plan_not_required_for_test(&temp.path, "default", "tester", &task.id)?;
    let claim = claim_task(&temp.path, "default", "worker", &task.id, 300_000)?;
    let completed = complete_task(
        &temp.path,
        "default",
        "worker",
        &task.id,
        Some(&claim.claim_token),
        false,
    )?;
    let before = completed.lock_version;
    let blank = result_err(reopen_task(
        &temp.path, "default", "tester", &task.id, "  \t",
    ))?;
    assert!(blank.to_string().contains("reopen reason is required"));
    assert_eq!(
        get_task(&temp.path, "default", &task.id)?.lock_version,
        before
    );
    assert!(
        list_events(&temp.path, "default", Some(&task.id))?
            .iter()
            .all(|event| event.kind != "task.reopened")
    );
    Ok(())
}

#[test]
fn updating_ready_task_to_future_schedule_makes_it_unclaimable_until_due() -> anyhow::Result<()> {
    let temp =
        TempDb::new("updating_ready_task_to_future_schedule_makes_it_unclaimable_until_due")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("future scheduled update"),
    )?;

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            scheduled_at: Some(Some(now_ms() + 3_600_000)),
            ..TaskPatch::default()
        },
    )?;

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
    )?;
    assert_eq!(result.claimed, 0);
    assert_eq!(
        get_task(&temp.path, "default", &task.id)?.status,
        TaskStatus::Scheduled
    );
    Ok(())
}

#[test]
fn clearing_schedule_recomputes_complete_task_without_dependencies_to_ready() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("clearing_schedule_recomputes_complete_task_without_dependencies_to_ready")?;
    init_database(&temp.path, "tester")?;
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
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;

    mark_execution_plan_not_required(&temp.path, "default", "tester", &task.id, "small task")?;
    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            scheduled_at: Some(None),
            ..TaskPatch::default()
        },
    )?;

    assert_eq!(updated.scheduled_at, None);
    assert_eq!(updated.status, TaskStatus::Ready);
    Ok(())
}

#[test]
fn clearing_schedule_recomputes_incomplete_dependencies_to_todo() -> anyhow::Result<()> {
    let temp = TempDb::new("clearing_schedule_recomputes_incomplete_dependencies_to_todo")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("incomplete parent"),
    )?;
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
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id)?;

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &child.id,
        TaskPatch {
            scheduled_at: Some(None),
            ..TaskPatch::default()
        },
    )?;

    assert_eq!(updated.status, TaskStatus::Todo);
    Ok(())
}

#[test]
fn clearing_schedule_recomputes_missing_description_to_triage() -> anyhow::Result<()> {
    let temp = TempDb::new("clearing_schedule_recomputes_missing_description_to_triage")?;
    init_database(&temp.path, "tester")?;
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
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            scheduled_at: Some(None),
            ..TaskPatch::default()
        },
    )?;

    assert_eq!(updated.status, TaskStatus::Triage);
    Ok(())
}

#[test]
fn updating_description_recomputes_active_triage_to_ready() -> anyhow::Result<()> {
    let temp = TempDb::new("updating_description_recomputes_active_triage_to_ready")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "needs spec".into(),
            description: None,
            status: None,
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    assert_eq!(task.status, TaskStatus::Triage);

    mark_execution_plan_not_required(&temp.path, "default", "tester", &task.id, "small task")?;
    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            description: Some(Some("ready spec".into())),
            ..TaskPatch::default()
        },
    )?;

    assert_eq!(updated.status, TaskStatus::Ready);
    Ok(())
}

#[test]
fn updating_description_recomputes_active_ready_to_triage() -> anyhow::Result<()> {
    let temp = TempDb::new("updating_description_recomputes_active_ready_to_triage")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("remove spec"),
    )?;
    mark_execution_plan_not_required(&temp.path, "default", "tester", &task.id, "small task")?;

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            description: Some(None),
            ..TaskPatch::default()
        },
    )?;

    assert_eq!(updated.status, TaskStatus::Triage);
    Ok(())
}

#[test]
fn claimed_task_has_current_run_running_run_and_claimed_event() -> anyhow::Result<()> {
    let temp = TempDb::new("claimed_task_has_current_run_running_run_and_claimed_event")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("claim invariant"),
    )?;
    mark_execution_plan_not_required(&temp.path, "default", "tester", &task.id, "small task")?;

    let claim = claim_task(&temp.path, "default", "worker", &task.id, 300_000)?;
    let claimed = get_task(&temp.path, "default", &task.id)?;
    assert_eq!(claimed.status, TaskStatus::Running);
    assert_eq!(
        claimed.current_run_id.as_deref(),
        Some(claim.run_id.as_str())
    );

    let running_runs = list_runs(&temp.path, "default", Some(&task.id))?
        .into_iter()
        .filter(|run| run.status == "running")
        .collect::<Vec<_>>();
    assert_eq!(running_runs.len(), 1);
    assert_eq!(running_runs[0].id, claim.run_id);

    let events = list_events(&temp.path, "default", Some(&task.id))?;
    assert!(
        events.iter().any(|event| event.kind == "task.claimed"
            && event.run_id.as_deref() == claimed.current_run_id.as_deref()),
        "events: {events:?}"
    );
    Ok(())
}

#[test]
fn heartbeat_against_stale_non_running_claim_fails_without_touching_heartbeat_fields()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "heartbeat_against_stale_non_running_claim_fails_without_touching_heartbeat_fields",
    )?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("stale hb"),
    )?;
    mark_execution_plan_not_required(&temp.path, "default", "tester", &task.id, "small task")?;
    let claim = claim_task(&temp.path, "default", "worker", &task.id, 300_000)?;
    block_task(
        &temp.path,
        "default",
        "human",
        &task.id,
        "manual block",
        None,
        true,
    )?;
    let blocked = get_task(&temp.path, "default", &task.id)?;

    let err = result_err(kanban_sqlite::api::heartbeat_task(
        &temp.path,
        "default",
        "worker",
        &task.id,
        &claim.claim_token,
        600_000,
    ))?;
    assert!(err.to_string().contains("matching running claim"));

    let after = get_task(&temp.path, "default", &task.id)?;
    assert_eq!(after.status, TaskStatus::Blocked);
    assert_eq!(after.claim_expires_at, blocked.claim_expires_at);
    assert_eq!(after.last_heartbeat_at, blocked.last_heartbeat_at);
    Ok(())
}

#[test]
fn execution_plan_required_blocks_promote_claim_and_dispatch_until_planned() -> anyhow::Result<()> {
    let temp = TempDb::new("execution_plan_required_blocks_executable_paths")?;
    init_database(&temp.path, "tester")?;
    let todo = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "needs plan".into(),
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

    let promote_err = result_err(promote_task(&temp.path, "default", "tester", &todo.id))?;
    assert!(
        matches!(promote_err, KanbanError::ExecutionPlanRequired(_)),
        "{promote_err}"
    );

    mark_execution_plan_not_required(
        &temp.path,
        "default",
        "tester",
        &todo.id,
        "single-step task",
    )?;
    let ready = get_task(&temp.path, "default", &todo.id)?;
    assert_eq!(ready.status, TaskStatus::Ready);
    assert_eq!(ready.execution_plan_state, StepPlanState::NotRequired);
    assert_eq!(ready.required_step_count, 0);

    let unplanned_ready = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("unplanned ready"),
    )?;
    assert_eq!(unplanned_ready.status, TaskStatus::Todo);
    let conn = connect_file(&temp.path)?;
    conn.execute(
        "UPDATE tasks SET status='ready' WHERE id=?1",
        params![unplanned_ready.id],
    )?;
    let claim_err = result_err(claim_task(
        &temp.path,
        "default",
        "worker",
        &unplanned_ready.id,
        300_000,
    ))?;
    assert!(
        matches!(claim_err, KanbanError::ExecutionPlanRequired(_)),
        "{claim_err}"
    );

    let result = dispatch_once(
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
            log_dir: temp.dir.join("logs"),
        },
    )?;
    assert_eq!(
        result.claimed, 1,
        "planned ready task should still be claimable"
    );
    assert_eq!(
        get_task(&temp.path, "default", &unplanned_ready.id)?.status,
        TaskStatus::Ready
    );

    let triage = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "specify without plan".into(),
            description: None,
            status: None,
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    let specified = specify_task(
        &temp.path,
        "tester",
        &triage.id,
        Some("ready spec".into()),
        None,
    )?;
    assert_eq!(specified.status, TaskStatus::Todo);

    let blocked = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "blocked without plan".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    block_task(
        &temp.path,
        "default",
        "tester",
        &blocked.id,
        "waiting",
        None,
        false,
    )?;
    let unblocked = unblock_task(&temp.path, "default", "tester", &blocked.id)?;
    assert_eq!(unblocked.status, TaskStatus::Todo);

    let reclaim_target = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("reclaim loses plan"),
    )?;
    mark_execution_plan_not_required(
        &temp.path,
        "default",
        "tester",
        &reclaim_target.id,
        "small task",
    )?;
    let reclaim_claim = claim_task(&temp.path, "default", "worker", &reclaim_target.id, 300_000)?;
    let conn = connect_file(&temp.path)?;
    conn.execute(
        "DELETE FROM task_execution_plans WHERE task_id=?1",
        params![reclaim_target.id],
    )?;
    let reclaimed = kanban_sqlite::api::reclaim_task(
        &temp.path,
        "default",
        "tester",
        &reclaim_claim.task.id,
        true,
    )?;
    assert_eq!(reclaimed.status, TaskStatus::Todo);
    Ok(())
}

#[test]
fn removing_last_step_demotes_ready_parent_to_todo() -> anyhow::Result<()> {
    let temp = TempDb::new("removing_last_step_demotes_ready_parent_to_todo")?;
    init_database(&temp.path, "tester")?;

    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("parent required toggle"),
    )?;
    let step = create_step(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        CreateStepInput {
            title: "only step".into(),
            body: None,
            linked_task_ref: None,
            position: None,
            required: true,
        },
    )?;
    assert_eq!(
        get_task(&temp.path, "default", &parent.id)?.status,
        TaskStatus::Ready
    );

    remove_step(&temp.path, "default", "tester", &parent.id, &step.id)?;
    let detached = get_task(&temp.path, "default", &parent.id)?;
    assert_eq!(detached.execution_plan_state, StepPlanState::Unplanned);
    assert_eq!(detached.status, TaskStatus::Todo);
    Ok(())
}

#[test]
fn required_steps_gate_complete_and_archive_parent() -> anyhow::Result<()> {
    let temp = TempDb::new("required_steps_gate_complete_and_archive_parent")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(&temp.path, "default", "tester", CreateTask::ready("parent"))?;
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "linked task context".into(),
            description: Some("task context, not completion state".into()),
            status: Some(TaskStatus::Todo),
            assignee: None,
            priority: 1,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    let step = create_step(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        CreateStepInput {
            title: "required verification".into(),
            body: None,
            linked_task_ref: Some(child.id.clone()),
            position: None,
            required: true,
        },
    )?;
    let planned = get_task(&temp.path, "default", &parent.id)?;
    assert_eq!(planned.execution_plan_state, StepPlanState::Planned);
    assert_eq!(planned.required_step_count, 1);
    assert_eq!(planned.completed_required_step_count, 0);

    let archive_err = result_err(archive_task(
        &temp.path, "default", "tester", &parent.id, false,
    ))?;
    assert!(
        matches!(archive_err, KanbanError::StepsIncomplete(_)),
        "{archive_err}"
    );

    let claim = claim_task(&temp.path, "default", "worker", &parent.id, 300_000)?;
    let complete_err = result_err(complete_task(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        Some(&claim.claim_token),
        false,
    ))?;
    assert!(
        matches!(complete_err, KanbanError::StepsIncomplete(_)),
        "{complete_err}"
    );

    archive_task(&temp.path, "default", "tester", &child.id, false)?;
    let still_blocked = result_err(complete_task(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        Some(&claim.claim_token),
        false,
    ))?;
    assert!(
        matches!(still_blocked, KanbanError::StepsIncomplete(_)),
        "{still_blocked}"
    );

    complete_step(
        &temp.path,
        "default",
        "tester",
        &parent.id,
        &step.id,
        "step verified",
    )?;
    let completed = complete_task(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        Some(&claim.claim_token),
        false,
    )?;
    assert_eq!(completed.status, TaskStatus::Done);
    assert_eq!(completed.completed_required_step_count, 1);

    let force_parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("force parent"),
    )?;
    create_step(
        &temp.path,
        "default",
        "tester",
        &force_parent.id,
        CreateStepInput {
            title: "active required step".into(),
            body: None,
            linked_task_ref: None,
            position: None,
            required: true,
        },
    )?;
    let archived = archive_task(&temp.path, "default", "tester", &force_parent.id, true)?;
    assert_eq!(archived.status, TaskStatus::Archived);
    Ok(())
}

fn run_last_heartbeat(path: &Path, run_id: &str) -> anyhow::Result<i64> {
    connect_file(path)?
        .query_row(
            "SELECT last_heartbeat_at FROM task_runs WHERE id=?1",
            params![run_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
}
