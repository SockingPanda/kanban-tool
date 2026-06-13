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
    let v2_sql = include_str!("../../../../migrations/001_initial.sql");
    let conn = Connection::open(&temp.path)?;
    conn.execute_batch(v2_sql)?;
    conn.execute_batch(include_str!(
        "../../../../migrations/002_knowledge_substrate.sql"
    ))?;
    conn.execute(
        "UPDATE schema_migrations SET checksum='fnv64:0ca871be950fc8a6' WHERE version=1",
        [],
    )?;
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

#[test]
fn storage_rejects_agent_type_for_non_agent_author_type() -> anyhow::Result<()> {
    let temp = TempDb::new("storage_rejects_agent_type_for_non_agent_author_type")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("storage invariant"),
    )?;

    let error = result_err(connect_file(&temp.path)?.execute(
        "INSERT INTO task_comments(id, board_id, task_id, author, author_type, agent_type, body, kind, created_at) \
         VALUES ('c_bad', ?1, ?2, 'tester', 'human', 'executor', 'bad', 'text', 1)",
        params![task.board_id, task.id],
    ))?;

    assert!(
        error.to_string().contains("CHECK constraint failed"),
        "error: {error}"
    );
    Ok(())
}

#[test]
fn legacy_jsonl_import_infers_comment_author_identity() -> anyhow::Result<()> {
    let source = TempDb::new("legacy_jsonl_import_infers_comment_author_identity_source")?;
    init_database(&source.path, "tester")?;
    let task = create_task(
        &source.path,
        "default",
        "tester",
        CreateTask::ready("legacy import"),
    )?;
    create_comment(&source.path, &task.id, "human", "text note", Some("text"))?;
    create_comment(
        &source.path,
        &task.id,
        "runner",
        "worker note",
        Some("worker"),
    )?;
    create_comment(
        &source.path,
        &task.id,
        "system",
        "system note",
        Some("system"),
    )?;

    let export_path = source.dir.join("comments.jsonl");
    export_jsonl(&source.path, "default", &export_path)?;
    let legacy_export = std::fs::read_to_string(&export_path)?
        .lines()
        .map(strip_comment_identity_fields)
        .collect::<anyhow::Result<Vec<_>>>()?
        .join("\n");
    let legacy_path = source.dir.join("legacy-comments.jsonl");
    std::fs::write(&legacy_path, format!("{legacy_export}\n"))?;

    let target = TempDb::new("legacy_jsonl_import_infers_comment_author_identity_target")?;
    init_database(&target.path, "tester")?;
    import_jsonl(&target.path, &legacy_path, true)?;

    let comments = list_comments(&target.path, &task.id)?;
    assert_eq!(comments.len(), 3);
    let text = comments
        .iter()
        .find(|comment| comment.kind == "text")
        .ok_or_else(|| test_error("missing text comment"))?;
    let worker = comments
        .iter()
        .find(|comment| comment.kind == "worker")
        .ok_or_else(|| test_error("missing worker comment"))?;
    let system = comments
        .iter()
        .find(|comment| comment.kind == "system")
        .ok_or_else(|| test_error("missing system comment"))?;
    assert_eq!(text.author_type, "human");
    assert_eq!(worker.author_type, "agent");
    assert_eq!(system.author_type, "system");
    assert_eq!(text.agent_type, None);
    assert_eq!(worker.agent_type, None);
    assert_eq!(system.agent_type, None);
    Ok(())
}

