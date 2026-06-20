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

#[derive(Debug, PartialEq, Eq)]
struct TaskCreateLabelCounts {
    tasks: i64,
    labels: i64,
    task_labels: i64,
    task_events: i64,
}

fn task_create_label_counts(path: &Path) -> anyhow::Result<TaskCreateLabelCounts> {
    let conn = connect_file(path)?;
    Ok(TaskCreateLabelCounts {
        tasks: count_rows(&conn, "tasks")?,
        labels: count_rows(&conn, "labels")?,
        task_labels: count_rows(&conn, "task_labels")?,
        task_events: count_rows(&conn, "task_events")?,
    })
}

fn count_rows(conn: &Connection, table: &str) -> anyhow::Result<i64> {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .context("count rows")
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
            labels: vec![],
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
            labels: vec![],
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

#[test]
fn task_create_with_missing_label_rolls_back_task_vocabulary_bindings_and_events()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "task_create_with_missing_label_rolls_back_task_vocabulary_bindings_and_events",
    )?;
    init_database(&temp.path, "tester")?;
    let before = task_create_label_counts(&temp.path)?;

    let error = result_err(kanban_sqlite::create_task_with_labels(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Missing label task"),
        &["missing".to_owned()],
    ))?;

    assert!(error.to_string().contains("label missing does not exist"));
    assert_eq!(task_create_label_counts(&temp.path)?, before);
    assert!(list_tasks(&temp.path, "default", &[], false)?.is_empty());
    assert!(kanban_sqlite::list_labels(&temp.path, "default")?.is_empty());
    Ok(())
}

#[test]
fn task_create_with_mixed_existing_and_missing_labels_rolls_back_atomically() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("task_create_with_mixed_existing_and_missing_labels_rolls_back_atomically")?;
    init_database(&temp.path, "tester")?;
    let backend = create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let before = task_create_label_counts(&temp.path)?;

    let error = result_err(kanban_sqlite::create_task_with_labels(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Partially labeled task"),
        &["backend".to_owned(), "missing".to_owned()],
    ))?;

    assert!(error.to_string().contains("label missing does not exist"));
    assert_eq!(task_create_label_counts(&temp.path)?, before);
    assert_eq!(
        kanban_sqlite::list_labels(&temp.path, "default")?
            .into_iter()
            .map(|label| label.id)
            .collect::<Vec<_>>(),
        [backend.id]
    );
    assert!(list_tasks(&temp.path, "default", &[], false)?.is_empty());
    Ok(())
}

#[test]
fn task_create_with_existing_label_binds_without_creating_vocabulary() -> anyhow::Result<()> {
    let temp = TempDb::new("task_create_with_existing_label_binds_without_creating_vocabulary")?;
    init_database(&temp.path, "tester")?;
    let backend = create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;

    let task = kanban_sqlite::create_task_with_labels(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Existing label task"),
        &["backend".to_owned()],
    )?;

    assert_eq!(task.labels.len(), 1);
    assert_eq!(task.labels[0].id, backend.id);
    assert_eq!(kanban_sqlite::list_labels(&temp.path, "default")?.len(), 1);
    let events = list_events(&temp.path, "default", Some(&task.id))?;
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == "task.label.added")
            .count(),
        1
    );
    assert!(
        events
            .iter()
            .all(|event| matches!(event.kind.as_str(), "task.created" | "task.label.added")),
        "{events:?}"
    );
    Ok(())
}

