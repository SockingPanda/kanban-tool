use crate::common::*;

#[test]
fn comments_create_and_list_author_identity_fields() -> anyhow::Result<()> {
    let temp = TempDb::new("comments_create_and_list_author_identity_fields")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("comment identity"),
    )?;

    let human = create_comment_with_options(
        &temp.path,
        &task.id,
        CreateComment {
            author: " alice ".into(),
            body: " human note ".into(),
            kind: Some("text".into()),
            author_type: Some("human".into()),
            agent_type: None,
        },
    )?;
    let agent = create_comment_with_options(
        &temp.path,
        &task.id,
        CreateComment {
            author: "runner".into(),
            body: "agent note".into(),
            kind: Some("worker".into()),
            author_type: Some("agent".into()),
            agent_type: Some(" executor ".into()),
        },
    )?;
    let system = create_comment_with_options(
        &temp.path,
        &task.id,
        CreateComment {
            author: "system".into(),
            body: "system note".into(),
            kind: Some("system".into()),
            author_type: Some("system".into()),
            agent_type: None,
        },
    )?;

    assert_eq!(human.author, "alice");
    assert_eq!(human.author_type, "human");
    assert_eq!(human.agent_type, None);
    assert_eq!(agent.author_type, "agent");
    assert_eq!(agent.agent_type.as_deref(), Some("executor"));
    assert_eq!(system.author_type, "system");

    let comments = list_comments(&temp.path, &task.id)?;
    assert_eq!(comments.len(), 3);
    assert_eq!(comments[0].author_type, "human");
    assert_eq!(comments[1].author_type, "agent");
    assert_eq!(comments[1].agent_type.as_deref(), Some("executor"));
    assert_eq!(comments[2].author_type, "system");
    Ok(())
}

#[test]
fn comments_infer_author_type_for_legacy_requests() -> anyhow::Result<()> {
    let temp = TempDb::new("comments_infer_author_type_for_legacy_requests")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("legacy comment identity"),
    )?;

    let text = create_comment(&temp.path, &task.id, "human", "text note", None)?;
    let worker = create_comment(
        &temp.path,
        &task.id,
        "runner",
        "worker note",
        Some("worker"),
    )?;
    let system = create_comment(
        &temp.path,
        &task.id,
        "system",
        "system note",
        Some("system"),
    )?;

    assert_eq!(text.author_type, "human");
    assert_eq!(worker.author_type, "agent");
    assert_eq!(system.author_type, "system");
    assert_eq!(worker.agent_type, None);
    Ok(())
}

#[test]
fn comments_reject_agent_type_for_non_agent_authors() -> anyhow::Result<()> {
    let temp = TempDb::new("comments_reject_agent_type_for_non_agent_authors")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("invalid identity"),
    )?;

    let error = result_err(create_comment_with_options(
        &temp.path,
        &task.id,
        CreateComment {
            author: "alice".into(),
            body: "note".into(),
            kind: None,
            author_type: Some("human".into()),
            agent_type: Some("executor".into()),
        },
    ))?;

    assert!(matches!(error, KanbanError::InvalidInput(_)));
    assert!(error.to_string().contains("agent_type"));
    Ok(())
}

#[test]
fn migration_backfills_comment_author_type_from_kind() -> anyhow::Result<()> {
    let temp = TempDb::new("migration_backfills_comment_author_type_from_kind")?;
    let conn = Connection::open(&temp.path)?;
    conn.execute_batch(include_str!("../../../../migrations/001_initial.sql"))?;
    conn.execute_batch(include_str!(
        "../../../../migrations/002_knowledge_substrate.sql"
    ))?;
    conn.execute(
        "INSERT INTO boards(id, slug, name, description, created_at, updated_at, archived_at) VALUES ('b_test', 'default', 'Default', NULL, 1, 1, NULL)",
        [],
    )?;
    conn.execute(
        "INSERT INTO tasks(id, board_id, seq, title, description, status, created_by, created_at, updated_at, metadata_json) VALUES ('t_test', 'b_test', 1, 'Commented', 'ready spec', 'ready', 'tester', 2, 2, '{}')",
        [],
    )?;
    for (id, kind) in [
        ("c_text", "text"),
        ("c_worker", "worker"),
        ("c_system", "system"),
    ] {
        conn.execute(
            "INSERT INTO task_comments(id, board_id, task_id, author, body, kind, created_at) VALUES (?1, 'b_test', 't_test', 'tester', ?2, ?3, 3)",
            params![id, format!("{kind} body"), kind],
        )?;
    }
    conn.pragma_update(None, "user_version", 2)?;
    drop(conn);

    init_database(&temp.path, "tester")?;

    let comments = list_comments(&temp.path, "t_test")?;
    let text = comments
        .iter()
        .find(|comment| comment.id == "c_text")
        .ok_or_else(|| test_error("missing text comment"))?;
    let worker = comments
        .iter()
        .find(|comment| comment.id == "c_worker")
        .ok_or_else(|| test_error("missing worker comment"))?;
    let system = comments
        .iter()
        .find(|comment| comment.id == "c_system")
        .ok_or_else(|| test_error("missing system comment"))?;
    assert_eq!(text.author_type, "human");
    assert_eq!(worker.author_type, "agent");
    assert_eq!(worker.agent_type, None);
    assert_eq!(system.author_type, "system");
    Ok(())
}