#[test]
fn legacy_jsonl_import_normalizes_task_priority() -> anyhow::Result<()> {
    let source = TempDb::new("legacy_jsonl_import_normalizes_task_priority_source")?;
    let legacy_path = source.dir.join("legacy-priority.jsonl");
    let records = vec![
        serde_json::json!({
            "type": "board",
            "data": {
                "id": "b_import",
                "slug": "default",
                "name": "Default",
                "description": null,
                "created_at": 1,
                "updated_at": 1,
                "archived_at": null
            }
        }),
        serde_json::json!({
            "type": "column",
            "data": {
                "id": "col_import_ready",
                "board_id": "b_import",
                "status": "ready",
                "title": "Ready",
                "position": 40,
                "hidden": 0,
                "wip_limit": null,
                "created_at": 1,
                "updated_at": 1
            }
        }),
    ];
    let mut lines = records
        .into_iter()
        .map(|record| record.to_string())
        .collect::<Vec<_>>();
    for (seq, title, priority) in [
        (1, "negative", -5),
        (2, "zero", 0),
        (3, "two", 2),
        (4, "eighty", 80),
    ] {
        lines.push(
            serde_json::json!({
                "type": "task",
                "data": {
                    "id": format!("t_import_{seq}"),
                    "board_id": "b_import",
                    "seq": seq,
                    "title": title,
                    "description": "ready spec",
                    "status": "ready",
                    "status_reason": null,
                    "assignee": null,
                    "priority": priority,
                    "position": seq * 1024,
                    "scheduled_at": null,
                    "due_at": null,
                    "created_by": "test",
                    "created_at": 1,
                    "updated_at": 1,
                    "started_at": null,
                    "completed_at": null,
                    "archived_at": null,
                    "claim_token": null,
                    "claim_owner": null,
                    "claim_expires_at": null,
                    "last_heartbeat_at": null,
                    "current_run_id": null,
                    "retry_count": 0,
                    "max_retries": null,
                    "result_summary": null,
                    "result_json": null,
                    "metadata_json": "{}",
                    "lock_version": 0
                }
            })
            .to_string(),
        );
    }
    std::fs::write(&legacy_path, format!("{}\n", lines.join("\n")))?;

    let target = TempDb::new("legacy_jsonl_import_normalizes_task_priority_target")?;
    init_database(&target.path, "tester")?;
    import_jsonl(&target.path, &legacy_path, true)?;

    let tasks = list_tasks(&target.path, "default", &[], true)?;
    let priorities = tasks
        .iter()
        .map(|task| (task.title.as_str(), task.priority))
        .collect::<Vec<_>>();
    assert_eq!(
        priorities,
        vec![("negative", 0), ("zero", 0), ("two", 2), ("eighty", 3)]
    );
    Ok(())
}

#[test]
fn legacy_jsonl_import_infers_agent_author_type_without_dropping_agent_type() -> anyhow::Result<()>
{
    let source = TempDb::new("legacy_jsonl_import_preserves_agent_type_source")?;
    init_database(&source.path, "tester")?;
    let task = create_task(
        &source.path,
        "default",
        "tester",
        CreateTask::ready("legacy agent import"),
    )?;
    create_comment_with_options(
        &source.path,
        &task.id,
        CreateComment {
            author: "runner".into(),
            body: "agent note".into(),
            kind: Some("worker".into()),
            author_type: Some("agent".into()),
            agent_type: Some("executor".into()),
        },
    )?;

    let export_path = source.dir.join("agent-comments.jsonl");
    export_jsonl(&source.path, "default", &export_path)?;
    let legacy_export = std::fs::read_to_string(&export_path)?
        .lines()
        .map(remove_comment_author_type)
        .collect::<anyhow::Result<Vec<_>>>()?
        .join("\n");
    let legacy_path = source.dir.join("legacy-agent-comments.jsonl");
    std::fs::write(&legacy_path, format!("{legacy_export}\n"))?;

    let target = TempDb::new("legacy_jsonl_import_preserves_agent_type_target")?;
    init_database(&target.path, "tester")?;
    import_jsonl(&target.path, &legacy_path, true)?;

    let comments = list_comments(&target.path, &task.id)?;
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].kind, "worker");
    assert_eq!(comments[0].author_type, "agent");
    assert_eq!(comments[0].agent_type.as_deref(), Some("executor"));
    Ok(())
}

fn strip_comment_identity_fields(line: &str) -> anyhow::Result<String> {
    let mut value = serde_json::from_str::<serde_json::Value>(line)?;
    if value["type"] == "comment" {
        let data = value["data"]
            .as_object_mut()
            .ok_or_else(|| test_error("expected comment data object"))?;
        data.remove("author_type");
        data.remove("agent_type");
    }
    Ok(value.to_string())
}

fn remove_comment_author_type(line: &str) -> anyhow::Result<String> {
    let mut value = serde_json::from_str::<serde_json::Value>(line)?;
    if value["type"] == "comment" {
        let data = value["data"]
            .as_object_mut()
            .ok_or_else(|| test_error("expected comment data object"))?;
        data.remove("author_type");
    }
    Ok(value.to_string())
}
