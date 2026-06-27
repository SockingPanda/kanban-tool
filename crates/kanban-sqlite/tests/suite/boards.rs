use crate::common::*;

#[test]
fn create_board_adds_default_columns_and_created_event() -> anyhow::Result<()> {
    let temp = TempDb::new("create_board_adds_default_columns_and_created_event")?;
    init_database(&temp.path, "tester")?;

    let board = create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "project.alpha".into(),
            name: "Project Alpha".into(),
            description: Some("local project board".into()),
        },
    )?;

    assert!(board.id.starts_with("b_"));
    assert_eq!(board.slug, "project.alpha");
    assert_eq!(board.name, "Project Alpha");
    assert_eq!(board.description.as_deref(), Some("local project board"));
    assert!(board.archived_at.is_none());

    let columns = list_board_columns(&temp.path, "project.alpha")?;
    assert_eq!(columns.len(), 9);
    assert_eq!(columns[0].status, TaskStatus::Triage);
    assert_eq!(columns[8].status, TaskStatus::Archived);
    assert!(columns[8].hidden);

    let events = list_events(&temp.path, "project.alpha", None)?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "board.created");
    Ok(())
}

#[test]
fn board_slug_validation_rejects_reserved_prefixes_and_uppercase() -> anyhow::Result<()> {
    let temp = TempDb::new("board_slug_validation_rejects_reserved_prefixes_and_uppercase")?;
    init_database(&temp.path, "tester")?;

    for slug in ["Bad", "t_work", "b_work", "col_work", ""] {
        let error = result_err(create_board(
            &temp.path,
            "tester",
            CreateBoard {
                slug: slug.into(),
                name: "Bad Board".into(),
                description: None,
            },
        ))?;
        assert!(
            error.to_string().contains("invalid board slug")
                || error.to_string().contains("slug is required"),
            "{slug}: {error}"
        );
    }
    Ok(())
}

#[test]
fn duplicate_board_slug_returns_invalid_input() -> anyhow::Result<()> {
    let temp = TempDb::new("duplicate_board_slug_returns_invalid_input")?;
    init_database(&temp.path, "tester")?;

    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "project".into(),
            name: "Project".into(),
            description: None,
        },
    )?;

    let error = result_err(create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "project".into(),
            name: "Duplicate Project".into(),
            description: None,
        },
    ))?;

    assert!(matches!(error, KanbanError::InvalidInput(_)));
    assert!(error.to_string().contains("board slug already exists"));
    Ok(())
}

#[test]
fn archived_board_is_hidden_by_default_and_rejects_task_writes() -> anyhow::Result<()> {
    let temp = TempDb::new("archived_board_is_hidden_by_default_and_rejects_task_writes")?;
    init_database(&temp.path, "tester")?;
    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "archivable".into(),
            name: "Archivable".into(),
            description: None,
        },
    )?;

    let archived = archive_board(&temp.path, "archivable", "tester")?;
    assert!(archived.archived_at.is_some());

    let active = list_boards(&temp.path, BoardListOptions::default())?;
    assert!(!active.iter().any(|board| board.slug == "archivable"));
    let all = list_boards(
        &temp.path,
        BoardListOptions {
            include_archived: true,
        },
    )?;
    assert!(all.iter().any(|board| board.slug == "archivable"));
    assert!(get_board(&temp.path, "archivable").is_err());

    let error = result_err(create_task(
        &temp.path,
        "archivable",
        "tester",
        CreateTask::ready("nope"),
    ))?;
    assert!(error.to_string().contains("board archivable"));

    let events = list_events(&temp.path, "archivable", None)?;
    let last_event = events
        .last()
        .ok_or_else(|| test_error("expected board archive event"))?;
    assert_eq!(last_event.kind, "board.archived");
    Ok(())
}

