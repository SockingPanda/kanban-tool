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

    let user = create_comment_with_options(
        &temp.path,
        &task.id,
        CreateComment {
            author: " alice ".into(),
            body: " human note ".into(),
            kind: Some("note".into()),
            author_type: Some("user".into()),
            agent_type: None,
            metadata_json: Some(r#"{"source":"test"}"#.into()),
        },
    )?;
    let agent = create_comment_with_options(
        &temp.path,
        &task.id,
        CreateComment {
            author: "runner".into(),
            body: "agent note".into(),
            kind: Some("note".into()),
            author_type: Some("agent".into()),
            agent_type: Some(" executor ".into()),
            metadata_json: None,
        },
    )?;

    assert_eq!(user.author, "alice");
    assert_eq!(user.author_type, "user");
    assert_eq!(user.agent_type, None);
    assert_eq!(user.kind, "note");
    assert_eq!(user.metadata_json, r#"{"source":"test"}"#);
    assert_eq!(agent.author_type, "agent");
    assert_eq!(agent.agent_type.as_deref(), Some("executor"));
    assert_eq!(agent.metadata_json, "{}");

    let comments = list_comments(&temp.path, &task.id)?;
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].author_type, "user");
    assert_eq!(comments[0].metadata_json, r#"{"source":"test"}"#);
    assert_eq!(comments[1].author_type, "agent");
    assert_eq!(comments[1].agent_type.as_deref(), Some("executor"));
    Ok(())
}

#[test]
fn comments_default_to_user_note_empty_metadata() -> anyhow::Result<()> {
    let temp = TempDb::new("comments_default_to_user_note_empty_metadata")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("legacy comment identity"),
    )?;

    let comment = create_comment(&temp.path, &task.id, "alice", "note body", None)?;

    assert_eq!(comment.kind, "note");
    assert_eq!(comment.author_type, "user");
    assert_eq!(comment.agent_type, None);
    assert_eq!(comment.metadata_json, "{}");
    Ok(())
}

#[test]
fn comments_accept_decision_kind_with_user_default() -> anyhow::Result<()> {
    let temp = TempDb::new("comments_accept_decision_kind_with_user_default")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("decision comment"),
    )?;

    let comment = create_comment_with_options(
        &temp.path,
        &task.id,
        CreateComment {
            author: "alice".into(),
            body: "Problem: choose queue. Choice: sqlite. Reason: local invariant.".into(),
            kind: Some("decision".into()),
            author_type: None,
            agent_type: None,
            metadata_json: Some(decision_metadata()),
        },
    )?;

    assert_eq!(comment.kind, "decision");
    assert_eq!(comment.author_type, "user");
    assert_eq!(comment.agent_type, None);
    assert_eq!(comment.metadata_json, decision_metadata());

    let comments = list_comments(&temp.path, &task.id)?;
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].kind, "decision");
    assert_eq!(comments[0].author_type, "user");
    Ok(())
}

#[test]
fn comments_reject_invalid_decision_metadata_schema() -> anyhow::Result<()> {
    let temp = TempDb::new("comments_reject_invalid_decision_metadata_schema")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("decision schema"),
    )?;

    for (name, metadata, message) in [
        (
            "missing reason",
            r#"{"options":[{"slug":"a","title":"A","detail":"A"}],"selected":"a"}"#,
            "reason",
        ),
        (
            "empty options",
            r#"{"options":[],"selected":"a","reason":"because"}"#,
            "options",
        ),
        (
            "selected mismatch",
            r#"{"options":[{"slug":"a","title":"A","detail":"A"}],"selected":"b","reason":"because"}"#,
            "selected",
        ),
        (
            "duplicate slug",
            r#"{"options":[{"slug":"a","title":"A","detail":"A"},{"slug":"a","title":"B","detail":"B"}],"selected":"a","reason":"because"}"#,
            "unique",
        ),
        (
            "bad slug",
            r#"{"options":[{"slug":"Bad Slug","title":"A","detail":"A"}],"selected":"Bad Slug","reason":"because"}"#,
            "slug",
        ),
        (
            "padded slug",
            r#"{"options":[{"slug":" a ","title":"A","detail":"A"}],"selected":"a","reason":"because"}"#,
            "slug",
        ),
        (
            "padded selected",
            r#"{"options":[{"slug":"a","title":"A","detail":"A"}],"selected":" a ","reason":"because"}"#,
            "selected",
        ),
    ] {
        let error = result_err(create_comment_with_options(
            &temp.path,
            &task.id,
            CreateComment {
                author: "alice".into(),
                body: name.into(),
                kind: Some("decision".into()),
                author_type: None,
                agent_type: None,
                metadata_json: Some(metadata.into()),
            },
        ))?;
        assert!(
            error.to_string().contains(message),
            "{name}: expected {message}, got {error}"
        );
    }
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
            author_type: Some("user".into()),
            agent_type: Some("executor".into()),
            metadata_json: None,
        },
    ))?;

    assert!(matches!(error, KanbanError::InvalidInput(_)));
    assert!(error.to_string().contains("agent_type"));
    Ok(())
}

