use crate::common::*;

#[test]
fn block_rolls_back_task_state_when_event_insert_fails() -> anyhow::Result<()> {
    let temp = TempDb::new("block_rolls_back_task_state_when_event_insert_fails")?;
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
    Ok(())
}

#[test]
fn update_rolls_back_task_state_when_event_insert_fails() -> anyhow::Result<()> {
    let temp = TempDb::new("update_rolls_back_task_state_when_event_insert_fails")?;
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
    Ok(())
}