#[test]
fn board_archive_rejects_running_work() -> anyhow::Result<()> {
    let temp = TempDb::new("board_archive_rejects_running_work")?;
    init_database(&temp.path, "tester")?;
    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "busy".into(),
            name: "Busy".into(),
            description: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "busy",
        "tester",
        CreateTask::ready("running task"),
    )?;
    mark_plan_not_required_for_test(&temp.path, "busy", "tester", &task.id)?;
    claim_task(&temp.path, "busy", "runner", &task.id, 60_000)?;

    let error = result_err(archive_board(&temp.path, "busy", "tester"))?;
    assert!(matches!(error, KanbanError::InvalidTransition(_)));
    assert!(error.to_string().contains("running work"));
    assert!(get_board(&temp.path, "busy")?.archived_at.is_none());
    Ok(())
}

#[test]
fn archived_board_keeps_read_only_history_inspectable() -> anyhow::Result<()> {
    let temp = TempDb::new("archived_board_keeps_read_only_history_inspectable")?;
    init_database(&temp.path, "tester")?;
    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "archivable".into(),
            name: "Archivable".into(),
            description: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "archivable",
        "tester",
        CreateTask::ready("history task"),
    )?;
    mark_plan_not_required_for_test(&temp.path, "archivable", "tester", &task.id)?;
    create_comment(&temp.path, &task.id, "tester", "history note", None)?;
    let claim = claim_task(&temp.path, "archivable", "runner", &task.id, 60_000)?;
    complete_task(
        &temp.path,
        "archivable",
        "runner",
        &task.id,
        Some(&claim.claim_token),
        false,
    )?;

    archive_board(&temp.path, "archivable", "tester")?;
    let create_after_archive = result_err(create_comment(
        &temp.path,
        &task.id,
        "tester",
        "late write",
        None,
    ))?;
    assert!(matches!(create_after_archive, KanbanError::NotFound(_)));
    let specify_after_archive = result_err(specify_task(
        &temp.path,
        "tester",
        &task.id,
        Some("late spec".into()),
        None,
    ))?;
    assert!(matches!(specify_after_archive, KanbanError::NotFound(_)));
    let retry_after_archive = result_err(set_task_retry_policy_by_id(
        &temp.path,
        "tester",
        &task.id,
        Some(2),
    ))?;
    assert!(matches!(retry_after_archive, KanbanError::NotFound(_)));

    let qualified_ref = "archivable#1";
    let events = list_events(&temp.path, "archivable", Some(qualified_ref))?;
    assert!(events.iter().any(|event| event.kind == "task.created"));
    assert!(
        events
            .iter()
            .any(|event| event.kind == "task.comment.created")
    );

    let runs = list_runs(&temp.path, "archivable", Some(qualified_ref))?;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].task_id, task.id);

    let comments = list_comments(&temp.path, qualified_ref)?;
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].body, "history note");
    Ok(())
}

#[test]
fn cross_board_dependencies_are_rejected_even_with_global_refs() -> anyhow::Result<()> {
    let temp = TempDb::new("cross_board_dependencies_are_rejected_even_with_global_refs")?;
    init_database(&temp.path, "tester")?;
    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "project".into(),
            name: "Project".into(),
            description: None,
        },
    )?;
    let default_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default task"),
    )?;
    let project_task = create_task(
        &temp.path,
        "project",
        "tester",
        CreateTask::ready("project task"),
    )?;

    let error = result_err(add_dependency(
        &temp.path,
        "project",
        "tester",
        &default_task.id,
        &project_task.id,
    ))?;
    assert!(error.to_string().contains("cross-board dependency"));
    Ok(())
}

#[test]
fn task_refs_resolve_board_seq_and_board_slug_prefixes() -> anyhow::Result<()> {
    let temp = TempDb::new("task_refs_resolve_board_seq_and_board_slug_prefixes")?;
    init_database(&temp.path, "tester")?;
    let board = create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "project".into(),
            name: "Project".into(),
            description: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "project",
        "tester",
        CreateTask::ready("project task"),
    )?;

    assert_eq!(get_task(&temp.path, "project", "1")?.id, task.id);
    assert_eq!(get_task(&temp.path, "project", "#1")?.id, task.id);
    assert_eq!(get_task(&temp.path, "default", "project#1")?.id, task.id);
    assert_eq!(get_task(&temp.path, "default", "project/#1")?.id, task.id);
    assert_eq!(
        get_task(&temp.path, "default", &format!("{}#1", board.id))?.id,
        task.id
    );
    Ok(())
}