#[test]
fn comments_reject_invalid_metadata_json() -> anyhow::Result<()> {
    let temp = TempDb::new("comments_reject_invalid_metadata_json")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("invalid metadata"),
    )?;

    let invalid_json = result_err(create_comment_with_options(
        &temp.path,
        &task.id,
        CreateComment {
            author: "alice".into(),
            body: "note".into(),
            kind: None,
            author_type: None,
            agent_type: None,
            metadata_json: Some("{not-json".into()),
        },
    ))?;
    assert!(matches!(invalid_json, KanbanError::InvalidInput(_)));
    assert!(invalid_json.to_string().contains("metadata_json"));

    let invalid_shape = result_err(create_comment_with_options(
        &temp.path,
        &task.id,
        CreateComment {
            author: "alice".into(),
            body: "note".into(),
            kind: None,
            author_type: None,
            agent_type: None,
            metadata_json: Some("[]".into()),
        },
    ))?;
    assert!(matches!(invalid_shape, KanbanError::InvalidInput(_)));
    assert!(invalid_shape.to_string().contains("JSON object"));
    Ok(())
}

#[test]
fn migration_backfills_comment_contract_and_metadata() -> anyhow::Result<()> {
    let temp = TempDb::new("migration_backfills_comment_contract_and_metadata")?;
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
    assert_eq!(text.author_type, "user");
    assert_eq!(text.kind, "note");
    assert_eq!(text.metadata_json, "{}");
    assert_eq!(worker.author_type, "agent");
    assert_eq!(worker.kind, "note");
    assert_eq!(worker.agent_type, None);
    assert_eq!(system.author_type, "agent");
    assert_eq!(system.kind, "note");
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
         VALUES ('c_bad', ?1, ?2, 'tester', 'user', 'executor', 'bad', 'note', 1)",
        params![task.board_id, task.id],
    ))?;

    assert!(
        error.to_string().contains("CHECK constraint failed"),
        "error: {error}"
    );
    Ok(())
}

#[test]
fn storage_rejects_non_object_comment_metadata_json() -> anyhow::Result<()> {
    let temp = TempDb::new("storage_rejects_non_object_comment_metadata_json")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("storage metadata invariant"),
    )?;

    let error = result_err(connect_file(&temp.path)?.execute(
        "INSERT INTO task_comments(id, board_id, task_id, author, author_type, agent_type, body, kind, metadata_json, created_at) \
         VALUES ('c_bad', ?1, ?2, 'tester', 'user', NULL, 'bad', 'note', '[]', 1)",
        params![task.board_id, task.id],
    ))?;

    assert!(
        error.to_string().contains("CHECK constraint failed"),
        "error: {error}"
    );
    Ok(())
}