#[test]
fn labels_create_attach_filter_and_remove_without_status_side_effects() -> anyhow::Result<()> {
    let temp = TempDb::new("labels_create_attach_filter_and_remove_without_status_side_effects")?;
    init_database(&temp.path, "tester")?;

    let ready = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Ready task"),
    )?;
    let todo = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Todo task".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Todo),
            assignee: None,
            priority: 3,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;

    let backend = kanban_sqlite::create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".into(),
            color: Some("#336699".into()),
        },
    )?;
    assert_eq!(backend.name, "backend");
    assert_eq!(
        kanban_sqlite::create_label(
            &temp.path,
            "default",
            kanban_sqlite::CreateLabel {
                name: "backend".into(),
                color: None,
            },
        )?
        .id,
        backend.id
    );

    let labeled =
        kanban_sqlite::add_task_label(&temp.path, "default", "tester", &ready.id, "backend")?;
    assert_eq!(labeled.status, TaskStatus::Ready);
    assert_eq!(
        labeled
            .labels
            .iter()
            .map(|label| label.name.as_str())
            .collect::<Vec<_>>(),
        ["backend"]
    );

    let duplicate =
        kanban_sqlite::add_task_label(&temp.path, "default", "tester", &ready.id, "backend")?;
    assert_eq!(duplicate.labels.len(), 1);

    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "frontend".into(),
            color: None,
        },
    )?;
    let labeled_todo =
        kanban_sqlite::add_task_label(&temp.path, "default", "tester", &todo.id, "frontend")?;
    assert_eq!(labeled_todo.status, TaskStatus::Todo);
    assert_eq!(labeled_todo.labels[0].name, "frontend");

    let page = kanban_sqlite::list_tasks_page(
        &temp.path,
        "default",
        kanban_sqlite::TaskListOptions {
            statuses: vec![],
            priorities: vec![],
            labels: vec!["backend".into()],
            include_archived: false,
            assignee: None,
            search: None,
            sort: kanban_sqlite::TaskListSort::Seq,
            limit: 100,
            offset: 0,
        },
    )?;
    assert_eq!(page.total, 1);
    assert_eq!(page.tasks[0].id, ready.id);
    assert_eq!(page.tasks[0].labels[0].id, backend.id);

    let removed =
        kanban_sqlite::remove_task_label(&temp.path, "default", "tester", &ready.id, "backend")?;
    assert_eq!(removed.status, TaskStatus::Ready);
    assert!(removed.labels.is_empty());

    Ok(())
}

#[test]
fn task_label_batch_add_normalizes_dedups_and_events_new_bindings_only() -> anyhow::Result<()> {
    let temp = TempDb::new("task_label_batch_add_normalizes_dedups_and_events_new_bindings_only")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Batch label target"),
    )?;
    for name in ["backend", "frontend", "api"] {
        create_label(
            &temp.path,
            "default",
            CreateLabel {
                name: name.to_owned(),
                color: None,
            },
        )?;
    }

    let first = kanban_sqlite::add_task_labels(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &["backend".into(), " frontend ".into(), "backend".into()],
    )?;
    assert_eq!(
        first
            .labels
            .iter()
            .map(|label| label.name.as_str())
            .collect::<Vec<_>>(),
        ["backend", "frontend"]
    );
    let added_events = list_events(&temp.path, "default", Some(&task.id))?
        .into_iter()
        .filter(|event| event.kind == "task.label.added")
        .collect::<Vec<_>>();
    assert_eq!(added_events.len(), 2);

    let second = kanban_sqlite::add_task_labels(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &["frontend".into(), "api".into()],
    )?;
    assert_eq!(
        second
            .labels
            .iter()
            .map(|label| label.name.as_str())
            .collect::<Vec<_>>(),
        ["api", "backend", "frontend"]
    );
    let added_events = list_events(&temp.path, "default", Some(&task.id))?
        .into_iter()
        .filter(|event| event.kind == "task.label.added")
        .collect::<Vec<_>>();
    assert_eq!(added_events.len(), 3);

    Ok(())
}

#[test]
fn task_label_add_requires_existing_label_unless_create_missing() -> anyhow::Result<()> {
    let temp = TempDb::new("task_label_add_requires_existing_label_unless_create_missing")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Vocabulary guard target"),
    )?;

    let error = result_err(kanban_sqlite::add_task_labels(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &["backend-typo".into()],
    ))?;
    assert!(
        error
            .to_string()
            .contains("label backend-typo does not exist")
    );
    assert!(get_task(&temp.path, "default", &task.id)?.labels.is_empty());
    assert!(kanban_sqlite::list_labels(&temp.path, "default")?.is_empty());

    let created = kanban_sqlite::add_task_labels_with_options(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &["backend-typo".into()],
        true,
    )?;
    assert_eq!(created.created_labels.len(), 1);
    assert_eq!(created.created_labels[0].name, "backend-typo");
    assert_eq!(created.task.labels[0].name, "backend-typo");
    assert_eq!(kanban_sqlite::list_labels(&temp.path, "default")?.len(), 1);

    let repeated = kanban_sqlite::add_task_labels_with_options(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &["backend-typo".into()],
        true,
    )?;
    assert!(repeated.created_labels.is_empty());
    assert_eq!(repeated.task.labels.len(), 1);

    Ok(())
}

#[test]
fn task_label_batch_add_validates_before_mutating() -> anyhow::Result<()> {
    let temp = TempDb::new("task_label_batch_add_validates_before_mutating")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Invalid batch label target"),
    )?;

    let error = result_err(kanban_sqlite::add_task_labels(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &["backend".into(), " ".into()],
    ))?;
    assert!(error.to_string().contains("label name is required"));
    assert!(get_task(&temp.path, "default", &task.id)?.labels.is_empty());
    assert!(kanban_sqlite::list_labels(&temp.path, "default")?.is_empty());

    Ok(())
}

