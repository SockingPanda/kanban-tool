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
    assert_eq!(unchanged.status, TaskStatus::Ready);
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
    let claim = claim_task(&temp.path, "default", "worker", &task.id, 300_000)?;

    for ttl_ms in [0, -1] {
        let err = result_err(kanban_sqlite::heartbeat_task(
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
fn force_archive_running_task_closes_active_run() -> anyhow::Result<()> {
    let temp = TempDb::new("force_archive_running_task_closes_active_run")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("archive running"),
    )?;
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

    let err = result_err(kanban_sqlite::heartbeat_task(
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
