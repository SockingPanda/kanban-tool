use std::path::Path;

use kanban_core::{KanbanError, TaskStatus};
use kanban_sqlite::{
    BoardListOptions, CreateBoard, CreateTask, add_dependency, archive_board, claim_task,
    create_board, create_comment, create_task, get_board, get_task, init_database,
    list_board_columns, list_boards, list_comments, list_events, list_runs,
};

#[test]
fn create_board_adds_default_columns_and_created_event() {
    let temp = TempDb::new("create_board_adds_default_columns_and_created_event");
    init_database(&temp.path, "tester").unwrap();

    let board = create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "project.alpha".into(),
            name: "Project Alpha".into(),
            description: Some("local project board".into()),
        },
    )
    .unwrap();

    assert!(board.id.starts_with("b_"));
    assert_eq!(board.slug, "project.alpha");
    assert_eq!(board.name, "Project Alpha");
    assert_eq!(board.description.as_deref(), Some("local project board"));
    assert!(board.archived_at.is_none());

    let columns = list_board_columns(&temp.path, "project.alpha").unwrap();
    assert_eq!(columns.len(), 9);
    assert_eq!(columns[0].status, TaskStatus::Triage);
    assert_eq!(columns[8].status, TaskStatus::Archived);
    assert!(columns[8].hidden);

    let events = list_events(&temp.path, "project.alpha", None).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "board.created");
}

#[test]
fn board_slug_validation_rejects_reserved_prefixes_and_uppercase() {
    let temp = TempDb::new("board_slug_validation_rejects_reserved_prefixes_and_uppercase");
    init_database(&temp.path, "tester").unwrap();

    for slug in ["Bad", "t_work", "b_work", "col_work", ""] {
        let error = create_board(
            &temp.path,
            "tester",
            CreateBoard {
                slug: slug.into(),
                name: "Bad Board".into(),
                description: None,
            },
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("invalid board slug")
                || error.to_string().contains("slug is required"),
            "{slug}: {error}"
        );
    }
}

#[test]
fn duplicate_board_slug_returns_invalid_input() {
    let temp = TempDb::new("duplicate_board_slug_returns_invalid_input");
    init_database(&temp.path, "tester").unwrap();

    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "project".into(),
            name: "Project".into(),
            description: None,
        },
    )
    .unwrap();

    let error = create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "project".into(),
            name: "Duplicate Project".into(),
            description: None,
        },
    )
    .unwrap_err();

    assert!(matches!(error, KanbanError::InvalidInput(_)));
    assert!(error.to_string().contains("board slug already exists"));
}

#[test]
fn archived_board_is_hidden_by_default_and_rejects_task_writes() {
    let temp = TempDb::new("archived_board_is_hidden_by_default_and_rejects_task_writes");
    init_database(&temp.path, "tester").unwrap();
    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "archivable".into(),
            name: "Archivable".into(),
            description: None,
        },
    )
    .unwrap();

    let archived = archive_board(&temp.path, "archivable", "tester").unwrap();
    assert!(archived.archived_at.is_some());

    let active = list_boards(&temp.path, BoardListOptions::default()).unwrap();
    assert!(!active.iter().any(|board| board.slug == "archivable"));
    let all = list_boards(
        &temp.path,
        BoardListOptions {
            include_archived: true,
        },
    )
    .unwrap();
    assert!(all.iter().any(|board| board.slug == "archivable"));
    assert!(get_board(&temp.path, "archivable").is_err());

    let error = create_task(
        &temp.path,
        "archivable",
        "tester",
        CreateTask::ready("nope"),
    )
    .unwrap_err();
    assert!(error.to_string().contains("board archivable"));

    let events = list_events(&temp.path, "archivable", None).unwrap();
    assert_eq!(events.last().unwrap().kind, "board.archived");
}

#[test]
fn archived_board_keeps_read_only_history_inspectable() {
    let temp = TempDb::new("archived_board_keeps_read_only_history_inspectable");
    init_database(&temp.path, "tester").unwrap();
    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "archivable".into(),
            name: "Archivable".into(),
            description: None,
        },
    )
    .unwrap();
    let task = create_task(
        &temp.path,
        "archivable",
        "tester",
        CreateTask::ready("history task"),
    )
    .unwrap();
    create_comment(&temp.path, &task.id, "tester", "history note", None).unwrap();
    claim_task(&temp.path, "archivable", "runner", &task.id, 60_000).unwrap();

    archive_board(&temp.path, "archivable", "tester").unwrap();
    let create_after_archive =
        create_comment(&temp.path, &task.id, "tester", "late write", None).unwrap_err();
    assert!(matches!(create_after_archive, KanbanError::NotFound(_)));

    let qualified_ref = "archivable#1";
    let events = list_events(&temp.path, "archivable", Some(qualified_ref)).unwrap();
    assert!(events.iter().any(|event| event.kind == "task.created"));
    assert!(
        events
            .iter()
            .any(|event| event.kind == "task.comment.created")
    );

    let runs = list_runs(&temp.path, "archivable", Some(qualified_ref)).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].task_id, task.id);

    let comments = list_comments(&temp.path, qualified_ref).unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].body, "history note");
}

#[test]
fn cross_board_dependencies_are_rejected_even_with_global_refs() {
    let temp = TempDb::new("cross_board_dependencies_are_rejected_even_with_global_refs");
    init_database(&temp.path, "tester").unwrap();
    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "project".into(),
            name: "Project".into(),
            description: None,
        },
    )
    .unwrap();
    let default_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default task"),
    )
    .unwrap();
    let project_task = create_task(
        &temp.path,
        "project",
        "tester",
        CreateTask::ready("project task"),
    )
    .unwrap();

    let error = add_dependency(
        &temp.path,
        "project",
        "tester",
        &default_task.id,
        &project_task.id,
    )
    .unwrap_err();
    assert!(error.to_string().contains("cross-board dependency"));
}

#[test]
fn task_refs_resolve_board_seq_and_board_slug_prefixes() {
    let temp = TempDb::new("task_refs_resolve_board_seq_and_board_slug_prefixes");
    init_database(&temp.path, "tester").unwrap();
    let board = create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "project".into(),
            name: "Project".into(),
            description: None,
        },
    )
    .unwrap();
    let task = create_task(
        &temp.path,
        "project",
        "tester",
        CreateTask::ready("project task"),
    )
    .unwrap();

    assert_eq!(get_task(&temp.path, "project", "1").unwrap().id, task.id);
    assert_eq!(get_task(&temp.path, "project", "#1").unwrap().id, task.id);
    assert_eq!(
        get_task(&temp.path, "default", "project#1").unwrap().id,
        task.id
    );
    assert_eq!(
        get_task(&temp.path, "default", "project/#1").unwrap().id,
        task.id
    );
    assert_eq!(
        get_task(&temp.path, "default", &format!("{}#1", board.id))
            .unwrap()
            .id,
        task.id
    );
}

struct TempDb {
    dir: std::path::PathBuf,
    path: std::path::PathBuf,
}

impl TempDb {
    fn new(name: &str) -> Self {
        let mut dir = std::env::temp_dir();
        dir.push(format!("kb-sqlite-board-mvp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("kb.db");
        Self { dir, path }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl AsRef<Path> for TempDb {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}