#[test]
fn task_label_mutations_by_id_use_task_board_and_reject_archived_targets() -> anyhow::Result<()> {
    let temp =
        TempDb::new("task_label_mutations_by_id_use_task_board_and_reject_archived_targets")?;
    init_database(&temp.path, "tester")?;
    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "other".into(),
            name: "Other".into(),
            description: None,
        },
    )?;

    let other_task = create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("Other board label target"),
    )?;
    create_label(
        &temp.path,
        "other",
        CreateLabel {
            name: "backend".into(),
            color: None,
        },
    )?;
    let labeled =
        kanban_sqlite::add_task_label_by_id(&temp.path, "tester", &other_task.id, "backend")?;
    assert_eq!(labeled.board_slug, "other");
    assert_eq!(labeled.labels[0].name, "backend");
    assert!(kanban_sqlite::list_labels(&temp.path, "default")?.is_empty());
    assert_eq!(
        kanban_sqlite::list_labels(&temp.path, "other")?[0].name,
        "backend"
    );

    let removed =
        kanban_sqlite::remove_task_label_by_id(&temp.path, "tester", &other_task.id, "backend")?;
    assert!(removed.labels.is_empty());

    let archived_task = create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("Archived label target"),
    )?;
    archive_task(&temp.path, "other", "tester", &archived_task.id, false)?;
    let archived_task_error = result_err(kanban_sqlite::add_task_label_by_id(
        &temp.path,
        "tester",
        &archived_task.id,
        "blocked",
    ))?;
    assert!(archived_task_error.to_string().contains("task "));

    archive_board(&temp.path, "other", "tester")?;
    let archived_board_error = result_err(kanban_sqlite::add_task_label_by_id(
        &temp.path,
        "tester",
        &other_task.id,
        "blocked",
    ))?;
    assert!(archived_board_error.to_string().contains("task "));

    Ok(())
}

#[test]
fn task_label_mutations_by_ref_reject_archived_tasks() -> anyhow::Result<()> {
    let temp = TempDb::new("task_label_mutations_by_ref_reject_archived_tasks")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "backend".into(),
            color: None,
        },
    )?;

    let add_target = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Archived add label target"),
    )?;
    archive_task(&temp.path, "default", "tester", &add_target.id, false)?;
    let add_error = result_err(kanban_sqlite::add_task_label(
        &temp.path,
        "default",
        "tester",
        &add_target.task_ref,
        "backend",
    ))?;
    assert!(add_error.to_string().contains("task "));

    let remove_target = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Archived remove label target"),
    )?;
    kanban_sqlite::add_task_label(
        &temp.path,
        "default",
        "tester",
        &remove_target.task_ref,
        "backend",
    )?;
    archive_task(&temp.path, "default", "tester", &remove_target.id, false)?;
    let remove_error = result_err(kanban_sqlite::remove_task_label(
        &temp.path,
        "default",
        "tester",
        &remove_target.task_ref,
        "backend",
    ))?;
    assert!(remove_error.to_string().contains("task "));

    Ok(())
}

#[test]
fn label_ref_resolution_prefers_exact_l_prefixed_name_before_id() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ref_resolution_prefers_exact_l_prefixed_name_before_id")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Reserved-looking label target"),
    )?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "l_bug".into(),
            color: None,
        },
    )?;

    let labeled =
        kanban_sqlite::add_task_label(&temp.path, "default", "tester", &task.id, "l_bug")?;
    assert_eq!(labeled.labels[0].name, "l_bug");

    let removed =
        kanban_sqlite::remove_task_label(&temp.path, "default", "tester", &task.id, "l_bug")?;
    assert!(removed.labels.is_empty());
    assert_eq!(
        kanban_sqlite::list_labels(&temp.path, "default")?[0].name,
        "l_bug"
    );

    Ok(())
}

#[test]
fn labels_reject_blank_names() -> anyhow::Result<()> {
    let temp = TempDb::new("labels_reject_blank_names")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Label target"),
    )?;

    let create_error = result_err(kanban_sqlite::create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "  ".into(),
            color: None,
        },
    ))?;
    assert!(create_error.to_string().contains("label name is required"));

    let attach_error = result_err(kanban_sqlite::add_task_label(
        &temp.path, "default", "tester", &task.id, "",
    ))?;
    assert!(attach_error.to_string().contains("label name is required"));

    Ok(())
}
