use crate::common::*;

#[test]
fn task_crud_writes_events_and_hides_archived_by_default() -> anyhow::Result<()> {
    let temp = TempDb::new("task_crud_writes_events_and_hides_archived_by_default")?;
    init_database(&temp.path, "tester")?;

    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "实现 Task CRUD".into(),
            description: Some("规格".into()),
            status: None,
            assignee: None,
            priority: 1,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;

    assert_eq!(task.seq, 1);
    assert_eq!(task.status, TaskStatus::Ready);
    assert_eq!(
        list_events(&temp.path, "default", Some(&task.id))?[0].kind,
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
            priority: Some(2),
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: None,
            expected_lock_version: Some(task.lock_version),
        },
    )?;
    assert_eq!(updated.title, "实现 Task CRUD v0.5");
    assert_eq!(updated.lock_version, task.lock_version + 1);

    archive_task(&temp.path, "default", "tester", &task.id, false)?;
    assert!(list_tasks(&temp.path, "default", &[], false)?.is_empty());
    assert_eq!(list_tasks(&temp.path, "default", &[], true)?.len(), 1);
    Ok(())
}

#[test]
fn task_create_with_invalid_max_retries_rolls_back_task_and_events() -> anyhow::Result<()> {
    let temp = TempDb::new("task_create_with_invalid_max_retries_rolls_back_task_and_events")?;
    init_database(&temp.path, "tester")?;

    let error = result_err(create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Invalid retry policy".into(),
            description: Some("ready spec".into()),
            status: None,
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: Some(0),
            metadata_json: "{}".into(),
        },
    ))?;
    assert!(error.to_string().contains("max_retries"));
    assert!(list_tasks(&temp.path, "default", &[], false)?.is_empty());
    assert!(
        list_events(&temp.path, "default", None)?
            .iter()
            .all(|event| event.kind != "task.created")
    );

    Ok(())
}

#[test]
fn task_create_and_update_reject_invalid_priority() -> anyhow::Result<()> {
    let temp = TempDb::new("task_create_and_update_reject_invalid_priority")?;
    init_database(&temp.path, "tester")?;

    let create_error = result_err(create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Invalid priority".into(),
            description: Some("ready spec".into()),
            status: None,
            assignee: None,
            priority: 70,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    ))?;
    assert!(
        create_error
            .to_string()
            .contains("priority must be one of P0, P1, P2, P3")
    );

    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Valid priority"),
    )?;
    let update_error = result_err(update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            priority: Some(-1),
            ..TaskPatch::default()
        },
    ))?;
    assert!(
        update_error
            .to_string()
            .contains("priority must be one of P0, P1, P2, P3")
    );
    assert_eq!(get_task(&temp.path, "default", &task.id)?.priority, 3);

    Ok(())
}

#[test]
fn task_update_with_invalid_max_retries_rolls_back_task_and_events() -> anyhow::Result<()> {
    let temp = TempDb::new("task_update_with_invalid_max_retries_rolls_back_task_and_events")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Original title".into(),
            description: Some("ready spec".into()),
            status: None,
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: Some(2),
            metadata_json: "{}".into(),
        },
    )?;
    let events_before = list_events(&temp.path, "default", Some(&task.id))?.len();

    let error = result_err(update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            title: Some("Should roll back".into()),
            description: None,
            assignee: None,
            priority: None,
            scheduled_at: None,
            due_at: None,
            max_retries: Some(Some(0)),
            metadata_json: None,
            expected_lock_version: Some(task.lock_version),
        },
    ))?;
    assert!(error.to_string().contains("max_retries"));

    let unchanged = get_task(&temp.path, "default", &task.id)?;
    assert_eq!(unchanged.title, task.title);
    assert_eq!(unchanged.max_retries, task.max_retries);
    assert_eq!(unchanged.lock_version, task.lock_version);
    assert_eq!(
        list_events(&temp.path, "default", Some(&task.id))?.len(),
        events_before
    );

    Ok(())
}

#[test]
fn task_update_description_preserves_explicit_todo_status() -> anyhow::Result<()> {
    let temp = TempDb::new("task_update_description_preserves_explicit_todo_status")?;
    init_database(&temp.path, "tester")?;

    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Plan rollout".into(),
            description: None,
            status: Some(TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    assert_eq!(task.status, TaskStatus::Todo);

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            title: None,
            description: Some(Some(
                "Detailed spec: keep this task in planning until it is explicitly promoted.".into(),
            )),
            assignee: None,
            priority: None,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: None,
            expected_lock_version: None,
        },
    )?;

    assert_eq!(
        updated.description.as_deref(),
        Some("Detailed spec: keep this task in planning until it is explicitly promoted.")
    );
    assert_eq!(updated.status, TaskStatus::Todo);
    Ok(())
}

#[test]
fn task_list_page_filters_priorities_and_sorts_by_table_fields() -> anyhow::Result<()> {
    let temp = TempDb::new("task_list_page_filters_priorities_and_sorts_by_table_fields")?;
    init_database(&temp.path, "tester")?;

    for (title, priority, assignee) in [
        ("bravo", 2, Some("worker-b")),
        ("alpha", 0, Some("worker-a")),
        ("charlie", 3, None),
    ] {
        create_task(
            &temp.path,
            "default",
            "tester",
            CreateTask {
                title: title.into(),
                description: Some("ready spec".into()),
                status: Some(TaskStatus::Ready),
                assignee: assignee.map(str::to_owned),
                priority,
                scheduled_at: None,
                due_at: None,
                max_retries: None,
                metadata_json: "{}".into(),
            },
        )?;
    }

    let page = kanban_sqlite::list_tasks_page(
        &temp.path,
        "default",
        kanban_sqlite::TaskListOptions {
            statuses: vec![],
            priorities: vec![0, 2],
            include_archived: false,
            assignee: None,
            search: None,
            sort: kanban_sqlite::TaskListSort::Title,
            limit: 100,
            offset: 0,
        },
    )?;

    assert_eq!(page.total, 2);
    assert_eq!(
        page.tasks
            .iter()
            .map(|task| task.title.as_str())
            .collect::<Vec<_>>(),
        ["alpha", "bravo"]
    );

    let page = kanban_sqlite::list_tasks_page(
        &temp.path,
        "default",
        kanban_sqlite::TaskListOptions {
            statuses: vec![],
            priorities: vec![],
            include_archived: false,
            assignee: None,
            search: None,
            sort: kanban_sqlite::TaskListSort::AssigneeDesc,
            limit: 100,
            offset: 0,
        },
    )?;
    assert_eq!(
        page.tasks
            .iter()
            .map(|task| task.assignee.as_deref())
            .collect::<Vec<_>>(),
        [Some("worker-b"), Some("worker-a"), None]
    );

    Ok(())
}