#[test]
fn migration_narrows_comment_kind_and_adds_metadata_to_existing_v5_database() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("migration_narrows_comment_kind_and_adds_metadata_to_existing_v5_database")?;
    let v1_sql = include_str!("../../../../migrations/001_initial.sql").replace(
        "'text', 'system', 'worker', 'decision'",
        "'text', 'system', 'worker'",
    );
    let conn = Connection::open(&temp.path)?;
    conn.execute_batch(&v1_sql)?;
    conn.execute_batch(include_str!(
        "../../../../migrations/002_knowledge_substrate.sql"
    ))?;
    conn.execute_batch(include_str!(
        "../../../../migrations/003_comment_author_identity.sql"
    ))?;
    conn.execute_batch(include_str!(
        "../../../../migrations/004_priority_levels.sql"
    ))?;
    conn.execute_batch(include_str!(
        "../../../../migrations/005_decision_comment_kind.sql"
    ))?;
    conn.execute(
        "INSERT INTO boards(id, slug, name, description, created_at, updated_at, archived_at) VALUES ('b_test', 'default', 'Default', NULL, 1, 1, NULL)",
        [],
    )?;
    conn.execute(
        "INSERT INTO tasks(id, board_id, seq, title, description, status, created_by, created_at, updated_at, metadata_json) VALUES ('t_test', 'b_test', 1, 'Decision', 'ready spec', 'ready', 'tester', 2, 2, '{}')",
        [],
    )?;
    conn.execute(
        "INSERT INTO task_comments(id, board_id, task_id, author, body, kind, created_at, author_type, agent_type) VALUES ('c_text', 'b_test', 't_test', 'tester', 'existing', 'text', 3, 'human', NULL)",
        [],
    )?;
    conn.execute(
        "INSERT INTO task_comments(id, board_id, task_id, author, body, kind, created_at, author_type, agent_type) VALUES ('c_decision_before', 'b_test', 't_test', 'tester', 'before', 'decision', 4, 'human', NULL)",
        [],
    )?;
    conn.pragma_update(None, "user_version", 5)?;
    drop(conn);

    init_database(&temp.path, "tester")?;

    let conn = Connection::open(&temp.path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(user_version, 8);
    let comments = list_comments(&temp.path, "t_test")?;
    assert_eq!(comments.len(), 2);
    assert!(comments.iter().all(|comment| comment.kind == "note"));
    assert!(comments.iter().all(|comment| comment.author_type == "user"));
    assert!(comments.iter().all(|comment| comment.metadata_json == "{}"));
    conn.execute(
        "INSERT INTO task_comments(id, board_id, task_id, author, body, kind, created_at, author_type, agent_type, metadata_json) VALUES ('c_decision_after', 'b_test', 't_test', 'tester', 'after', 'decision', 5, 'user', NULL, '{}')",
        [],
    )?;
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
    create_comment(&source.path, &task.id, "human", "text note", None)?;
    create_comment_with_options(
        &source.path,
        &task.id,
        CreateComment {
            author: "runner".into(),
            body: "worker note".into(),
            kind: Some("note".into()),
            author_type: Some("agent".into()),
            agent_type: None,
            metadata_json: None,
        },
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
    assert_eq!(comments.len(), 2);
    let text = comments
        .iter()
        .find(|comment| comment.body == "text note")
        .ok_or_else(|| test_error("missing text comment"))?;
    let worker = comments
        .iter()
        .find(|comment| comment.body == "worker note")
        .ok_or_else(|| test_error("missing worker comment"))?;
    assert_eq!(text.kind, "note");
    assert_eq!(worker.kind, "note");
    assert_eq!(text.author_type, "user");
    assert_eq!(worker.author_type, "agent");
    assert_eq!(text.agent_type, None);
    assert_eq!(worker.agent_type, None);
    assert_eq!(text.metadata_json, "{}");
    assert_eq!(worker.metadata_json, "{}");
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
            kind: Some("note".into()),
            author_type: Some("agent".into()),
            agent_type: Some("executor".into()),
            metadata_json: Some(r#"{"origin":"legacy"}"#.into()),
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
    assert_eq!(comments[0].kind, "note");
    assert_eq!(comments[0].author_type, "agent");
    assert_eq!(comments[0].agent_type.as_deref(), Some("executor"));
    assert_eq!(comments[0].metadata_json, r#"{"origin":"legacy"}"#);
    Ok(())
}

#[test]
fn jsonl_import_rejects_comment_metadata_json_non_object() -> anyhow::Result<()> {
    let source = TempDb::new("jsonl_import_rejects_comment_metadata_json_non_object_source")?;
    init_database(&source.path, "tester")?;
    let task = create_task(
        &source.path,
        "default",
        "tester",
        CreateTask::ready("invalid metadata import"),
    )?;
    create_comment_with_options(
        &source.path,
        &task.id,
        CreateComment {
            author: "runner".into(),
            body: "agent note".into(),
            kind: Some("note".into()),
            author_type: Some("agent".into()),
            agent_type: Some("executor".into()),
            metadata_json: Some(r#"{"origin":"export"}"#.into()),
        },
    )?;

    let export_path = source.dir.join("comments.jsonl");
    export_jsonl(&source.path, "default", &export_path)?;
    let invalid_export = std::fs::read_to_string(&export_path)?
        .lines()
        .map(replace_comment_metadata_json_with_array)
        .collect::<anyhow::Result<Vec<_>>>()?
        .join("\n");
    let invalid_path = source.dir.join("invalid-comments.jsonl");
    std::fs::write(&invalid_path, format!("{invalid_export}\n"))?;

    let target = TempDb::new("jsonl_import_rejects_comment_metadata_json_non_object_target")?;
    init_database(&target.path, "tester")?;
    let error = result_err(import_jsonl(&target.path, &invalid_path, true))?;
    assert!(error.to_string().contains("metadata_json"));
    Ok(())
}

#[test]
fn jsonl_import_rejects_invalid_decision_metadata_schema() -> anyhow::Result<()> {
    let source = TempDb::new("jsonl_import_rejects_invalid_decision_metadata_schema_source")?;
    let import_path = source.dir.join("invalid-decision.jsonl");
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
        serde_json::json!({
            "type": "task",
            "data": {
                "id": "t_import",
                "board_id": "b_import",
                "seq": 1,
                "title": "Decision import",
                "description": "ready spec",
                "status": "ready",
                "status_reason": null,
                "assignee": null,
                "priority": 3,
                "position": 1024,
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
        }),
        serde_json::json!({
            "type": "comment",
            "data": {
                "id": "c_import",
                "board_id": "b_import",
                "task_id": "t_import",
                "author": "tester",
                "author_type": "user",
                "agent_type": null,
                "body": "invalid decision",
                "kind": "decision",
                "metadata_json": r#"{"options":[{"slug":" import ","title":"Import","detail":"Import detail"}],"selected":"import","reason":"because"}"#,
                "created_at": 1
            }
        }),
    ];
    std::fs::write(
        &import_path,
        format!(
            "{}\n",
            records
                .into_iter()
                .map(|record| record.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )?;

    let target = TempDb::new("jsonl_import_rejects_invalid_decision_metadata_schema_target")?;
    init_database(&target.path, "tester")?;
    let error = result_err(import_jsonl(&target.path, &import_path, true))?;
    assert!(error.to_string().contains("slug"), "error: {error}");
    Ok(())
}

fn strip_comment_identity_fields(line: &str) -> anyhow::Result<String> {
    let mut value = serde_json::from_str::<serde_json::Value>(line)?;
    if value["type"] == "comment" {
        let data = value["data"]
            .as_object_mut()
            .ok_or_else(|| test_error("expected comment data object"))?;
        if data.get("body").and_then(|value| value.as_str()) == Some("worker note") {
            data.insert("kind".into(), serde_json::json!("worker"));
        }
        data.remove("author_type");
        data.remove("agent_type");
    }
    Ok(value.to_string())
}

fn replace_comment_metadata_json_with_array(line: &str) -> anyhow::Result<String> {
    let mut value = serde_json::from_str::<serde_json::Value>(line)?;
    if value["type"] == "comment" {
        let data = value["data"]
            .as_object_mut()
            .ok_or_else(|| test_error("expected comment data object"))?;
        data.insert("metadata_json".into(), serde_json::json!("[]"));
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

fn decision_metadata() -> String {
    r#"{"options":[{"slug":"sqlite","title":"Use SQLite","detail":"Keep the decision payload in comment metadata."},{"slug":"table","title":"Add a table","detail":"Store decisions in a separate table."}],"selected":"sqlite","reason":"Keeps decisions local to the discussion.","risk":"Schema drift would make older comments ambiguous.","verification":"Service, CLI, API, and Desktop tests cover the contract."}"#.into()
}
